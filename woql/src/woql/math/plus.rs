use crate::*;

/// Add two numbers.
ast_struct!(
    Plus as plus {
        /// First operand of add.
        left: ArithmeticExpression,
        /// Second operand of add.
        right: ArithmeticExpression
    }
);

impl ToCLIQueryAST for Plus {
    fn to_ast(&self) -> String {
        todo!()
    }
}