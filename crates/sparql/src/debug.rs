//! Make compiled WOQL observable without a database — the first debugging tool
//! when a query returns nothing or errors.

use terminusdb_woql2::dsl::ToDSL;

use crate::compile::CompileOptions;
use crate::error::Result;

/// A human-readable explanation of what a SPARQL query compiled to.
#[derive(Debug, Clone)]
pub struct Explanation {
    /// The original SPARQL text.
    pub sparql: String,
    /// The projected variable names.
    pub variables: Vec<String>,
    /// The compiled WOQL rendered as the readable WOQL DSL.
    pub dsl: String,
    /// The compiled WOQL as the JSON-LD actually POSTed to `/api/woql`.
    pub json: serde_json::Value,
}

impl Explanation {
    /// A multi-line report: SPARQL, projected variables, WOQL DSL, WOQL JSON.
    pub fn report(&self) -> String {
        format!(
            "SPARQL:\n  {}\n\nprojected variables: {}\n\nWOQL (DSL):\n{}\n\nWOQL (JSON-LD):\n{}",
            self.sparql.trim(),
            self.variables.join(", "),
            indent(&self.dsl, "  "),
            indent(
                &serde_json::to_string_pretty(&self.json).unwrap_or_default(),
                "  "
            ),
        )
    }
}

/// Compile `sparql` and produce an [`Explanation`] using default options.
pub fn explain(sparql: &str) -> Result<Explanation> {
    explain_with(sparql, &CompileOptions::default())
}

/// Like [`explain`], with explicit [`CompileOptions`].
pub fn explain_with(sparql: &str, opts: &CompileOptions) -> Result<Explanation> {
    let compiled = crate::compile_with(sparql, opts)?;
    Ok(Explanation {
        sparql: sparql.to_string(),
        variables: compiled.variables,
        dsl: compiled.query.to_dsl(),
        json: compiled.query.to_woql_json(),
    })
}

fn indent(s: &str, pad: &str) -> String {
    s.lines()
        .map(|l| format!("{pad}{l}"))
        .collect::<Vec<_>>()
        .join("\n")
}
