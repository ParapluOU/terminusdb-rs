use crate::*;

/// Distance between strings, similar to a Levenstein distance.
ast_struct!(
    Like as like {
        /// The first string.
        left: DataValue,
        /// The second string.
        right: DataValue,
        /// Number between -1 and 1 which gives a scale for similarity.
        similarity: DataValue
    }
);

impl ToCLIQueryAST for Like {
    fn to_ast(&self) -> String {
        todo!()
    }
}