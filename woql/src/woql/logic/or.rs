use crate::*;

/// A disjunction of queries any of which can provide a solution.
ast_struct!(
    Or as or {
        /// List of queries which may hold.
        or: (Vec<Query>)
    }
);

impl ToCLIQueryAST for Or {
    fn to_ast(&self) -> String {
        todo!()
    }
}