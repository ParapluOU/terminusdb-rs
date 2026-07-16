//! The catalog: a truthful mirror of the TerminusDB schema graph, pinned to a
//! concrete commit, exposed to DataFusion as a set of tables.
//!
//! [`Catalog::build`] reads schema-graph documents and produces one table per
//! concrete document class. Datatype properties become value columns; object
//! properties become IRI foreign-key columns; every table gets a synthetic `iri`
//! column (the subject IRI). Non-representable properties are *omitted with a
//! recorded reason* so that a query touching one yields a precise
//! [`SqlError::UnsupportedColumn`] rather than "no such column".

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use datafusion_common::arrow::datatypes::{DataType, Field, Schema as ArrowSchema, SchemaRef};
use datafusion_common::config::ConfigOptions;
use datafusion_common::DataFusionError;
use datafusion_expr::LogicalPlan;
use datafusion_sql::parser::DFParser;
use datafusion_sql::planner::SqlToRel;
use serde_json::Value;
use terminusdb_schema::is_primitive;

use crate::context::TdbContextProvider;
use crate::error::{Result, SqlError};
use crate::mangle::{mangle, IdentMap};
use crate::meta::{ColumnKind, ColumnMeta, OmitReason, OmittedColumn, TableMeta, IRI_COLUMN};
use crate::schema_read::{parse_schema, Family, RawClass, RawProperty};
use crate::typemap::datatype_to_arrow;

/// Turn a schema entity name into the WOQL prefixed-name form used as a triple
/// predicate / type object (`Person` → `@schema:Person`). Already-qualified names
/// (containing `:`) are left untouched. This mirrors `terminusdb-xpath`.
pub(crate) fn schema_iri(name: &str) -> String {
    if name.contains(':') {
        name.to_string()
    } else {
        format!("@schema:{name}")
    }
}

/// A commit-pinned mirror of the schema graph, ready to plan SQL against.
#[derive(Debug)]
pub struct Catalog {
    /// The concrete commit id this catalog was type-checked against.
    commit: String,
    /// Concrete class tables, keyed by mangled SQL name.
    tables: HashMap<String, TableMeta>,
    /// Classes present but not representable as tables, with the reason.
    omitted_tables: HashMap<String, OmitReason>,
    /// Omitted columns, keyed by (table sql name, column sql name), with reason.
    omitted_cols: HashMap<(String, String), OmitReason>,
    /// DataFusion planning options (identifier normalisation etc.).
    options: ConfigOptions,
}

