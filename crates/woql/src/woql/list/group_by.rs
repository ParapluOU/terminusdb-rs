use crate::*;

/// Group a query into a list with each element of the list specified by 'template' using a given variable set for the group.
ast_struct!(
    GroupBy as group_by {
        /// The template of elements in the result list.
        template: Value,
        /// The variables which should be grouped into like solutions.
        group_by: (Vec<Variable>),
        /// The subquery providing the solutions for the grouping.
        query: Query,
        /// The final list of templated solutions.
        grouped: Value
    }
);

impl ToCLIQueryAST for GroupBy {
    fn to_ast(&self) -> String {
        todo!()
    }
}