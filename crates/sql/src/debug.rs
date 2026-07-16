//! Inspection helpers: turn a SQL statement into a human-readable explanation of
//! the DataFusion logical plan and the WOQL it compiles to — without touching a
//! database. Handy while iterating (mirrors `terminusdb-xpath`'s `debug`).

use terminusdb_woql2::dsl::ToDSL;

use crate::catalog::Catalog;
use crate::error::Result;

/// A compiled-query explanation.
pub struct Explanation {
    pub sql: String,
    /// The DataFusion logical plan (indented).
    pub logical_plan: String,
    /// The WOQL rendered as its DSL.
    pub dsl: String,
    /// The WOQL JSON-LD that would be sent to the server.
    pub json: serde_json::Value,
    /// The projected SQL columns and the WOQL variables carrying them.
    pub projection: Vec<(String, String)>,
}

impl Explanation {
    /// A multi-line report suitable for printing.
    pub fn report(&self) -> String {
        let cols = self
            .projection
            .iter()
            .map(|(n, v)| format!("{n} = ?{v}"))
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "SQL:\n  {}\n\nLogicalPlan:\n{}\n\nWOQL (DSL):\n  {}\n\nProjection: {}\n\nWOQL (JSON):\n{}",
            self.sql,
            self.logical_plan,
            self.dsl,
            cols,
            serde_json::to_string_pretty(&self.json).unwrap_or_default(),
        )
    }
}

/// Compile `sql` against `catalog` and produce an [`Explanation`].
pub fn explain(sql: &str, catalog: &Catalog) -> Result<Explanation> {
    let plan = catalog.plan(sql)?;
    let logical_plan = format!("{}", plan.display_indent());
    let compiled = crate::compile_sql(sql, catalog)?;
    Ok(Explanation {
        sql: sql.to_string(),
        logical_plan,
        dsl: compiled.woql.to_dsl(),
        json: compiled.woql.to_woql_json(),
        projection: compiled
            .projection
            .iter()
            .map(|p| (p.sql_name.clone(), p.woql_var.clone()))
            .collect(),
    })
}
