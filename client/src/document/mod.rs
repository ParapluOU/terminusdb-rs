mod get;
mod insert;

pub use {get::*, insert::*};

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum DocumentType {
    #[default]
    Instance,
    Schema,
}

impl DocumentType {
    pub fn is_instance(&self) -> bool {
        matches!(self, DocumentType::Instance)
    }
}

impl ToString for DocumentType {
    fn to_string(&self) -> String {
        format!("{:?}", self).to_lowercase()
    }
}
