use crate::document::DocumentType;
use crate::spec::BranchSpec;
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
    /// force despite it already exists
    pub force: bool,
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
            },
            force: false,
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
