//! Database and graph specification types with builder pattern.
//!
//! These types provide compile-time safe construction of TerminusDB path specifications.

use std::fmt;

/// Graph type for operations that target specific graphs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphType {
    /// Instance data graph
    Instance,
    /// Schema graph
    Schema,
}

impl GraphType {
    pub fn as_str(&self) -> &'static str {
        match self {
            GraphType::Instance => "instance",
            GraphType::Schema => "schema",
        }
    }
}

/// Database specification following the pattern:
/// `<organization>/<database>/<repository>/branch|commit/<name>`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbSpec {
    organization: String,
    database: String,
    repository: String,
    location: Location,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Location {
    /// Implied main branch
    Default,
    /// Specific branch
    Branch(String),
    /// Specific commit
    Commit(String),
    /// Repository commits graph
    Commits,
    /// Repository metadata
    Meta,
}

impl DbSpec {
    /// Create a new database specification.
    ///
    /// This creates a spec for `<organization>/<database>/local/branch/main`
    ///
    /// # Example
    ///
    /// ```
    /// use terminusdb_bin::api::DbSpec;
    ///
    /// let spec = DbSpec::new("admin", "mydb");
    /// assert_eq!(spec.to_string(), "admin/mydb/local/branch/main");
    /// ```
    pub fn new(organization: impl Into<String>, database: impl Into<String>) -> Self {
        Self {
            organization: organization.into(),
            database: database.into(),
            repository: "local".to_string(),
            location: Location::Default,
        }
    }

    /// Create a system database specification (_system).
    ///
    /// # Example
    ///
    /// ```
    /// use terminusdb_bin::api::DbSpec;
    ///
    /// let spec = DbSpec::system();
    /// assert_eq!(spec.to_string(), "_system");
    /// ```
    pub fn system() -> Self {
        Self {
            organization: "_system".to_string(),
            database: String::new(),
            repository: String::new(),
            location: Location::Default,
        }
    }

    /// Set the repository (default: "local").
    pub fn repository(mut self, repo: impl Into<String>) -> Self {
        self.repository = repo.into();
        self
    }

    /// Set to a specific branch.
    pub fn branch(mut self, name: impl Into<String>) -> Self {
        self.location = Location::Branch(name.into());
        self
    }

    /// Set to a specific commit.
    pub fn commit(mut self, id: impl Into<String>) -> Self {
        self.location = Location::Commit(id.into());
        self
    }

    /// Set to repository metadata graph.
    pub fn meta(mut self) -> Self {
        self.location = Location::Meta;
        self
    }

    /// Set to commits graph.
    pub fn commits(mut self) -> Self {
        self.location = Location::Commits;
        self
    }

    /// Convert to a graph specification for a specific graph type.
    pub fn graph(self, graph_type: GraphType) -> GraphSpec {
        GraphSpec {
            db_spec: self,
            graph_type,
        }
    }
}

impl fmt::Display for DbSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.organization == "_system" {
            return write!(f, "_system");
        }

        write!(f, "{}/{}", self.organization, self.database)?;

        if !self.repository.is_empty() {
            write!(f, "/{}", self.repository)?;
        }

        match &self.location {
            Location::Default => write!(f, "/branch/main"),
            Location::Branch(name) => write!(f, "/branch/{}", name),
            Location::Commit(id) => write!(f, "/commit/{}", id),
            Location::Commits => write!(f, "/_commits"),
            Location::Meta => write!(f, "/_meta"),
        }
    }
}

/// Branch specification (subset of DbSpec).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchSpec {
    db_spec: DbSpec,
}

impl BranchSpec {
    /// Create a branch specification from a database spec.
    pub fn new(db_spec: DbSpec) -> Self {
        Self { db_spec }
    }

    /// Get the underlying database spec.
    pub fn db_spec(&self) -> &DbSpec {
        &self.db_spec
    }
}

impl From<DbSpec> for BranchSpec {
    fn from(db_spec: DbSpec) -> Self {
        Self::new(db_spec)
    }
}

impl fmt::Display for BranchSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.db_spec)
    }
}

/// Commit specification (subset of DbSpec).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitSpec {
    db_spec: DbSpec,
}

impl CommitSpec {
    /// Create a commit specification from a database spec.
    pub fn new(db_spec: DbSpec) -> Self {
        Self { db_spec }
    }

    /// Get the underlying database spec.
    pub fn db_spec(&self) -> &DbSpec {
        &self.db_spec
    }
}

impl From<DbSpec> for CommitSpec {
    fn from(db_spec: DbSpec) -> Self {
        Self::new(db_spec)
    }
}

impl fmt::Display for CommitSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.db_spec)
    }
}

/// Graph specification: DB_SPEC + graph type (instance or schema).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphSpec {
    db_spec: DbSpec,
    graph_type: GraphType,
}

impl GraphSpec {
    /// Create a new graph specification.
    pub fn new(db_spec: DbSpec, graph_type: GraphType) -> Self {
        Self {
            db_spec,
            graph_type,
        }
    }

    /// Get the underlying database spec.
    pub fn db_spec(&self) -> &DbSpec {
        &self.db_spec
    }

    /// Get the graph type.
    pub fn graph_type(&self) -> GraphType {
        self.graph_type
    }
}

impl fmt::Display for GraphSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.db_spec, self.graph_type.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_db_spec_basic() {
        let spec = DbSpec::new("admin", "mydb");
        assert_eq!(spec.to_string(), "admin/mydb/local/branch/main");
    }

    #[test]
    fn test_db_spec_system() {
        let spec = DbSpec::system();
        assert_eq!(spec.to_string(), "_system");
    }

    #[test]
    fn test_db_spec_custom_branch() {
        let spec = DbSpec::new("admin", "mydb").branch("dev");
        assert_eq!(spec.to_string(), "admin/mydb/local/branch/dev");
    }

    #[test]
    fn test_db_spec_commit() {
        let spec = DbSpec::new("admin", "mydb").commit("abc123");
        assert_eq!(spec.to_string(), "admin/mydb/local/commit/abc123");
    }

    #[test]
    fn test_db_spec_meta() {
        let spec = DbSpec::new("admin", "mydb").meta();
        assert_eq!(spec.to_string(), "admin/mydb/local/_meta");
    }

    #[test]
    fn test_db_spec_commits() {
        let spec = DbSpec::new("admin", "mydb").commits();
        assert_eq!(spec.to_string(), "admin/mydb/local/_commits");
    }

    #[test]
    fn test_graph_spec() {
        let spec = DbSpec::new("admin", "mydb").graph(GraphType::Instance);
        assert_eq!(spec.to_string(), "admin/mydb/local/branch/main/instance");

        let spec = DbSpec::new("admin", "mydb").branch("dev").graph(GraphType::Schema);
        assert_eq!(spec.to_string(), "admin/mydb/local/branch/dev/schema");
    }
}
