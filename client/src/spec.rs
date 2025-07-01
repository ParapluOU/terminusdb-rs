use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BranchSpec {
    /// DB/dataset to insert into. This is NOT the branch, but more like a "product"
    pub db: String,
    /// branch for versioning product data
    pub branch: Option<String>,
    /// commit reference for time-travel queries (commit ID)
    pub ref_commit: Option<String>,
}

impl<AsStr: AsRef<str>> From<AsStr> for BranchSpec {
    fn from(value: AsStr) -> Self {
        Self {
            db: value.as_ref().to_string(),
            branch: None,
            ref_commit: None,
        }
    }
}

impl AsRef<String> for BranchSpec {
    fn as_ref(&self) -> &String {
        &self.db
    }
}

impl BranchSpec {
    /// Create a new BranchSpec with database name only
    pub fn new(db: impl Into<String>) -> Self {
        Self {
            db: db.into(),
            branch: None,
            ref_commit: None,
        }
    }

    /// Create a BranchSpec with database and branch
    pub fn with_branch(db: impl Into<String>, branch: impl Into<String>) -> Self {
        Self {
            db: db.into(),
            branch: Some(branch.into()),
            ref_commit: None,
        }
    }

    /// Create a BranchSpec pointing to a specific commit for time-travel queries
    pub fn with_commit(db: impl Into<String>, commit_id: impl Into<String>) -> Self {
        Self {
            db: db.into(),
            branch: None,
            ref_commit: Some(commit_id.into()),
        }
    }

    /// Set the commit reference for time-travel functionality
    pub fn ref_commit(mut self, commit_id: impl Into<String>) -> Self {
        self.ref_commit = Some(commit_id.into());
        self
    }

    /// Check if this BranchSpec points to a specific commit
    pub fn is_commit_ref(&self) -> bool {
        self.ref_commit.is_some()
    }

    /// Get the commit ID if this is a commit reference
    pub fn commit_id(&self) -> Option<&str> {
        self.ref_commit.as_deref()
    }
}
