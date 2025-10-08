use crate::json::InstancePropertyFromJson;
use crate::{FromInstanceProperty, FromTDBInstance, Instance, InstanceProperty, Primitive, Property, RelationValue, Schema, SetCardinality, TerminusDBModel, ToInstanceProperty, ToSchemaClass, ToSchemaProperty, ToTDBInstance, ToTDBInstances, ToTDBSchema, TypeFamily};
use anyhow::anyhow;
use serde_json::Value;
use std::collections::{BTreeSet, HashSet};
use std::hash::Hash;

impl<T: ToTDBSchema> ToTDBSchema for HashSet<T> {
    fn to_schema() -> Schema {
        T::to_schema()
    }

    fn to_schema_tree() -> Vec<Schema> {
        T::to_schema_tree()
    }

    fn to_schema_tree_mut(collection: &mut HashSet<Schema>) {
        T::to_schema_tree_mut(collection);
    }
}

impl<T: ToTDBSchema> ToTDBSchema for BTreeSet<T> {
    fn to_schema() -> Schema {
        T::to_schema()
    }

    fn to_schema_tree() -> Vec<Schema> {
        T::to_schema_tree()
    }

    fn to_schema_tree_mut(collection: &mut HashSet<Schema>) {
        T::to_schema_tree_mut(collection);
    }
}

impl<Parent, T: ToSchemaClass> ToSchemaProperty<Parent> for HashSet<T> {
    fn to_property(name: &str) -> Property {
        Property {
            name: name.to_string(),
            // todo: retrieve set cardinality as macro derive attr?
            r#type: Some(TypeFamily::Set(SetCardinality::None)),
            class: T::to_class().to_string(),
        }
    }
}

impl<Parent, T: ToSchemaClass> ToSchemaProperty<Parent> for BTreeSet<T> {
    fn to_property(name: &str) -> Property {
        Property {
            name: name.to_string(),
            // todo: retrieve set cardinality as macro derive attr?
            r#type: Some(TypeFamily::Set(SetCardinality::None)),
            class: T::to_class().to_string(),
        }
    }
}

// Implement ToInstanceProperty for Vec<T> where T implements ToTDBInstance
impl<T: Eq + ToTDBInstance + FromTDBInstance + InstancePropertyFromJson<S> + Hash, S> ToInstanceProperty<S> for HashSet<T> {
    fn to_property(self, field_name: &str, parent: &Schema) -> InstanceProperty {
        // Check if T is a subdocument type
        let is_subdocument = T::to_schema().is_subdocument();
        
        InstanceProperty::Relations(
            self.into_iter()
                .map(|item| {
                    let mut instance = item.to_instance(None);
                    // If this is a subdocument, we need to ensure it stays embedded
                    // The flatten process will check is_subdocument() and skip flattening
                    RelationValue::One(instance)
                })
                .collect(),
        )
    }
}

impl<T: Primitive+ToInstanceProperty<S>, S> ToInstanceProperty<S> for HashSet<T> {
    fn to_property(self, field_name: &str, parent: &Schema) -> InstanceProperty {
        InstanceProperty::Primitives(self.into_iter().map(|item| item.into()).collect())
    }
}

// Implement ToInstanceProperty for BTreeSet<T> where T implements ToTDBInstance
impl<T: Ord + ToTDBInstance, S> ToInstanceProperty<S> for BTreeSet<T> {
    fn to_property(self, field_name: &str, parent: &Schema) -> InstanceProperty {
        // Check if T is a subdocument type
        let is_subdocument = T::to_schema().is_subdocument();
        
        InstanceProperty::Relations(
            self.into_iter()
                .map(|item| {
                    let mut instance = item.to_instance(None);
                    // If this is a subdocument, we need to ensure it stays embedded
                    // The flatten process will check is_subdocument() and skip flattening
                    RelationValue::One(instance)
                })
                .collect(),
        )
    }
}

impl<T: Primitive + Ord, S> ToInstanceProperty<S> for BTreeSet<T> {
    fn to_property(self, field_name: &str, parent: &Schema) -> InstanceProperty {
        InstanceProperty::Primitives(self.into_iter().map(|item| item.into()).collect())
    }
}

// Implement ToInstanceProperty for HashSet<T> where T implements ToInstanceProperty
// impl<Parent, T> ToInstanceProperty<Parent> for HashSet<T>
// where
//     T: ToInstanceProperty<Parent>,
// {
//     fn to_property(self, _field_name: &str, parent: &Schema) -> InstanceProperty {
//         let values: Vec<InstanceProperty> = self
//             .into_iter()
//             .map(|item| item.to_property("", parent))
//             .collect();
//
//         // Check if all values are primitives
//         if values
//             .iter()
//             .all(|v| matches!(v, InstanceProperty::Primitive(_)))
//         {
//             let primitives: Vec<_> = values
//                 .into_iter()
//                 .filter_map(|v| match v {
//                     InstanceProperty::Primitive(prim) => Some(prim),
//                     _ => None,
//                 })
//                 .collect();
//             InstanceProperty::Primitives(primitives)
//         } else {
//             InstanceProperty::Any(values)
//         }
//     }
// }

