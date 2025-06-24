use crate::*;

/// Predicate determining if one thing is greater than another according to natural ordering.
ast_struct!(
    Greater as gt {
        /// The greater element.
        left: DataValue,
        /// The lesser element.
        right: DataValue
    }
);

impl ToCLIQueryAST for Greater {
    fn to_ast(&self) -> String {
        todo!()
    }
}