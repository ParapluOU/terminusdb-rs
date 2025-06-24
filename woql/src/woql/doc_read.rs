use crate::*;

/// Read a full document from an identifier.
ast_struct!(
    ReadDocument as read_doc {
        /// The document to update. Must either have an '@id' or have a class specified key.
        document: Value,
        /// An optional returned identifier specifying the documentation location.
        identifier: NodeValue
    }
);

impl ToCLIQueryAST for ReadDocument {
    fn to_ast(&self) -> String {
        todo!()
    }
}