use crate::*;

/// length of a list
ast_struct!(
    Length as len {
        /// The list of which to find the length.
        list: DataValue,
        /// The length of the list.
        length: DataValue
    }
);

impl ToCLIQueryAST for Length {
    fn to_ast(&self) -> String {
        todo!()
    }
}