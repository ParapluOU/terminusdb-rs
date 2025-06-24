use crate::*;

ast_struct!(
    AddData as add_data {
        /// A URI or variable which is the source or subject of the graph edge.
        subject: NodeValue,
        /// A URI or variable which is the edge-label or predicate of the graph edge.
        predicate: NodeValue,
        /// A data type or variable which is the target or object of the graph edge.
        object: DataValue,
        /// An optional graph (either 'instance' or 'schema')
        graph: TargetGraphType
    }
);

impl ToCLIQueryAST for AddData {
    fn to_ast(&self) -> String {
        todo!()
    }
}