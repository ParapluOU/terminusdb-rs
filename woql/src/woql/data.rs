use crate::*;

/// Specify an edge pattern which is terminal, and provides a data value association.
ast_struct!(
    Data {
        /// A URI or variable which is the source or subject of the graph edge.
        subject: NodeValue,
        /// A URI or variable which is the edge-label or predicate of the graph edge.
        predicate: NodeValue,
        /// A data type or variable which is the target or object of the graph edge.
        object: Value,
        /// An optional graph (either 'instance' or 'schema')
        graph: TargetGraphType
    }
);

impl ToCLIQueryAST for Data {
    fn to_ast(&self) -> String {
        todo!()
    }
}