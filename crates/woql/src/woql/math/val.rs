use crate::*;

/// A variable or node.
ast_struct! (
    ArithmeticValue {
        data(XSDAnySimpleType),
        variable(Variable)
    }
);