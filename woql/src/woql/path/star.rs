use crate::*;

/// The path pattern specified by 'star' may hold zero or more times in succession.
ast_struct!(
    PathStar as path_star {
        /// A path pattern.
        star: PathPattern
    }
);

impl ToCLIQueryAST for PathStar {
    fn to_ast(&self) -> String {
        todo!()
    }
}