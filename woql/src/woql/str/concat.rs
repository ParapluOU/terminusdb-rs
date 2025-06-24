use crate::*;

/// Concatenate a list of strings.
ast_struct!(
    Concatenate as concat {
        /// The list to concatenate.
        list: DataValue,
        /// The result string.
        result: DataValue
    }
);

impl ToCLIQueryAST for Concatenate {
    fn to_ast(&self) -> String {
        todo!()
    }
}