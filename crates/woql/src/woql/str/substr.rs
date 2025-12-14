use crate::*;

/// Finds the boundaries of a substring in a string.
ast_struct!(
    Substring as substr {
        /// The super-string as data or variable.
        string: DataValue,
        /// The length of the string as an integer or variable.
        length: DataValue,
        /// The count of characters before substring as an integer or variable.
        before: DataValue,
        /// The count of characters after substring as an integer or variable.
        after: DataValue,
        /// The super-string as data or variable.
        substring: DataValue
    }
);

impl ToCLIQueryAST for Substring {
    fn to_ast(&self) -> String {
        todo!()
    }
}