use crate::*;

/// Start a query at the nth solution specified by 'start'. Allows resumption and paging of queries.
ast_struct!(
    Start as start {
        ///The query to perform."
        query: Query,
        ///The numbered solution to start at."
        start: usize
    }
);

impl ToCLIQueryAST for Start {
    fn to_ast(&self) -> String {
        todo!()
    }
}