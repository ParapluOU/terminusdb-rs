use crate::*;

/// Uppercase a string.
ast_struct!(
    Upper as upper {
        /// The mixed case string.
        mixed: DataValue,
        /// The upper case string.
        upper: DataValue
    }
);

impl ToCLIQueryAST for Upper {
    fn to_ast(&self) -> String {
        todo!()
    }
}