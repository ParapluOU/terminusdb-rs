use crate::*;

impl ToCLIQueryAST for Using {
    fn to_ast(&self) -> String {
        todo!()
    }
}

/// Select a specific collection for query.
ast_struct!(
    Using as using {

        ///The resource over which to run the query."
        collection: NodeURI,

        ///The query which will be run on the selected collection."
        query: Query
    }
);

#[macro_export]
macro_rules! using {
    ($coll:expr, $query:expr) => {
        using($coll, $query)
    };
}
