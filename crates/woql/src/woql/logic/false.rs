use crate::*;

#[derive(Clone, Copy)]
pub struct False;

pub fn r#false() -> False {
    False {}
}

impl ToCLIQueryAST for False {
    fn to_ast(&self) -> String {
        "false".to_string()
    }
}