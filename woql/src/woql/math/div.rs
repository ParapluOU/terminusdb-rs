use crate::*;

/// Subtract two numbers.
ast_struct!(
    Divide as divide {
        /// First operand of add.
        left: ArithmeticExpression,
        /// Second operand of add.
        right: ArithmeticExpression
    }
);

impl ToCLIQueryAST for Divide {
    fn to_ast(&self) -> String {
        todo!()
    }
}

//
//
//

/// Divide two integers.
ast_struct!(
    Div as div {
        /// First operand of add.
        left: ArithmeticExpression,
        /// Second operand of add.
        right: ArithmeticExpression
    }
);

impl ToCLIQueryAST for Div {
    fn to_ast(&self) -> String {
        todo!()
    }
}