use crate::*;

/// The negation of a query. Provides no solution bindings, but will succeed if its sub-query fails.
ast_struct!(
    Not as not {
        /// The query which must not hold.
        query: Query
    }
);

impl ToCLIQueryAST for Not {
    fn to_ast(&self) -> String {
        todo!()
    }
}