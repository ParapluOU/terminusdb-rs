use crate::*;

ast_struct!(
    AddLink as add_link {
        subject: NodeValue,
        predicate: NodeValue,
        object: NodeValue,
        graph: TargetGraphType
    }
);

impl ToCLIQueryAST for AddLink {
    fn to_ast(&self) -> String {
        todo!()
    }
}