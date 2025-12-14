use crate::*;

/// Select specific variables from a query to return.
ast_struct!(
    Select as select {
        /// The query which will be run prior to selection.
        query: Query,

        /// The variables to select from the query.
        variables: (Vec<Variable>)
    }
);


// #[derive(Clone, Debug, TerminusDBSchema)]
// pub struct Select {
//     /// The query which will be run prior to selection.
//     pub query: Query,
//     /// The variables to select from the query.
//     pub variables: Vec<Variable>,
// }
//
// // todo: generate from struct
// pub fn select(variables: Vec<impl std::convert::Into<Variable>>, query: impl std::convert::Into<Query>) -> Select {
//     Select {
//         query: query.into(),
//         variables: variables.into_iter().map(std::convert::Into::into).collect(),
//     }
// }

#[macro_export]
macro_rules! select {
    ([$($var:expr),*], $query:expr) => {
        select($query, vec!($(($var).clone()),*))
    };
}

// #[test]
// fn test_select() {
//     let var = var!( V );
//     select!(|var| t!(var, pred!( filehash ), var));
// }

impl ToCLIQueryAST for Select {
    fn to_ast(&self) -> String {
        format!("select([{}],{})",
                self.variables.to_ast(),
                self.query.to_ast()
        ).to_string()
    }
}