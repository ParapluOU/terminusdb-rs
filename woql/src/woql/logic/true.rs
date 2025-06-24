use crate::*;

#[derive(Clone, Copy)]
pub struct True;

pub fn r#true() -> True {
    True {}
}

impl ToCLIQueryAST for True {
    fn to_ast(&self) -> String {
        "true".to_string()
    }
}