use crate::*;

/// Find the integral part of a number.
ast_struct!(
    Floor as floor {
        argument: ArithmeticExpression
    }
);

impl ToCLIQueryAST for Floor {
    fn to_ast(&self) -> String {
        todo!()
    }
}