// === FromInstanceProperty implementations ===

impl<T> FromInstanceProperty for HashSet<T>
where
    T: FromInstanceProperty + Eq + std::hash::Hash,
{
    fn from_property(prop: &InstanceProperty) -> anyhow::Result<Self> {
        match prop {
            InstanceProperty::Primitives(primitives) => {
                let mut set = HashSet::new();
                for prim in primitives {
                    let prim_prop = InstanceProperty::Primitive(prim.clone());
                    let item = T::from_property(&prim_prop)?;
                    set.insert(item);
                }
                Ok(set)
            }
            InstanceProperty::Relations(relations) => {
                let mut set = HashSet::new();
                for rel in relations {
                    match rel {
                        RelationValue::One(_instance) => {
                            let rel_prop = InstanceProperty::Relation(rel.clone());
                            let item = T::from_property(&rel_prop)?;
                            set.insert(item);
                        }
                        _ => return Err(anyhow::anyhow!("Unsupported relation type for HashSet")),
                    }
                }
                Ok(set)
            }
            InstanceProperty::Any(any_values) => {
                let mut set = HashSet::new();
                for value in any_values {
                    let item = T::from_property(value)?;
                    set.insert(item);
                }
                Ok(set)
            }
            _ => Err(anyhow::anyhow!(
                "Expected Primitives, Relations, or Any for HashSet, got {:?}",
                prop
            )),
        }
    }
}

impl<T> FromInstanceProperty for BTreeSet<T>
where
    T: FromInstanceProperty + Ord,
{
    fn from_property(prop: &InstanceProperty) -> anyhow::Result<Self> {
        match prop {
            InstanceProperty::Primitives(primitives) => {
                let mut set = BTreeSet::new();
                for prim in primitives {
                    let prim_prop = InstanceProperty::Primitive(prim.clone());
                    let item = T::from_property(&prim_prop)?;
                    set.insert(item);
                }
                Ok(set)
            }
            InstanceProperty::Relations(relations) => {
                let mut set = BTreeSet::new();
                for rel in relations {
                    match rel {
                        RelationValue::One(_instance) => {
                            let rel_prop = InstanceProperty::Relation(rel.clone());
                            let item = T::from_property(&rel_prop)?;
                            set.insert(item);
                        }
                        _ => return Err(anyhow::anyhow!("Unsupported relation type for BTreeSet")),
                    }
                }
                Ok(set)
            }
            InstanceProperty::Any(any_values) => {
                let mut set = BTreeSet::new();
                for value in any_values {
                    let item = T::from_property(value)?;
                    set.insert(item);
                }
                Ok(set)
            }
            _ => Err(anyhow::anyhow!(
                "Expected Primitives, Relations, or Any for BTreeSet, got {:?}",
                prop
            )),
        }
    }
}

// // === InstancePropertyFromJson implementations ===
//
// impl<Parent, T> InstancePropertyFromJson<Parent> for HashSet<T>
// where
//     T: Primitive + InstancePropertyFromJson<Parent> + Eq + std::hash::Hash,
// {
//     fn property_from_json(json: Value) -> anyhow::Result<InstanceProperty> {
//         match json {
//             Value::Array(arr) => {
//                 Ok(InstanceProperty::Primitives({
//                     arr.into_iter().map(|item| {
//                         Ok(match T::property_from_json(item)? {
//                             // possibly URI's
//                             InstanceProperty::Primitive(prim) => {
//                                 prim
//                             },
//                             v => {
//                                 return Err(anyhow::anyhow!(
//                                     "Expected primitive values in HashSet JSON array, but got {:#?}", v
//                                 ))
//                             }
//                         })
//                     }).collect::<anyhow::Result<_>>()?
//
//                     // let mut primitive_values = Vec::new();
//                     // for item in arr {
//                     //     let item_prop = T::property_from_json(item)?;
//                     //     match item_prop {
//                     //         // possibly URI's
//                     //         InstanceProperty::Primitive(prim) => {
//                     //             primitive_values.push(prim);
//                     //         }
//                     //         v => {
//                     //             return Err(anyhow::anyhow!(
//                     //             "Expected primitive values in HashSet JSON array, but got {:#?}", v
//                     //         ))
//                     //         }
//                     //     }
//                     // }
//                     // primitive_values
//                 }))
//             }
//             _ => Err(anyhow::anyhow!("Expected primitive array for HashSet")),
//         }
//     }
// }

