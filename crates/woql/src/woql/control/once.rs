use crate::*;

/// Obtains exactly one solution from a query. Simliar to a limit of 1.
ast_struct!(
    Once as once {
        /// The query from which to obtain a solution."
        query: Query
    }
);

impl ToCLIQueryAST for Once {
    fn to_ast(&self) -> String {
        todo!()
    }
}