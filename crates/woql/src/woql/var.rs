use std::marker::PhantomData;
use serde::de::DeserializeOwned;
use crate::*;

#[macro_export]
macro_rules! var {
    ($name:ident) => {
        &var(stringify!($name))
    }
}

newtype!({
    name: Variable,
    type: String,
    schemaclass: STRING
});

#[derive(Clone, Debug)]
pub struct TypedVariable<T: DeserializeOwned> {
    var: Variable,
    r#type: PhantomData<T>
}

impl <T: DeserializeOwned> std::convert::From<Variable> for TypedVariable<T> {
    fn from(var: Variable) -> Self {
        Self {
            var, r#type: Default::default()
        }
    }
}

impl <T: DeserializeOwned> std::convert::From<&Variable> for TypedVariable<T> {
    fn from(var: &Variable) -> Self {
        (*var).clone().into()
    }
}

impl <T: DeserializeOwned> std::convert::Into<Variable> for &TypedVariable<T> {
    fn into(self) -> Variable {
        self.var.clone()
    }
}

impl <T: DeserializeOwned> AsRef<Variable> for TypedVariable<T> {
    fn as_ref(&self) -> &Variable {
        &self.var
    }
}

impl ToMaybeTDBSchema for Variable {}

impl ToString for Variable {
    fn to_string(&self) -> String {
        self.0.clone()
    }
}

impl <T: DeserializeOwned> ToString for TypedVariable<T> {
    fn to_string(&self) -> String {
        self.var.to_string()
    }
}

impl std::convert::Into<Variable> for &Variable {
    fn into(self) -> Variable {
        self.clone()
    }
}

pub fn var(name: impl AsRef<str>) -> Variable {
    Variable(name.as_ref().to_string())
}

impl ToCLIQueryAST for Variable {
    fn to_ast(&self) -> String {
        self.0.clone()
    }
}

impl ToRESTQuery for Variable {
    fn to_rest_query_json(&self) -> serde_json::Value {
        self.0.clone().into()
    }
}