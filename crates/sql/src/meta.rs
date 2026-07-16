//! Catalog metadata shared between the loader (Half 1) and the emitter (Half 2).
//!
//! A [`TableMeta`] carries everything the emitter needs to turn a `LogicalPlan`
//! node referencing this table back into WOQL: the real class IRI (for the
//! `rdf:type` filter), and per-column the WOQL predicate, its kind, and its
//! nullability. Omitted (non-representable) properties are recorded *with a reason*
//! so a query referencing one yields a precise "unsupported" error.

use datafusion_common::arrow::datatypes::SchemaRef;

use crate::typemap::{RejectReason, Semantic};

/// The synthetic name of the subject-IRI column present on every table.
///
/// We call it `iri` (not `id`) because TerminusDB documents commonly carry their
/// own `id` datatype property distinct from the node identity; naming the
/// synthetic column `iri` exposes the node IRI without shadowing a real `id`
/// column. Object-property foreign keys join against a target class's `iri`.
pub const IRI_COLUMN: &str = "iri";

/// A concrete class exposed as a SQL table.
#[derive(Debug, Clone)]
pub struct TableMeta {
    /// The mangled SQL identifier for this table.
    pub sql_name: String,
    /// The real class IRI, used to build the `rdf:type` → `@schema:Class` filter.
    pub class_iri: String,
    /// Arrow schema: field order matches [`TableMeta::columns`] exactly.
    pub arrow: SchemaRef,
    /// Column metadata, 1:1 and in the same order as `arrow.fields()`.
    pub columns: Vec<ColumnMeta>,
    /// Properties omitted from the catalog, each with the reason.
    pub omitted: Vec<OmittedColumn>,
}

impl TableMeta {
    /// Look up a column by its (already normalised) SQL name.
    pub fn column(&self, sql_name: &str) -> Option<&ColumnMeta> {
        self.columns.iter().find(|c| c.sql_name == sql_name)
    }
}

/// A representable column on a table.
#[derive(Debug, Clone)]
pub struct ColumnMeta {
    /// The mangled SQL identifier for this column.
    pub sql_name: String,
    /// The WOQL predicate to emit for this column's triple, e.g. `@schema:name`.
    /// `None` for the synthetic `id` column, which binds the row subject and has
    /// no triple of its own.
    pub predicate: Option<String>,
    /// What kind of column this is (drives literal/join emission).
    pub kind: ColumnKind,
    /// Whether the column may be SQL NULL (`Optional` cardinality). The `id`
    /// column and exactly-one properties are non-nullable.
    pub nullable: bool,
}

impl ColumnMeta {
    /// True for the synthetic subject-IRI column.
    pub fn is_id(&self) -> bool {
        matches!(self.kind, ColumnKind::Id)
    }
}

/// The role a column plays, which determines how literals and joins against it are
/// emitted.
#[derive(Debug, Clone)]
pub enum ColumnKind {
    /// The subject-IRI column (`id`): binds the row subject variable. A literal
    /// compared against it is a node IRI, not a data value.
    Id,
    /// A datatype property → a scalar value column.
    Scalar { semantic: Semantic },
    /// An enum-typed property → a string column; `values` are the allowed members.
    Enum { values: Vec<String> },
    /// An object property → an IRI foreign key, joinable against the target class's
    /// `iri` column.
    ObjectRef { target_class_iri: String },
}

/// A property that exists in the schema but is not representable as a v1 column.
#[derive(Debug, Clone)]
pub struct OmittedColumn {
    /// The SQL name the column would have had (for the error message).
    pub sql_name: String,
    /// The property's name / IRI in the schema.
    pub prop_iri: String,
    /// Why it was omitted.
    pub reason: OmitReason,
}

/// Why a property or class is not representable in v1.
#[derive(Debug, Clone)]
pub enum OmitReason {
    /// A Set/List/Array-valued property. The `&str` names the container.
    MultiValued(&'static str),
    /// A property whose range is a subdocument class.
    Subdocument,
    /// A property whose datatype the type map rejects.
    RejectedType(RejectReason),
    /// A class that is abstract (not a concrete document table).
    AbstractClass,
    /// A class that is a subdocument (not a top-level table).
    SubdocumentClass,
}

impl std::fmt::Display for OmitReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OmitReason::MultiValued(c) => write!(f, "multi-valued: {c}"),
            OmitReason::Subdocument => write!(f, "range is a subdocument"),
            OmitReason::RejectedType(r) => write!(f, "{r}"),
            OmitReason::AbstractClass => write!(f, "abstract class"),
            OmitReason::SubdocumentClass => write!(f, "subdocument class"),
        }
    }
}
