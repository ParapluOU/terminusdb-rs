use crate::*;

/// Counts the number of solutions of a query.
ast_struct!(
    Count as count {
        ///The query from which to obtain the count"
        query: Query,

        ///The count of the number of solutions"
        count: DataValue
    }
);

impl ToCLIQueryAST for Count {
    fn to_ast(&self) -> String {
        format!("count({}, {})",
                self.query.to_ast(),
                self.count.to_ast()).to_string()
    }
}