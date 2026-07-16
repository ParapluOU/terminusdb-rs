//! Error type for the SQL → WOQL compiler.
//!
//! The discipline mirrors `terminusdb-xpath`: a small, explicit error enum where
//! every construct we cannot faithfully translate becomes an [`SqlError::Unsupported`]
//! naming the construct — never a silent approximation.

/// Errors produced while loading a catalog, planning, or emitting WOQL.
#[derive(Debug, thiserror::Error)]
pub enum SqlError {
    /// The target database does not exist on the server.
    #[error("database `{0}` does not exist")]
    DatabaseNotFound(String),

    /// The requested branch does not exist on the database (`branch`, `db`).
    #[error("branch `{0}` does not exist on database `{1}`")]
    BranchNotFound(String, String),

    /// Authentication against the server failed.
    #[error("authentication failed")]
    Auth,

    /// The schema graph could not be read from the server.
    #[error("could not read schema graph: {0}")]
    SchemaRead(String),

    /// A schema document could not be parsed into our raw model.
    #[error("could not parse schema document: {0}")]
    SchemaParse(String),

    /// Two distinct schema entities mangle to the same SQL identifier. This is a
    /// hard error at load time — we never pick a winner (no last-write-wins).
    #[error("SQL identifier `{sql}` is produced by two schema entities: `{first}` and `{second}`")]
    IdentifierCollision {
        sql: String,
        first: String,
        second: String,
    },

    /// A referenced column exists in the schema but is not representable in v1.
    #[error("column `{column}` exists in table `{table}` but is unsupported ({reason})")]
    UnsupportedColumn {
        table: String,
        column: String,
        reason: String,
    },

    /// A referenced table exists in the schema but is not representable in v1
    /// (e.g. an abstract class or a subdocument class).
    #[error("table `{table}` exists in the schema but is unsupported ({reason})")]
    UnsupportedTable { table: String, reason: String },

    /// The SQL string could not be parsed.
    #[error("failed to parse SQL: {0}")]
    Parse(String),

    /// DataFusion planning (name resolution / type checking) failed.
    #[error("SQL planning failed: {0}")]
    Plan(String),

    /// The statement planned successfully but uses a construct outside the subset
    /// the emitter can faithfully translate to WOQL.
    #[error("unsupported SQL construct: {0}")]
    Unsupported(String),

    /// The branch moved since the session pinned its commit — reconnect to pick
    /// up the new schema (`pinned_commit`, `current_head`).
    #[error("schema changed since connect; reconnect (session pinned at {0}, branch now at {1})")]
    SchemaDrift(String, String),

    /// The SQL string contained no statement.
    #[error("empty SQL statement")]
    Empty,
}

pub type Result<T> = std::result::Result<T, SqlError>;

impl SqlError {
    pub(crate) fn unsupported(msg: impl Into<String>) -> Self {
        SqlError::Unsupported(msg.into())
    }
}
