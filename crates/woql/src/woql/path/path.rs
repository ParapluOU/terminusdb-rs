use crate::*;

/// Find a path through the graph according to 'pattern'. This 'pattern' is a regular graph expression which avoids cycles.
ast_struct!(
    Path as path {
        /// The starting node.
        subject: Value,
        /// The pattern which describes how to traverse edges.
        pattern: PathPattern,
        /// The ending node.
        object: Value,
        /// An optional list of edges traversed.
        path: (Option<Value>)
    }
);

impl ToCLIQueryAST for Path {
    fn to_ast(&self) -> String {
        todo!()
    }
}