impl Catalog {
    /// Build a catalog from schema-graph documents pinned at `commit`.
    pub fn build(commit: impl Into<String>, docs: &[Value]) -> Result<Catalog> {
        let raw = parse_schema(docs)?;

        let class_by_id: HashMap<&str, &RawClass> =
            raw.classes.iter().map(|c| (c.id.as_str(), c)).collect();
        let enum_values: HashMap<&str, &Vec<String>> =
            raw.enums.iter().map(|e| (e.id.as_str(), &e.values)).collect();

        let mut table_idents = IdentMap::new();
        let mut tables = HashMap::new();
        let mut omitted_tables = HashMap::new();
        let mut omitted_cols = HashMap::new();

        for class in &raw.classes {
            // Abstract and subdocument classes are not v1 tables; record why so a
            // reference to them explains itself.
            if class.is_abstract {
                omitted_tables.insert(mangle(&class.id), OmitReason::AbstractClass);
                continue;
            }
            if effectively_subdocument(class, &class_by_id) {
                omitted_tables.insert(mangle(&class.id), OmitReason::SubdocumentClass);
                continue;
            }

            let sql_name = table_idents.insert(&class.id)?;
            let props = resolve_properties(class, &class_by_id);

            let mut col_idents = IdentMap::new();
            col_idents.reserve(IRI_COLUMN, "<iri column>");

            let mut columns = Vec::new();
            let mut fields = Vec::new();
            let mut omitted = Vec::new();

            // The synthetic subject-IRI column.
            columns.push(ColumnMeta {
                sql_name: IRI_COLUMN.to_string(),
                predicate: None,
                kind: ColumnKind::Id,
                nullable: false,
            });
            fields.push(Field::new(IRI_COLUMN, DataType::Utf8, false));

            for prop in &props {
                // Multi-valued properties are rejected at load time.
                if let Some(fam) = prop.family {
                    if fam.is_multivalued() {
                        record_omitted(
                            &sql_name,
                            prop,
                            OmitReason::MultiValued(fam.container_name()),
                            &mut omitted,
                            &mut omitted_cols,
                        );
                        continue;
                    }
                }

                let nullable = matches!(prop.family, Some(Family::Optional)) || prop.force_nullable;

                // Classify the property's range.
                let (kind, datatype) = if is_primitive(&prop.class) || prop.class.starts_with("sys:")
                {
                    match datatype_to_arrow(&prop.class) {
                        Ok((dt, semantic)) => (ColumnKind::Scalar { semantic }, dt),
                        Err(reason) => {
                            record_omitted(
                                &sql_name,
                                prop,
                                OmitReason::RejectedType(reason),
                                &mut omitted,
                                &mut omitted_cols,
                            );
                            continue;
                        }
                    }
                } else if let Some(values) = enum_values.get(prop.class.as_str()) {
                    (ColumnKind::Enum { values: (*values).clone() }, DataType::Utf8)
                } else if class_by_id
                    .get(prop.class.as_str())
                    .is_some_and(|c| effectively_subdocument(c, &class_by_id))
                {
                    record_omitted(
                        &sql_name,
                        prop,
                        OmitReason::Subdocument,
                        &mut omitted,
                        &mut omitted_cols,
                    );
                    continue;
                } else {
                    // An object property: a link to another (concrete or abstract)
                    // class. The value is an IRI foreign key joinable to that
                    // class's `iri` column.
                    (
                        ColumnKind::ObjectRef {
                            target_class_iri: prop.class.clone(),
                        },
                        DataType::Utf8,
                    )
                };

                let col_sql = col_idents.insert(&prop.name)?;
                fields.push(Field::new(&col_sql, datatype, nullable));
                columns.push(ColumnMeta {
                    sql_name: col_sql,
                    predicate: Some(schema_iri(&prop.name)),
                    kind,
                    nullable,
                });
            }

            let arrow: SchemaRef = Arc::new(ArrowSchema::new(fields));
            tables.insert(
                sql_name.clone(),
                TableMeta {
                    sql_name,
                    class_iri: class.id.clone(),
                    arrow,
                    columns,
                    omitted,
                },
            );
        }

        Ok(Catalog {
            commit: commit.into(),
            tables,
            omitted_tables,
            omitted_cols,
            options: ConfigOptions::default(),
        })
    }

    /// The commit this catalog is pinned to.
    pub fn commit(&self) -> &str {
        &self.commit
    }

    /// Look up a table by its (normalised) SQL name.
    pub fn table(&self, sql_name: &str) -> Option<&TableMeta> {
        self.tables.get(sql_name)
    }

    /// All tables, unordered.
    pub fn tables(&self) -> impl Iterator<Item = &TableMeta> {
        self.tables.values()
    }

    /// The omission reason for a class that exists but is not a v1 table.
    pub(crate) fn omitted_table_reason(&self, sql_name: &str) -> Option<&OmitReason> {
        self.omitted_tables.get(sql_name)
    }

    pub(crate) fn options(&self) -> &ConfigOptions {
        &self.options
    }

