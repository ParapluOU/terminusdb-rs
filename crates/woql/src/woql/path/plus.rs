use crate::*;

/// The path pattern specified by 'plus' must hold one or more times in succession.
ast_struct!(
    PathPlus as path_plus {
        /// A path patterns.
        plus: PathPattern
    }
);

impl ToCLIQueryAST for PathPlus {
    fn to_ast(&self) -> String {
        todo!()
    }
}