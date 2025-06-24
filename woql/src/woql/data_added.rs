use crate::*;

ast_struct!(
    AddedData as added_data {
        subject: NodeValue,
        predicate: NodeValue,
        object: DataValue,
        graph: TargetGraphType
    }
);

impl ToCLIQueryAST for AddedData {
    fn to_ast(&self) -> String {
        todo!()
    }
}