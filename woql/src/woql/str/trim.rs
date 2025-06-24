use crate::*;

/// Trims whitespace from 'untrimmed
ast_struct!(
    Trim as trim {
        /// The untrimmed string
        untrimmed: DataValue,
        /// The string to be trimmed.
        trimmed: DataValue
    }
);

impl ToCLIQueryAST for Trim {
    fn to_ast(&self) -> String {
        todo!()
    }
}