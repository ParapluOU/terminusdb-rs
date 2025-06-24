use crate::*;

/// Generates a key identical to those generated automatically by 'LexicalKey' specifications.
ast_struct!(
    RandomKey as key_random {
        /// The URI base to the left of the key.
        base: DataValue,
        /// The resulting URI.
        uri: NodeValue
    }
);

impl ToCLIQueryAST for RandomKey {
    fn to_ast(&self) -> String {
        todo!()
    }
}