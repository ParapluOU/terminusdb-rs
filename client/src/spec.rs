use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BranchSpec {
    /// DB/dataset to insert into. This is NOT the branch, but more like a "product"
    pub db: String,
    /// branch for versioning product data
    pub branch: Option<String>,
}

impl<AsStr: AsRef<str>> From<AsStr> for BranchSpec {
    fn from(value: AsStr) -> Self {
        Self {
            db: value.as_ref().to_string(),
            branch: None,
        }
    }
}

impl AsRef<String> for BranchSpec {
    fn as_ref(&self) -> &String {
        &self.db
    }
}
