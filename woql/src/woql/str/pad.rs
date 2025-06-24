use crate::*;

/// Pad a string.
ast_struct!(
    Pad as pad {
        /// The starting string.
        string: DataValue,
        /// The padding character.
        char: DataValue,
        /// The number of times to repeat the padding character.
        times: DataValue,
        /// The result of the padding as a string.
        result: DataValue
    }
);

impl ToCLIQueryAST for Pad {
    fn to_ast(&self) -> String {
        todo!()
    }
}