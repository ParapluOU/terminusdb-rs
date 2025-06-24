use crate::*;

/// Join a list of strings using 'separator'.
ast_struct!(
    Join as join {
        /// The list to concatenate.
        list: DataValue,
        /// The separator between each joined string
        separator: DataValue,
        /// The result string.
        result: DataValue
    }
);

impl ToCLIQueryAST for Join {
    fn to_ast(&self) -> String {
        todo!()
    }
}