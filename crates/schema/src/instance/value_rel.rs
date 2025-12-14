use crate::{json::ToJson, Instance};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Hash)]
pub enum RelationValue {
    /// a reference to an entity that already exists in the database
    ExternalReference(String),
    /// a reference to an entity that already exists in the database
    ExternalReferences(Vec<String>),
    /// When inserting or replacing a document that needs to refer to another document inserted in the same operation,
    /// you can use a json dictionary of the form {"@ref": "..id.."} in place of an ordinary id.
    TransactionRef(String),
    /// When inserting or replacing a document that needs to refer to another document inserted in the same operation,
    /// you can use a json dictionary of the form {"@ref": "..id.."} in place of an ordinary id.
    TransactionRefs(Vec<String>),
    /// nested entity. be careful to not assign static ID's when nesting because the DB
    /// will reject the duplication
    One(Instance),
    More(Vec<Instance>),
}

impl RelationValue {
    pub fn is_reference(&self) -> bool {
        matches!(self, RelationValue::ExternalReference(_))
            || matches!(self, RelationValue::ExternalReferences(_))
            || matches!(self, RelationValue::TransactionRef(_))
            || matches!(self, RelationValue::TransactionRefs(_))
    }

    pub fn is_one(&self) -> bool {
        matches!(self, RelationValue::One(_))
    }

    pub fn id(&self) -> Option<String> {
        match self {
            RelationValue::ExternalReference(r) => Some(r.clone()),
            RelationValue::One(i) => i.id.clone(),
            _ => None,
        }
    }

    pub fn make_ext_ref(&mut self) -> Vec<Instance> {
        let mut new_self = self.clone();
        let mut to_ret = vec![];

        match self {
            RelationValue::ExternalReference(_) | RelationValue::ExternalReferences(_) => {
                return vec![]; // noop
            }
            RelationValue::One(nested) => {
                new_self =
                    RelationValue::ExternalReference(nested.gen_id().expect(
                        "could not derive external ID for referencing entity in transaction",
                    ));

                to_ret.push(nested.clone());
            }
            RelationValue::More(nesteds) => {
                new_self = RelationValue::ExternalReferences(nesteds.into_iter().map(|nested| {
                    nested.gen_id().expect("could not derive internal ID for referencing entity in transaction")
                }).collect::<Vec<_>>());

                to_ret = nesteds.into_iter().map(|nested| nested.clone()).collect();
            }
            RelationValue::TransactionRef(_) => {
                todo!()
            }
            RelationValue::TransactionRefs(_) => {
                todo!()
            }
        }

        *self = new_self;
        to_ret
    }

    /// after inverting the structure and pulling out the nested instances for flattened serialization when inserting documents into the DB,
    /// we should turn the nested instances into transaction references
    pub fn make_tx_ref(&mut self) -> Vec<Instance> {
        let mut new_self = self.clone();
        let mut to_ret = vec![];

        match self {
            RelationValue::TransactionRef(_) | RelationValue::TransactionRefs(_) => {
                return vec![]; // noop
            }
            RelationValue::One(nested) => {
                if !nested.is_reference() {
                    new_self = RelationValue::TransactionRef(nested.gen_id().expect(
                        "could not derive internal ID for referencing entity in transaction",
                    ));

                    to_ret.push(nested.clone());
                }
            }
            RelationValue::More(nesteds) => {
                new_self = RelationValue::TransactionRefs(nesteds.into_iter().map(|nested| {
                    nested.gen_id().expect("could not derive internal ID for referencing entity in transaction")
                }).collect::<Vec<_>>());

                to_ret = nesteds.into_iter().map(|nested| nested.clone()).collect();
            }
            RelationValue::ExternalReference(r) => {
                // new_self = RelationValue::TransactionRef(r.clone());
            }
            RelationValue::ExternalReferences(rs) => {
                // new_self = RelationValue::TransactionRefs(rs.clone());
            }
        }

        *self = new_self;
        to_ret
    }
}

impl From<Instance> for RelationValue {
    fn from(inst: Instance) -> Self {
        // todo!()
        Self::One(inst)
    }
}

impl Into<serde_json::Value> for RelationValue {
    fn into(self) -> Value {
        match self {
            RelationValue::ExternalReference(r) => Value::String(r),
            RelationValue::ExternalReferences(rs) => rs.into(),
            RelationValue::One(o) => {
                if o.is_reference() {
                    Value::String(o.id().unwrap().clone())
                } else {
                    o.set_id_capture(false).to_json()
                }
            }
            RelationValue::More(m) => m
                .into_iter()
                .map(|i| i.set_id_capture(false).to_map().into())
                .collect::<Vec<serde_json::Value>>()
                .into(),
            RelationValue::TransactionRef(r) => {
                let mut map = serde_json::Map::new();

                // class type name
                map.insert("@ref".to_string(), r.into());

                map.into()
            }
            RelationValue::TransactionRefs(rs) => {
                rs.into_iter()
                    .map(|r| {
                        let mut map = serde_json::Map::new();

                        // class type name
                        map.insert("@ref".to_string(), r.into());

                        map.into()
                    })
                    .collect::<Vec<Map<_, _>>>()
                    .into()
            }
        }
    }
}
