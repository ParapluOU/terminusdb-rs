use crate::*;

/// A set of patterns in which each of the patterns can result in objects starting from our current subject set.
ast_struct!(
    PathOr as path_or {
        /// A disjunction of path patterns.
        or: (Vec<PathPattern>)
    }
);

impl ToCLIQueryAST for PathOr {
    fn to_ast(&self) -> String {
        todo!()
    }
}