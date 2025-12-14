use crate::*;

/// The order template, consisting of the variable and ordering direction.
ast_struct!(
    OrderTemplate as order_tpl {
        order: Order,
        variable: Variable
    }
);

impl ToCLIQueryAST for OrderTemplate {
    fn to_ast(&self) -> String {
        todo!()
    }
}