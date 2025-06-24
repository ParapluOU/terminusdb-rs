use crate::*;

/// A conjunction of queries which must all have a solution.
ast_struct!(
    And as and {
        /// List of queries which must hold.
        and: (Vec<Query>)
    }
);

impl ToCLIQueryAST for And {
    fn to_ast(&self) -> String {
        format!("({})", self.and.to_ast()).to_string()
    }
}

#[macro_export]
macro_rules! and {
    ($($q:expr),+) => {
        and(
            vec!(
                $( {let q : Query = $q.into(); q} ),*
            )
        )
    }
}