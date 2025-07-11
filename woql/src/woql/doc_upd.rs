use crate::*;

/// Update a document in the graph.
ast_struct!(
    UpdateDocument as update_doc {
        /// The document to update. Must either have an '@id' or have a class specified key.
        document: Value,
        /// An optional returned identifier specifying the documentation location.
        identifier: (Option<NodeValue>)
    }
);

impl ToCLIQueryAST for UpdateDocument {
    fn to_ast(&self) -> String {
        todo!()
    }
}