use crate::*;

/// Split a string.
ast_struct!(
    Split as split {
        /// The starting string.
        string: DataValue,
        /// The splitting pattern.
        pattern: DataValue,
        /// The result list of strings.
        list: DataValue
    }
);

impl ToCLIQueryAST for Split {
    fn to_ast(&self) -> String {
        todo!()
    }
}