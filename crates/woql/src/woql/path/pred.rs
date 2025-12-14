use crate::*;

/// A predicate to traverse.
ast_struct!(
    PathPredicate as path_pred {
        /// The predicate to use in the pattern traversal.
        predicate: (Option<String>)
    }
);

impl ToCLIQueryAST for PathPredicate {
    fn to_ast(&self) -> String {
        todo!()
    }
}