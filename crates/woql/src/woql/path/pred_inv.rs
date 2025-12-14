use crate::*;

/// A predicate to traverse *backwards*.
ast_struct!(
    InversePathPredicate as path_predicate_inverse {
        /// The predicate to use in reverse direction in the pattern traversal.
        predicate: (Option<String>)
    }
);

impl ToCLIQueryAST for InversePathPredicate {
    fn to_ast(&self) -> String {
        todo!()
    }
}