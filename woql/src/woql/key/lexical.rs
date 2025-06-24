use crate::*;

/// Generates a key identical to those generated automatically by 'LexicalKey' specifications.
ast_struct!(
    LexicalKey as key_lexical {
        /// The URI base to the left of the key.
        base: DataValue,
        /// List of data elements required to generate the key.
        key_list: (Vec<DataValue>),
        /// The resulting URI.
        uri: NodeValue
    }
);

impl ToCLIQueryAST for LexicalKey {
    fn to_ast(&self) -> String {
        todo!()
    }
}