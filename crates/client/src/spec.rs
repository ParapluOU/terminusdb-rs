use crate::CommitId;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BranchSpec {
    /// DB/dataset to insert into. This is NOT the branch, but more like a "product"
    pub db: String,
    /// branch for versioning product data
    pub branch: Option<String>,
    /// commit reference for time-travel queries (commit ID)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    #[serde(
        deserialize_with = "deserialize_commit_id",
        serialize_with = "serialize_commit_id"
    )]
    pub ref_commit: Option<CommitId>,
}

fn deserialize_commit_id<'de, D>(deserializer: D) -> Result<Option<CommitId>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt: Option<String> = Option::deserialize(deserializer)?;
    Ok(opt.map(CommitId::from))
}

fn serialize_commit_id<S>(commit_id: &Option<CommitId>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    match commit_id {
        Some(id) => serializer.serialize_some(id.as_str()),
        None => serializer.serialize_none(),
    }
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
    pub fn with_commit(db: impl Into<String>, commit_id: impl Into<CommitId>) -> Self {
        Self {
            db: db.into(),
            branch: None,
            ref_commit: Some(commit_id.into()),
        }
    }

    /// Set the commit reference for time-travel functionality
    pub fn ref_commit(mut self, commit_id: impl Into<CommitId>) -> Self {
        self.ref_commit = Some(commit_id.into());
        self
    }

    /// Check if this BranchSpec points to a specific commit
    pub fn is_commit_ref(&self) -> bool {
        self.ref_commit.is_some()
    }

    /// Get the commit ID if this is a commit reference
    pub fn commit_id(&self) -> Option<&CommitId> {
        self.ref_commit.as_ref()
    }
}
