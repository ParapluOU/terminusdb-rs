use crate::*;

/// Ensure variables listed result in distinct solutions.
ast_struct!(
    Distinct as distinct {
        /// The query which will be run prior to selection.
        query: Query,
        /// The variables which must be distinct from the query.
        variables: (Vec<Variable>)
    }
);

#[macro_export]
macro_rules! distinct {
    ([$($var:expr),*], $query:expr) => {
        distinct($query, vec!($(($var).clone()),*))
    }
}

impl ToCLIQueryAST for Distinct {
    fn to_ast(&self) -> String {
        format!("distinct([{}],{})",
                self.variables.to_ast(),
                self.query.to_ast()
        ).to_string()
    }
}