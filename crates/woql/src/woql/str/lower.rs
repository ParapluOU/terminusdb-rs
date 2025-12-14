use crate::*;

/// Lowercase a string.
ast_struct!(
    Lower as lower {
        /// The mixed case string.
        mixed: DataValue,
        /// The upper case string.
        lower: DataValue
    }
);

impl ToCLIQueryAST for Lower {
    fn to_ast(&self) -> String {
        todo!()
    }
}