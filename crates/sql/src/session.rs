//! A session: a catalog pinned to a concrete commit plus the backend that serves
//! it. This is the top-level entry point for running SQL against TerminusDB.

use datafusion_expr::LogicalPlan;

use crate::backend::CatalogBackend;
use crate::catalog::Catalog;
use crate::emit::SqlQuery;
use crate::error::{Result, SqlError};
use crate::runner::QueryResponse;

/// A SQL session over one database/branch, pinned to the commit resolved at
/// [`Session::open`]. The schema we type-check against is provably the schema we
/// query. Schema changes are handled at the session level (reconnect), never by
/// per-query revalidation — see [`Session::check_drift`].
pub struct Session<B: CatalogBackend> {
    backend: B,
    db: String,
    branch: Option<String>,
    catalog: Catalog,
}

impl<B: CatalogBackend> Session<B> {
    /// Open a session: validate db/branch/auth, resolve the branch to a concrete
    /// commit, read the schema graph at that commit, and build the catalog.
    pub async fn open(backend: B, db: &str, branch: Option<&str>) -> Result<Self> {
        let commit = backend.resolve_commit(db, branch).await?;
        let docs = backend.read_schema_documents(db, &commit).await?;
        let catalog = Catalog::build(commit, &docs)?;
        Ok(Session {
            backend,
            db: db.to_string(),
            branch: branch.map(String::from),
            catalog,
        })
    }

    /// The loaded catalog.
    pub fn catalog(&self) -> &Catalog {
        &self.catalog
    }

    /// The commit this session is pinned to.
    pub fn commit(&self) -> &str {
        self.catalog.commit()
    }

    /// Plan a SQL statement to a DataFusion `LogicalPlan` (no emission).
    pub fn plan_sql(&self, sql: &str) -> Result<LogicalPlan> {
        self.catalog.plan(sql)
    }

    /// Compile a SQL statement to WOQL (no execution).
    pub fn compile(&self, sql: &str) -> Result<SqlQuery> {
        crate::compile_sql(sql, &self.catalog)
    }

    /// Compile and execute a SQL statement, returning decoded rows.
    pub async fn run(&self, sql: &str) -> Result<QueryResponse> {
        let query = crate::compile_sql(sql, &self.catalog)?;
        let woql = query.woql.to_woql_json();
        let bindings = self
            .backend
            .execute_woql(&self.db, self.catalog.commit(), woql)
            .await?;
        Ok(QueryResponse::decode(bindings, &query.projection))
    }

    /// Check whether the branch has moved since this session pinned its commit.
    /// A moved branch means the schema we type-checked against may differ from the
    /// live one — the caller should reconnect (open a new session).
    pub async fn check_drift(&self) -> Result<()> {
        let latest = self
            .backend
            .resolve_commit(&self.db, self.branch.as_deref())
            .await?;
        if latest != self.catalog.commit() {
            return Err(SqlError::SchemaDrift(
                self.catalog.commit().to_string(),
                latest,
            ));
        }
        Ok(())
    }
}
