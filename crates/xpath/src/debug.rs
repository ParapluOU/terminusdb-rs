//! Make compiled WOQL *observable*.
//!
//! WOQL is hard to debug: it has no static types, sparse docs, and no REPL, so a
//! subtly-wrong query tends to either fail with a cryptic "Badly formed ast" or —
//! worse — silently return nothing. The first line of defence is simply *seeing*
//! what an XPath compiled to before (or instead of) blaming the data.
//!
//! [`explain`] is pure (no database) and renders both surfaces of a compiled
//! query: the human-readable WOQL **DSL** and the exact **JSON-LD** that the
//! client POSTs to `/api/woql`.

use terminusdb_woql2::prelude::ToDSL;

use crate::{compile, CompiledXPath, Result};

/// A human-facing view of a compiled XPath expression.
#[derive(Debug, Clone)]
pub struct Explanation {
    /// The original XPath source.
    pub xpath: String,
    /// The database named by `db("...")`, if any.
    pub using_db: Option<String>,
    /// The variable that holds the result.
    pub result_var: String,
    /// The WOQL DSL rendering (best-effort; unsupported nodes show as `// TODO`).
    pub dsl: String,
    /// The JSON-LD actually sent to the server (`query.to_woql_json()`).
    pub json: serde_json::Value,
}

impl Explanation {
    /// A multi-line report suitable for printing while iterating.
    pub fn report(&self) -> String {
        let json = serde_json::to_string_pretty(&self.json).unwrap_or_else(|_| self.json.to_string());
        format!(
            "xpath:   {}\ndb:      {}\nresult:  ?{}\n--- WOQL DSL ---\n{}\n--- WOQL JSON-LD ---\n{}",
            self.xpath,
            self.using_db.as_deref().unwrap_or("<current>"),
            self.result_var,
            self.dsl,
            json,
        )
    }
}

/// Compile `expr` and describe the resulting WOQL, without touching a database.
pub fn explain(expr: &str) -> Result<Explanation> {
    let compiled = compile(expr)?;
    Ok(explain_compiled(expr, &compiled))
}

/// Describe an already-[`compile`]d query.
pub fn explain_compiled(expr: &str, compiled: &CompiledXPath) -> Explanation {
    Explanation {
        xpath: expr.to_string(),
        using_db: compiled.using_db.clone(),
        result_var: compiled.result_var.clone(),
        dsl: compiled.query.to_dsl(),
        json: compiled.query.to_woql_json(),
    }
}
