use crate::*;

/// Subtract two numbers.
ast_struct!(
    Minus as minus {
        /// First operand of add.
        left: ArithmeticExpression,
        /// Second operand of add.
        right: ArithmeticExpression
    }
);

impl ToCLIQueryAST for Minus {
    fn to_ast(&self) -> String {
        todo!()
    }
}