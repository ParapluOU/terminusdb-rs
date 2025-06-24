use crate::*;

/// Limit a query to a particular maximum number of solutions specified by 'limit'. Can be used with start to perform paging.
ast_struct!(
    Limit as limit {
        /// The query to perform."
        query: Query,
        /// The numbered solution to start at."
        limit: usize
    }
);

impl ToCLIQueryAST for Limit {
    fn to_ast(&self) -> String {
        todo!()
    }
}