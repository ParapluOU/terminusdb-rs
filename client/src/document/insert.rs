use crate::document::DocumentType;
use crate::spec::BranchSpec;
use std::time::Duration;
use tap::Tap;

#[derive(Clone)]
pub struct DocumentInsertArgs {
    /// commit message
    pub message: String,
    /// type of document to insert. for functions that expect schema's, this is overridden
    pub ty: DocumentType,
    /// author of commit. TODO: do something more structured with this
    pub author: String,
    /// spec
    pub spec: BranchSpec,
    /// Controls whether to use the `full_replace=true` query parameter in POST operations:
    /// - `false` (default): Normal POST behavior - fails if document IDs already exist.
    /// - `true`: Uses `full_replace=true` - TerminusDB will replace any existing documents with the same ID.
    ///
    /// This field works independently from `skip_existence_check`. Common combinations:
    /// - `force=false, skip_existence_check=false` (default): Safe, checks and updates via PUT
    /// - `force=true, skip_existence_check=true`: Fastest, skips check and replaces any duplicates
    /// - `force=false, skip_existence_check=true`: Fast but may error on duplicates
    /// - `force=true, skip_existence_check=false`: Checks then replaces (validates but always succeeds)
    pub force: bool,
    /// Controls whether to check for existing document IDs before POST operations:
    /// - `false` (default): Perform WOQL query to check for existing IDs and automatically update them via PUT.
    ///   This is safer and prevents duplicate errors, but requires an additional WOQL query.
    /// - `true`: Skip the existence check for better performance.
    ///   Useful for bulk insertions of known-new documents or when using `force=true`.
    ///
    /// When disabled and duplicates exist, behavior depends on the `force` field:
    /// - If `force=false`: POST will fail with a duplicate ID error from TerminusDB
    /// - If `force=true`: POST will succeed and replace existing documents
    pub skip_existence_check: bool,
    /// optional request timeout
    pub timeout: Option<Duration>,
}

impl DocumentInsertArgs {
    pub fn as_schema(self) -> Self {
        self.tap_mut(|a| {
            a.ty = DocumentType::Schema;
        })
    }

    pub fn with_force(mut self, force: bool) -> Self {
        self.force = force;
        self
    }

    pub fn with_skip_existence_check(mut self, skip: bool) -> Self {
        self.skip_existence_check = skip;
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }
}

impl Default for DocumentInsertArgs {
    fn default() -> Self {
        Self {
            message: "insert document".to_string(),
            ty: Default::default(),
            author: "system".to_string(),
            spec: BranchSpec {
                db: "test".to_string(),
                branch: None,
                ref_commit: None,
            },
            force: false,
            skip_existence_check: false,
            timeout: None,
        }
    }
}

impl AsRef<BranchSpec> for DocumentInsertArgs {
    fn as_ref(&self) -> &BranchSpec {
        &self.spec
    }
}

impl From<BranchSpec> for DocumentInsertArgs {
    fn from(value: BranchSpec) -> Self {
        Self::default().tap_mut(|args| {
            args.spec = value;
        })
    }
}
