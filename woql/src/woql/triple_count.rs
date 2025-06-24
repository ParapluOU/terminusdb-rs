use crate::*;

/// The number of edges in a database.
ast_struct!(
    TripleCount as triple_count {
        /// The resource to obtain the edges from.
        resource: String,
        /// The count of edges.
        count: DataValue
    }
);

impl ToCLIQueryAST for TripleCount {
    fn to_ast(&self) -> String {
        todo!()
    }
}