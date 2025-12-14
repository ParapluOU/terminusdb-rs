use crate::*;

/// Orders query results according to an ordering specification.
ast_struct!(
    OrderBy as order_by {
        /// The base query giving the solutions to order.
        query: Query,
        /// A specification of the ordering of solutions.
        ordering: (Vec<OrderTemplate>)
    }
);

impl ToCLIQueryAST for OrderBy {
    fn to_ast(&self) -> String {
        todo!()
    }
}