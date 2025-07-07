use std::collections::{BTreeSet, HashSet};
use serde_json::Value;
use anyhow::anyhow;
use crate::{FromInstanceProperty, InstanceProperty, Property, Schema, SetCardinality, ToInstanceProperty, ToSchemaClass, ToSchemaProperty, TypeFamily, RelationValue, ToTDBSchema};
use crate::json::InstancePropertyFromJson;

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

// Implement ToInstanceProperty for HashSet<T> where T implements ToInstanceProperty
impl<Parent, T> ToInstanceProperty<Parent> for HashSet<T> 
where 
    T: ToInstanceProperty<Parent>
{
    fn to_property(self, _field_name: &str, parent: &Schema) -> InstanceProperty {
        let values: Vec<InstanceProperty> = self
            .into_iter()
            .map(|item| item.to_property("", parent))
            .collect();
        
        // Check if all values are primitives
        if values.iter().all(|v| matches!(v, InstanceProperty::Primitive(_))) {
            let primitives: Vec<_> = values
                .into_iter()
                .filter_map(|v| match v {
                    InstanceProperty::Primitive(prim) => Some(prim),
                    _ => None,
                })
                .collect();
            InstanceProperty::Primitives(primitives)
        } else {
            InstanceProperty::Any(values)
        }
    }
}


// === FromInstanceProperty implementations ===

impl<T> FromInstanceProperty for HashSet<T> 
where 
    T: FromInstanceProperty + Eq + std::hash::Hash
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
            _ => Err(anyhow::anyhow!("Expected Primitives, Relations, or Any for HashSet, got {:?}", prop)),
        }
    }
}

impl<T> FromInstanceProperty for BTreeSet<T> 
where 
    T: FromInstanceProperty + Ord
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
            _ => Err(anyhow::anyhow!("Expected Primitives, Relations, or Any for BTreeSet, got {:?}", prop)),
        }
    }
}

// === InstancePropertyFromJson implementations ===

impl<Parent, T> InstancePropertyFromJson<Parent> for HashSet<T> 
where 
    T: InstancePropertyFromJson<Parent> + Eq + std::hash::Hash
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
                        _ => return Err(anyhow::anyhow!("Expected primitive values in HashSet JSON array")),
                    }
                }
                Ok(InstanceProperty::Primitives(primitive_values))
            }
            _ => Err(anyhow::anyhow!("Expected JSON array for HashSet")),
        }
    }
}

impl<Parent, T> InstancePropertyFromJson<Parent> for BTreeSet<T> 
where 
    T: InstancePropertyFromJson<Parent> + Ord + ToInstanceProperty<T>,
    BTreeSet<T>: ToInstanceProperty<Parent>
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
                        _ => return Err(anyhow::anyhow!("Expected primitive values in BTreeSet JSON array")),
                    }
                }
                Ok(InstanceProperty::Primitives(primitive_values))
            }
            _ => Err(anyhow::anyhow!("Expected JSON array for BTreeSet")),
        }
    }
}