    /// A [`ContextProvider`](datafusion_expr::planner::ContextProvider) borrowing
    /// this catalog for one planning call.
    pub(crate) fn context_provider(&self) -> TdbContextProvider<'_> {
        TdbContextProvider::new(self)
    }

    /// Parse and plan a single SQL statement into a `LogicalPlan`. DataFusion does
    /// all name resolution and type checking here; we only translate planning
    /// failures into precise catalog-aware [`SqlError`]s.
    pub fn plan(&self, sql: &str) -> Result<LogicalPlan> {
        let mut stmts =
            DFParser::parse_sql(sql).map_err(|e| SqlError::Parse(e.to_string()))?;
        let stmt = stmts.pop_front().ok_or(SqlError::Empty)?;
        if !stmts.is_empty() {
            return Err(SqlError::unsupported("multiple statements in one query"));
        }
        SqlToRel::new(&self.context_provider())
            .statement_to_plan(stmt)
            .map_err(|e| self.enrich_plan_error(e))
    }

    /// Turn a DataFusion planning error into a precise catalog-aware error when it
    /// concerns an omitted column/table, so the user learns the column *exists but
    /// is unsupported* rather than seeing a bare "not found".
    pub(crate) fn enrich_plan_error(&self, e: DataFusionError) -> SqlError {
        let msg = e.to_string();
        for ((table, col), reason) in &self.omitted_cols {
            if error_mentions(&msg, col) {
                return SqlError::UnsupportedColumn {
                    table: table.clone(),
                    column: col.clone(),
                    reason: reason.to_string(),
                };
            }
        }
        for (table, reason) in &self.omitted_tables {
            if error_mentions(&msg, table) {
                return SqlError::UnsupportedTable {
                    table: table.clone(),
                    reason: reason.to_string(),
                };
            }
        }
        SqlError::Plan(msg)
    }
}

/// Heuristic: does a DataFusion error name this identifier? We check for the
/// identifier delimited by quotes/word boundaries to avoid matching a substring of
/// a longer name.
fn error_mentions(msg: &str, ident: &str) -> bool {
    let bytes = msg.as_bytes();
    let ilen = ident.len();
    let mut from = 0;
    while let Some(pos) = msg[from..].find(ident) {
        let start = from + pos;
        let end = start + ilen;
        let before_ok = start == 0 || !is_ident_char(bytes[start - 1]);
        let after_ok = end >= bytes.len() || !is_ident_char(bytes[end]);
        if before_ok && after_ok {
            return true;
        }
        from = start + 1;
    }
    false
}

fn is_ident_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn record_omitted(
    table_sql: &str,
    prop: &RawProperty,
    reason: OmitReason,
    omitted: &mut Vec<OmittedColumn>,
    omitted_cols: &mut HashMap<(String, String), OmitReason>,
) {
    let sql = mangle(&prop.name);
    omitted.push(OmittedColumn {
        sql_name: sql.clone(),
        prop_iri: prop.name.clone(),
        reason: reason.clone(),
    });
    omitted_cols.insert((table_sql.to_string(), sql), reason);
}

/// A class is effectively a subdocument if it, or any ancestor it `@inherits`
/// from, is declared `@subdocument`.
fn effectively_subdocument(class: &RawClass, by_id: &HashMap<&str, &RawClass>) -> bool {
    fn walk(class: &RawClass, by_id: &HashMap<&str, &RawClass>, seen: &mut HashSet<String>) -> bool {
        if class.is_subdocument {
            return true;
        }
        if !seen.insert(class.id.clone()) {
            return false;
        }
        class
            .inherits
            .iter()
            .filter_map(|p| by_id.get(p.as_str()))
            .any(|parent| walk(parent, by_id, seen))
    }
    let mut seen = HashSet::new();
    walk(class, by_id, &mut seen)
}

/// Resolve a class's full property set, merging inherited properties. Child
/// properties override parents with the same name; cycles are guarded.
fn resolve_properties(class: &RawClass, by_id: &HashMap<&str, &RawClass>) -> Vec<RawProperty> {
    fn collect(
        class: &RawClass,
        by_id: &HashMap<&str, &RawClass>,
        acc: &mut Vec<RawProperty>,
        seen: &mut HashSet<String>,
        visited: &mut HashSet<String>,
    ) {
        if !visited.insert(class.id.clone()) {
            return;
        }
        for p in &class.properties {
            if seen.insert(p.name.clone()) {
                acc.push(p.clone());
            }
        }
        for parent_id in &class.inherits {
            if let Some(parent) = by_id.get(parent_id.as_str()) {
                collect(parent, by_id, acc, seen, visited);
            }
        }
    }
    let mut acc = Vec::new();
    let mut seen = HashSet::new();
    let mut visited = HashSet::new();
    collect(class, by_id, &mut acc, &mut seen, &mut visited);
    acc
}
