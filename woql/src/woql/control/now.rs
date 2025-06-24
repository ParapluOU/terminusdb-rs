use crate::*;

/// Attempts to perform all side-effecting operations immediately. Can have strange non-backtracking effects but can also increase performance. Use at your own risk.
ast_struct!(
    Immediately as now {
        /// The query from which to obtain the side-effects."
        query: Query
    }
);

impl ToCLIQueryAST for Immediately {
    fn to_ast(&self) -> String {
        todo!()
    }
}