// impl<Parent, T> ToInstanceProperty<Parent> for HashSet<T>
// where
//     T: Eq + FromTDBInstance + InstancePropertyFromJson<Parent> + std::hash::Hash,
// {
//     fn to_property(self, field_name: &str, parent: &Schema) -> InstanceProperty {
//         todo!()
//     }
// }

// impl<Parent, T> InstancePropertyFromJson<Parent> for HashSet<T>
// where
//     T: FromTDBInstance + InstancePropertyFromJson<Parent> + Eq + std::hash::Hash,
// {
//     fn property_from_json(json: Value) -> anyhow::Result<InstanceProperty> {
//         match json {
//             Value::Array(arr) => {
//                 Ok(InstanceProperty::Relations({
//                     arr.into_iter().map(|item| {
//                         Ok(match T::property_from_json(item)? {
//                             // possibly URI's
//                             InstanceProperty::Relation(rel) => {
//                                 rel
//                             },
//                             v => {
//                                 return Err(anyhow::anyhow!(
//                                     "Expected relation values in HashSet JSON array, but got {:#?}", v
//                                 ))
//                             }
//                         })
//                     }).collect::<anyhow::Result<_>>()?
//
//                 }))
//             }
//             _ => Err(anyhow::anyhow!("Expected relation/URI array for HashSet")),
//         }
//     }
// }

impl<T, Parent> InstancePropertyFromJson<Parent> for HashSet<T>
where
    T: InstancePropertyFromJson<Parent>,
    HashSet<T>: ToInstanceProperty<Parent>,
{
    fn property_from_json(json: Value) -> anyhow::Result<InstanceProperty> {
        match json {
            Value::Array(array) => {
                // For each element in the array, convert to InstanceProperty
                let mut properties = Vec::with_capacity(array.len());

                for value in array {
                    let prop = T::property_from_json(value)?;
                    properties.push(prop);
                }

                // Determine what kind of container to use based on the first element
                if properties.is_empty() {
                    Ok(InstanceProperty::Primitives(Vec::new()))
                } else if properties[0].is_primitive() {
                    // Convert all to primitives
                    let primitives = properties
                        .into_iter()
                        .map(|p| {
                            if let InstanceProperty::Primitive(pv) = p {
                                Ok(pv)
                            } else {
                                Err(anyhow!(
                                    "Expected all array elements to be primitive values"
                                ))
                            }
                        })
                        .collect::<anyhow::Result<Vec<_>>>()?;

                    Ok(InstanceProperty::Primitives(primitives))
                } else if properties[0].is_relation() {
                    // Convert all to relations
                    let relations = properties
                        .into_iter()
                        .map(|p| {
                            if let InstanceProperty::Relation(rv) = p {
                                Ok(rv)
                            } else {
                                Err(anyhow!("Expected all array elements to be relations"))
                            }
                        })
                        .collect::<anyhow::Result<Vec<_>>>()?;

                    Ok(InstanceProperty::Relations(relations))
                } else {
                    // Mixed or other types
                    Ok(InstanceProperty::Any(properties))
                }
            }
            _ => Err(anyhow!("Expected an array, got {:?}", json)),
        }
    }
}

impl<Parent, T> InstancePropertyFromJson<Parent> for BTreeSet<T>
where
    T: InstancePropertyFromJson<Parent> + Ord + ToInstanceProperty<T>,
    BTreeSet<T>: ToInstanceProperty<Parent>,
{
    fn property_from_json(json: Value) -> anyhow::Result<InstanceProperty> {
        match json {
            Value::Array(arr) => {
                let mut primitive_values = Vec::new();
                for item in arr {
                    let item_prop = T::property_from_json(item)?;
                    match item_prop {
                        InstanceProperty::Primitive(prim) => {
                            primitive_values.push(prim);
                        }
                        _ => {
                            return Err(anyhow::anyhow!(
                                "Expected primitive values in BTreeSet JSON array"
                            ))
                        }
                    }
                }
                Ok(InstanceProperty::Primitives(primitive_values))
            }
            _ => Err(anyhow::anyhow!("Expected JSON array for BTreeSet")),
        }
    }
}

// === ToTDBInstances implementations ===

impl<I: ToTDBInstance> ToTDBInstances for HashSet<I> {
    fn to_instance_tree(&self) -> Vec<Instance> {
        self.iter()
            .map(|v| v.to_instance_tree())
            .flatten()
            .collect()
    }
}

impl<I: ToTDBInstance> ToTDBInstances for BTreeSet<I> {
    fn to_instance_tree(&self) -> Vec<Instance> {
        self.iter()
            .map(|v| v.to_instance_tree())
            .flatten()
            .collect()
    }
}
