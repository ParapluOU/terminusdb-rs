use crate::*;

ast_struct!(
    AddedLink as added_link {
        subject: NodeValue,
        predicate: NodeValue,
        object: NodeValue,
        graph: TargetGraphType
    }
);

impl ToCLIQueryAST for AddedLink {
    fn to_ast(&self) -> String {
        todo!()
    }
}