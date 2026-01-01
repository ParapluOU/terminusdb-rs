use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::File;
use std::io::Write;

use crate::*;

// fn serialize_docs(docs: impl Iterator<Item=&Document>) -> String {
//     serde_json::to_string(
//         &serde_json::Value::Array(
//             docs.map(|s| s.to_json()).collect::<Vec<_>>()
//         )
//     ).expect("json parse fail")
// }

pub trait ToMaybeSchemaDocuments: ToMaybeTDBSchema {
    fn to_schema_tree_documents() -> Documents {
        Self::to_schema_tree().into()
    }
}

pub trait ToSchemaDocuments: ToTDBSchema {
    fn to_schema_tree_documents() -> Documents {
        Self::to_schema_tree().into()
    }
}

pub trait ToTDBInstanceDocument: ToTDBInstance {
    fn to_instance_document(&self, id: Option<String>) -> Document {
        self.to_instance(id).into()
    }
}

impl<T: ToMaybeTDBSchema> ToMaybeSchemaDocuments for T {}
impl<T: ToTDBSchema> ToSchemaDocuments for T {}
impl<T: ToTDBInstance> ToTDBInstanceDocument for T {}

#[derive(Clone)]
pub enum Document {
    Schema(Schema),
    Instance(Instance),
}

impl Document {
    pub fn graph_type(&self) -> GraphType {
        (match self {
            Document::Schema(_) => GraphType::Schema,
            Document::Instance(_) => GraphType::Instance,
        })
    }

    pub fn to_json(self) -> serde_json::Value {
        match self {
            Document::Schema(schema) => schema.to_json(),
            Document::Instance(inst) => serde_json::Value::Object(inst.to_map()),
        }
    }

    pub fn to_json_string(self) -> String {
        serde_json::to_string(&self.to_json()).expect("json parse fail")
    }
}

impl From<Schema> for Document {
    fn from(schema: Schema) -> Self {
        Self::Schema(schema)
    }
}

impl From<Instance> for Document {
    fn from(inst: Instance) -> Self {
        Self::Instance(inst)
    }
}

#[derive(Clone)]
pub enum Documents {
    Schema(HashSet<Schema>),
    Instance(Vec<Instance>),
}

impl From<Document> for Documents {
    fn from(doc: Document) -> Self {
        match doc {
            Document::Schema(schema) => Self::Schema(vec![schema].into_iter().collect()),
            Document::Instance(inst) => Self::Instance(vec![inst]),
        }
    }
}

impl From<Vec<Schema>> for Documents {
    fn from(schema: Vec<Schema>) -> Self {
        Documents::Schema(schema.into_iter().collect())
    }
}

impl Documents {
    pub fn graph_type(&self) -> GraphType {
        match self {
            Documents::Schema(schemas) => GraphType::Schema,
            Documents::Instance(instances) => GraphType::Instance,
        }
    }

    pub fn to_json(self, ctx: &Option<Context>) -> serde_json::Value {
        serde_json::Value::Array(match self {
            // todo: deduplicate
            Documents::Schema(values) => {
                let mut v = vec![];
                if let Some(context) = ctx {
                    v.push(context.to_json());
                }
                v.extend(values.into_iter().map(|s| s.to_json()));
                // schemas.into_iter().map(|s| s.to_json()).collect::<Vec<_>>()
                v
            }
            Documents::Instance(values) => {
                let mut v = vec![];
                if let Some(context) = ctx {
                    v.push(context.to_json());
                }
                v.extend(values.into_iter().map(|s| s.to_json()));
                // schemas.into_iter().map(|s| s.to_json()).collect::<Vec<_>>()
                v
            }
        })
    }

    pub fn to_json_str(self, ctx: &Option<Context>) -> String {
        serde_json::to_string(&self.to_json(ctx)).expect("json parse fail")
    }

    pub fn to_file(&self, path: &str) -> std::io::Result<String> {
        let path = format!("{}.schema.json", path);

        match self {
            Documents::Schema(schemas) => {
                let mut output = File::create(&path)?;
                let line = schemas.iter().map(|s| s.to_string()).join(", ");
                write!(output, "[{}]", line)?;
                Ok(path)
            }
            Documents::Instance(_) => {
                unimplemented!()
            }
        }
    }
}
