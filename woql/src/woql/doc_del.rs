use crate::*;

/// Delete a document from the graph.
ast_struct!(
    DeleteDocument as delete_doc {
        /// "An identifier specifying the documentation location to delete."
        identifier: NodeValue
    }
);

impl ToCLIQueryAST for DeleteDocument {
    fn to_ast(&self) -> String {
        todo!()
    }
}