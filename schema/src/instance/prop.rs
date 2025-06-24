use crate::{
    json::ToJson, FromTDBInstance, Property, Schema, ToTDBInstance, ToTDBInstances, ToTDBSchema,
};
use crate::{Instance, Key, PrimitiveValue, RelationValue};
use anyhow::Context;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::any;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::marker::PhantomData;
use std::sync::Arc;
use uuid::Uuid;

// trait for converting any object into a Instance document property, globally
pub trait ToInstanceProperty<Parent> {
    fn to_property(self, field_name: &str, parent: &Schema) -> InstanceProperty;
}

pub trait FromInstanceProperty: Sized {
    fn from_property(prop: &InstanceProperty) -> anyhow::Result<Self>;

    fn from_maybe_property(prop: &Option<InstanceProperty>) -> anyhow::Result<Self> {
        if let Some(ip) = prop {
            return Self::from_property(&ip);
        }
        panic!(
            "expected property not found for {}",
            std::any::type_name::<Self>()
        )
    }
}

/// blanket impl for all fields of types that are models
impl<T: FromTDBInstance + ToTDBInstance> FromInstanceProperty for T {
    default fn from_property(prop: &InstanceProperty) -> anyhow::Result<Self> {
        match prop {
            InstanceProperty::Relation(RelationValue::One(instance)) => T::from_instance(instance),

            InstanceProperty::Primitive(v) => {
                todo!("convert {:#?} to {}", v, any::type_name::<T>())
            }
            InstanceProperty::Primitives(v) => {
                todo!("convert {:#?} to {}", v, any::type_name::<T>())
            }
            InstanceProperty::Relations(v) => {
                todo!("convert {:#?} to {}", v, any::type_name::<T>())
            }
            InstanceProperty::Any(v) => {
                todo!("convert {:#?} to {}", v, any::type_name::<T>())
            }
            InstanceProperty::Relation(RelationValue::ExternalReference(_)) => {
                todo!()
            }
            InstanceProperty::Relation(RelationValue::ExternalReferences(_)) => {
                todo!()
            }
            InstanceProperty::Relation(RelationValue::TransactionRef(_)) => {
                unimplemented!()
            }
            InstanceProperty::Relation(RelationValue::TransactionRefs(_)) => {
                unimplemented!()
            }
            InstanceProperty::Relation(RelationValue::More(v)) => {
                todo!("convert {:#?} to {}", v, any::type_name::<T>())
            }
        }
    }
}

impl<T: FromInstanceProperty> FromInstanceProperty for Option<T> {
    default fn from_property(prop: &InstanceProperty) -> anyhow::Result<Self> {
        match prop {
            InstanceProperty::Primitive(PrimitiveValue::Null) => Ok(None),
            _ => T::from_property(prop).map(Some),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Hash)]
pub enum InstanceProperty {
    Primitive(PrimitiveValue),
    Primitives(Vec<PrimitiveValue>),
    Relation(RelationValue),
    Relations(Vec<RelationValue>),
    Any(Vec<InstanceProperty>),
}

impl InstanceProperty {
    pub fn is_primitive(&self) -> bool {
        matches!(self, InstanceProperty::Primitive(_))
    }

    /// Flattens nested relations to references by extracting their IDs.
    /// This is useful when saving instances to TerminusDB to avoid duplicate saves
    /// of nested instances that are already in the database.
    /// Returns a Vec of all instances that were removed from nesting.
    pub fn flatten(&mut self, for_transaction: bool) -> Vec<Instance> {
        let mut removed = Vec::new();
        match self {
            InstanceProperty::Relation(rel) => {
                if let RelationValue::One(inst) = rel {
                    if let Some(id) = inst.id.clone() {
                        removed.push(inst.clone());
                        *rel = RelationValue::ExternalReference(id);

                        if for_transaction {
                            rel.make_tx_ref();
                        }
                    }
                }
            }
            InstanceProperty::Relations(rels) => {
                for rel in rels.iter_mut() {
                    if let RelationValue::One(inst) = rel {
                        if let Some(id) = inst.id.clone() {
                            removed.push(inst.clone());
                            *rel = RelationValue::ExternalReference(id);

                            if for_transaction {
                                rel.make_tx_ref();
                            }
                        }
                    }
                }
            }
            InstanceProperty::Any(any) => {
                for prop in any.iter_mut() {
                    removed.extend(prop.flatten(for_transaction));
                }
            }
            _ => {}
        }
        removed
    }

    pub fn is_null(&self) -> bool {
        matches!(self, InstanceProperty::Primitive(PrimitiveValue::Null))
    }

    pub fn is_relation(&self) -> bool {
        self.is_any_relation()
            || matches!(
                self,
                InstanceProperty::Relation(_) | InstanceProperty::Relations(_)
            )
    }

    pub fn is_any_relation(&self) -> bool {
        match self {
            InstanceProperty::Any(any) => any.iter().any(|p| p.is_relation()),
            _ => false,
        }
    }

    pub fn is_relations(&self) -> bool {
        self.is_any_relation() || matches!(self, InstanceProperty::Relations(_))
    }

    pub fn relation(&self) -> Option<&RelationValue> {
        match self {
            InstanceProperty::Relation(r) => Some(r),
            _ => None,
        }
    }

    pub fn relations(&self) -> Option<Vec<&RelationValue>> {
        match self {
            InstanceProperty::Relations(r) => Some(r.iter().collect()),
            InstanceProperty::Any(r) => {
                if r.is_empty() || !r[0].is_relation() {
                    return None;
                }

                r.iter()
                    .map(|v| v.relation().unwrap())
                    .collect::<Vec<_>>()
                    .into()
            }
            _ => None,
        }
    }

    pub fn is_reference(&self) -> bool {
        self.relation().map_or(false, |r| r.is_reference())
            || self
                .relations()
                .map_or(false, |r| r.iter().any(|v| v.is_reference()))
    }

    // pub fn as_reference(&self) -> Option<serde_json::Value> {
    //     self.relation().and_then(|r| r.clone().make_tx_ref())
    // }

    // pub fn as_references(&self) -> Option<Vec<serde_json::Value>> {
    //     self.relations().and_then(|r| {
    //         Some(
    //             r.clone()
    //                 .into_iter()
    //                 .map(|rv| rv.clone().make_tx_ref())
    //                 .collect(),
    //         )
    //     })
    // }

    pub fn as_id(&self) -> Option<String> {
        self.relation().and_then(|r| match r {
            RelationValue::ExternalReference(r) => Some(r.clone()),
            RelationValue::One(i) => i.id.clone(),
            _ => None,
        })
    }

    pub fn as_ids(&self) -> Option<Vec<String>> {
        self.relations()
            .and_then(|r| Some(r.iter().map(|v| v.id().unwrap()).collect()))
    }
}

// impl From<Instance> for InstanceProperty {
//     fn from(inst: Instance) -> Self {
//         Self::Relation(inst.into())
//     }
// }

impl<T: Into<Instance>> From<T> for InstanceProperty {
    fn from(t: T) -> Self {
        let inst: Instance = t.into();
        Self::Relation(inst.into())
    }
}

impl Into<serde_json::Value> for InstanceProperty {
    fn into(self) -> Value {
        match self {
            InstanceProperty::Primitive(p) => p.into(),
            InstanceProperty::Relation(r) => r.into(),
            InstanceProperty::Primitives(arr) => arr
                .into_iter()
                .map(Into::into)
                .collect::<Vec<serde_json::Value>>()
                .into(),
            InstanceProperty::Relations(rels) => rels
                .into_iter()
                .map(Into::into)
                .collect::<Vec<serde_json::Value>>()
                .into(),
            InstanceProperty::Any(objs) => objs
                .into_iter()
                .map(Into::into)
                .collect::<Vec<serde_json::Value>>()
                .into(),
        }
    }
}

// Helper function to deserialize a JSON value to an InstanceProperty
// todo: this has to be derived per model
pub fn deserialize_property(value: Value) -> anyhow::Result<InstanceProperty> {
    match value {
        Value::Null => Ok(InstanceProperty::Primitive(PrimitiveValue::Null)),
        Value::Bool(b) => Ok(InstanceProperty::Primitive(PrimitiveValue::Bool(b))),
        Value::Number(n) => Ok(InstanceProperty::Primitive(PrimitiveValue::Number(n))),
        Value::String(s) => Ok(InstanceProperty::Primitive(PrimitiveValue::String(s))),
        Value::Array(arr) => {
            if arr.is_empty() {
                Ok(InstanceProperty::Primitive(PrimitiveValue::Unit))
            } else {
                // Check the type of the first element to determine if it's primitives or relations
                match &arr[0] {
                    Value::Object(obj) if obj.contains_key("@type") || obj.contains_key("@ref") => {
                        // This is likely a list of relations
                        let mut relations = Vec::new();
                        for item in arr {
                            if let Value::Object(obj) = &item {
                                if obj.contains_key("@ref") {
                                    if let Some(Value::String(ref_id)) = obj.get("@ref") {
                                        relations
                                            .push(RelationValue::ExternalReference(ref_id.clone()));
                                    }
                                } else if obj.contains_key("@type") {
                                    todo!()
                                    // This is a nested instance
                                    // let instance = Instance::from_json(item)?;
                                    // relations.push(RelationValue::One(instance));
                                }
                            }
                        }
                        Ok(InstanceProperty::Relations(relations))
                    }
                    _ => {
                        // This is a list of primitives
                        let mut primitives = Vec::new();
                        for item in arr {
                            match item {
                                Value::Null => primitives.push(PrimitiveValue::Null),
                                Value::Bool(b) => primitives.push(PrimitiveValue::Bool(b)),
                                Value::Number(n) => primitives.push(PrimitiveValue::Number(n)),
                                Value::String(s) => primitives.push(PrimitiveValue::String(s)),
                                Value::Object(obj) => {
                                    primitives.push(PrimitiveValue::Object(Value::Object(obj)))
                                }
                                Value::Array(_) => primitives.push(PrimitiveValue::Object(item)),
                            }
                        }
                        Ok(InstanceProperty::Primitives(primitives))
                    }
                }
            }
        }
        Value::Object(obj) => {
            if obj.contains_key("@ref") {
                // This is a reference to another instance
                if let Some(Value::String(ref_id)) = obj.get("@ref") {
                    Ok(InstanceProperty::Relation(
                        RelationValue::ExternalReference(ref_id.clone()),
                    ))
                } else {
                    Err(anyhow::anyhow!("Invalid @ref value"))
                }
            } else if obj.contains_key("@type") {
                todo!()
                // This is a nested instance
                // let instance = Instance::from_json_with_schema::<T>(Value::Object(obj))?;
                // Ok(InstanceProperty::Relation(RelationValue::One(instance)))
            } else {
                // This is a generic object
                Ok(InstanceProperty::Primitive(PrimitiveValue::Object(
                    Value::Object(obj),
                )))
            }
        }
    }
}

// Basic implementations for primitive types
impl FromInstanceProperty for String {
    fn from_property(prop: &InstanceProperty) -> anyhow::Result<Self> {
        match prop {
            InstanceProperty::Primitive(PrimitiveValue::String(s)) => Ok(s.clone()),
            _ => Err(anyhow::anyhow!("Expected String primitive, got {:?}", prop)),
        }
    }
}

impl FromInstanceProperty for bool {
    fn from_property(prop: &InstanceProperty) -> anyhow::Result<Self> {
        match prop {
            InstanceProperty::Primitive(PrimitiveValue::Bool(b)) => Ok(*b),
            _ => Err(anyhow::anyhow!(
                "Expected Boolean primitive, got {:?}",
                prop
            )),
        }
    }
}

impl FromInstanceProperty for i32 {
    fn from_property(prop: &InstanceProperty) -> anyhow::Result<Self> {
        match prop {
            InstanceProperty::Primitive(PrimitiveValue::Number(n)) => n
                .as_i64()
                .and_then(|i| i32::try_from(i).ok())
                .ok_or_else(|| anyhow::anyhow!("Number cannot be converted to i32")),
            _ => Err(anyhow::anyhow!("Expected Number primitive, got {:?}", prop)),
        }
    }
}

impl FromInstanceProperty for i64 {
    fn from_property(prop: &InstanceProperty) -> anyhow::Result<Self> {
        match prop {
            InstanceProperty::Primitive(PrimitiveValue::Number(n)) => n
                .as_i64()
                .ok_or_else(|| anyhow::anyhow!("Number cannot be converted to i64")),
            _ => Err(anyhow::anyhow!("Expected Number primitive, got {:?}", prop)),
        }
    }
}

impl FromInstanceProperty for u32 {
    fn from_property(prop: &InstanceProperty) -> anyhow::Result<Self> {
        match prop {
            InstanceProperty::Primitive(PrimitiveValue::Number(n)) => n
                .as_u64()
                .and_then(|i| u32::try_from(i).ok())
                .ok_or_else(|| anyhow::anyhow!("Number cannot be converted to u32")),
            _ => Err(anyhow::anyhow!("Expected Number primitive, got {:?}", prop)),
        }
    }
}

impl FromInstanceProperty for u64 {
    fn from_property(prop: &InstanceProperty) -> anyhow::Result<Self> {
        match prop {
            InstanceProperty::Primitive(PrimitiveValue::Number(n)) => n
                .as_u64()
                .ok_or_else(|| anyhow::anyhow!("Number cannot be converted to u64")),
            _ => Err(anyhow::anyhow!("Expected Number primitive, got {:?}", prop)),
        }
    }
}

impl FromInstanceProperty for usize {
    fn from_property(prop: &InstanceProperty) -> anyhow::Result<Self> {
        match prop {
            InstanceProperty::Primitive(PrimitiveValue::Number(n)) => n
                .as_u64()
                .and_then(|i| usize::try_from(i).ok())
                .ok_or_else(|| anyhow::anyhow!("Number cannot be converted to usize")),
            _ => Err(anyhow::anyhow!("Expected Number primitive, got {:?}", prop)),
        }
    }
}

impl FromInstanceProperty for isize {
    fn from_property(prop: &InstanceProperty) -> anyhow::Result<Self> {
        match prop {
            InstanceProperty::Primitive(PrimitiveValue::Number(n)) => n
                .as_i64()
                .and_then(|i| isize::try_from(i).ok())
                .ok_or_else(|| anyhow::anyhow!("Number cannot be converted to isize")),
            _ => Err(anyhow::anyhow!("Expected Number primitive, got {:?}", prop)),
        }
    }
}

impl FromInstanceProperty for u8 {
    fn from_property(prop: &InstanceProperty) -> anyhow::Result<Self> {
        match prop {
            InstanceProperty::Primitive(PrimitiveValue::Number(n)) => n
                .as_u64()
                .and_then(|i| u8::try_from(i).ok())
                .ok_or_else(|| anyhow::anyhow!("Number cannot be converted to u8")),
            _ => Err(anyhow::anyhow!("Expected Number primitive, got {:?}", prop)),
        }
    }
}

impl FromInstanceProperty for i8 {
    fn from_property(prop: &InstanceProperty) -> anyhow::Result<Self> {
        match prop {
            InstanceProperty::Primitive(PrimitiveValue::Number(n)) => n
                .as_i64()
                .and_then(|i| i8::try_from(i).ok())
                .ok_or_else(|| anyhow::anyhow!("Number cannot be converted to i8")),
            _ => Err(anyhow::anyhow!("Expected Number primitive, got {:?}", prop)),
        }
    }
}

impl FromInstanceProperty for f64 {
    fn from_property(prop: &InstanceProperty) -> anyhow::Result<Self> {
        match prop {
            InstanceProperty::Primitive(PrimitiveValue::Number(n)) => n
                .as_f64()
                .ok_or_else(|| anyhow::anyhow!("Number cannot be converted to f64")),
            _ => Err(anyhow::anyhow!("Expected Number primitive, got {:?}", prop)),
        }
    }
}

impl FromInstanceProperty for f32 {
    fn from_property(prop: &InstanceProperty) -> anyhow::Result<Self> {
        match prop {
            InstanceProperty::Primitive(PrimitiveValue::Number(n)) => n
                .as_f64()
                .map(|f| f as f32)
                .ok_or_else(|| anyhow::anyhow!("Number cannot be converted to f32")),
            _ => Err(anyhow::anyhow!("Expected Number primitive, got {:?}", prop)),
        }
    }
}

// Implementation for Option<T>
// impl<T: FromInstanceProperty> FromInstanceProperty for Option<T> {
//     fn from_property(prop: &InstanceProperty) -> anyhow::Result<Self> {
//         match prop {
//             InstanceProperty::Primitive(PrimitiveValue::Null) => Ok(None),
//             _ => T::from_property(prop).map(Some)
//         }
//     }
//
//     fn from_maybe_property(prop: &Option<InstanceProperty>) -> anyhow::Result<Self> {
//         match prop {
//             None | Some(InstanceProperty::Primitive(PrimitiveValue::Null)) => Ok(None),
//             Some(other) => Ok(T::from_property(other).ok())
//         }
//     }
// }
//
// // Implementation for Vec<T>
// impl<T: FromInstanceProperty> FromInstanceProperty for Vec<T> {
//     fn from_property(prop: &InstanceProperty) -> anyhow::Result<Self> {
//         match prop {
//             InstanceProperty::Primitives(primitives) => {
//                 let mut result = Vec::with_capacity(primitives.len());
//                 for prim in primitives {
//                     result.push(T::from_property(&InstanceProperty::Primitive(
//                         prim.clone(),
//                     ))?);
//                 }
//                 Ok(result)
//             }
//             InstanceProperty::Relations(relations) => {
//                 let mut result = Vec::with_capacity(relations.len());
//                 for rel in relations {
//                     result.push(T::from_property(&InstanceProperty::Relation(rel.clone()))?);
//                 }
//                 Ok(result)
//             }
//             InstanceProperty::Any(items) => {
//                 let mut result = Vec::with_capacity(items.len());
//                 for item in items {
//                     result.push(T::from_property(item)?);
//                 }
//                 Ok(result)
//             }
//             _ => Err(anyhow::anyhow!(
//                 "Expected a collection property, got {:?}",
//                 prop
//             )),
//         }
//     }
// }

// Add a helper function for adapting FromTDBInstance to FromInstanceProperty
pub fn from_tdb_instance_property<T: FromTDBInstance>(
    prop: &InstanceProperty,
) -> anyhow::Result<T> {
    match prop {
        InstanceProperty::Relation(RelationValue::One(instance)) => T::from_instance(instance),
        InstanceProperty::Relation(RelationValue::ExternalReference(id)) => Err(anyhow::anyhow!(
            "Cannot deserialize from external reference '{}' without instance tree",
            id
        )),
        InstanceProperty::Relation(RelationValue::TransactionRef(id)) => Err(anyhow::anyhow!(
            "Cannot deserialize from external reference '{}' without instance tree",
            id
        )),
        _ => Err(anyhow::anyhow!(
            "Expected a relation property for complex type, got {:?}",
            prop
        )),
    }
}

// Universal trait that can be safely called on any type
// Returns None for primitive types (use FromInstanceProperty)
// Returns Some(T) for complex types (successfully deserialized from instance)
pub trait MaybeFromTDBInstance: Sized {
    fn maybe_from_instance(instance: &crate::Instance) -> anyhow::Result<Option<Self>>;
    fn maybe_from_property(prop: &InstanceProperty) -> anyhow::Result<Option<Self>>;
}

// Default implementation for complex types that implement FromTDBInstance
impl<T: FromTDBInstance> MaybeFromTDBInstance for T {
    default fn maybe_from_instance(instance: &crate::Instance) -> anyhow::Result<Option<Self>> {
        Ok(Some(T::from_instance(instance)?))
    }

    default fn maybe_from_property(prop: &InstanceProperty) -> anyhow::Result<Option<Self>> {
        Ok(Some(from_tdb_instance_property(prop)?))
    }
}

// Specialized implementations for primitive types that return None
impl MaybeFromTDBInstance for String {
    fn maybe_from_instance(_instance: &crate::Instance) -> anyhow::Result<Option<Self>> {
        Ok(None) // Indicate this is a primitive type
    }

    fn maybe_from_property(_prop: &InstanceProperty) -> anyhow::Result<Option<Self>> {
        Ok(None) // Indicate this is a primitive type
    }
}

impl MaybeFromTDBInstance for Vec<String> {
    fn maybe_from_instance(_instance: &crate::Instance) -> anyhow::Result<Option<Self>> {
        Ok(None) // Indicate this is a primitive type
    }

    fn maybe_from_property(_prop: &InstanceProperty) -> anyhow::Result<Option<Self>> {
        Ok(None) // Indicate this is a primitive type
    }
}

// Add more primitive types as needed
impl MaybeFromTDBInstance for i32 {
    fn maybe_from_instance(_instance: &crate::Instance) -> anyhow::Result<Option<Self>> {
        Ok(None)
    }
    fn maybe_from_property(_prop: &InstanceProperty) -> anyhow::Result<Option<Self>> {
        Ok(None)
    }
}

impl MaybeFromTDBInstance for i64 {
    fn maybe_from_instance(_instance: &crate::Instance) -> anyhow::Result<Option<Self>> {
        Ok(None)
    }
    fn maybe_from_property(_prop: &InstanceProperty) -> anyhow::Result<Option<Self>> {
        Ok(None)
    }
}

impl MaybeFromTDBInstance for u32 {
    fn maybe_from_instance(_instance: &crate::Instance) -> anyhow::Result<Option<Self>> {
        Ok(None)
    }
    fn maybe_from_property(_prop: &InstanceProperty) -> anyhow::Result<Option<Self>> {
        Ok(None)
    }
}

impl MaybeFromTDBInstance for u64 {
    fn maybe_from_instance(_instance: &crate::Instance) -> anyhow::Result<Option<Self>> {
        Ok(None)
    }
    fn maybe_from_property(_prop: &InstanceProperty) -> anyhow::Result<Option<Self>> {
        Ok(None)
    }
}

impl MaybeFromTDBInstance for f32 {
    fn maybe_from_instance(_instance: &crate::Instance) -> anyhow::Result<Option<Self>> {
        Ok(None)
    }
    fn maybe_from_property(_prop: &InstanceProperty) -> anyhow::Result<Option<Self>> {
        Ok(None)
    }
}

impl MaybeFromTDBInstance for f64 {
    fn maybe_from_instance(_instance: &crate::Instance) -> anyhow::Result<Option<Self>> {
        Ok(None)
    }
    fn maybe_from_property(_prop: &InstanceProperty) -> anyhow::Result<Option<Self>> {
        Ok(None)
    }
}

impl MaybeFromTDBInstance for bool {
    fn maybe_from_instance(_instance: &crate::Instance) -> anyhow::Result<Option<Self>> {
        Ok(None)
    }
    fn maybe_from_property(_prop: &InstanceProperty) -> anyhow::Result<Option<Self>> {
        Ok(None)
    }
}

impl MaybeFromTDBInstance for usize {
    fn maybe_from_instance(_instance: &crate::Instance) -> anyhow::Result<Option<Self>> {
        Ok(None)
    }
    fn maybe_from_property(_prop: &InstanceProperty) -> anyhow::Result<Option<Self>> {
        Ok(None)
    }
}

impl MaybeFromTDBInstance for isize {
    fn maybe_from_instance(_instance: &crate::Instance) -> anyhow::Result<Option<Self>> {
        Ok(None)
    }
    fn maybe_from_property(_prop: &InstanceProperty) -> anyhow::Result<Option<Self>> {
        Ok(None)
    }
}

impl MaybeFromTDBInstance for u8 {
    fn maybe_from_instance(_instance: &crate::Instance) -> anyhow::Result<Option<Self>> {
        Ok(None)
    }
    fn maybe_from_property(_prop: &InstanceProperty) -> anyhow::Result<Option<Self>> {
        Ok(None)
    }
}

impl MaybeFromTDBInstance for i8 {
    fn maybe_from_instance(_instance: &crate::Instance) -> anyhow::Result<Option<Self>> {
        Ok(None)
    }
    fn maybe_from_property(_prop: &InstanceProperty) -> anyhow::Result<Option<Self>> {
        Ok(None)
    }
}

// Helper trait for types that might be primitive or complex
pub trait FromInstancePropertyOrTDB: Sized {
    fn from_property_or_tdb(prop: &InstanceProperty) -> anyhow::Result<Self>;
}

// Default implementation for types that implement FromTDBInstance
impl<T: FromTDBInstance> FromInstancePropertyOrTDB for T {
    default fn from_property_or_tdb(prop: &InstanceProperty) -> anyhow::Result<Self> {
        from_tdb_instance_property(prop)
    }
}

// Specialized implementation for String (which is Primitive)
impl FromInstancePropertyOrTDB for String {
    fn from_property_or_tdb(prop: &InstanceProperty) -> anyhow::Result<Self> {
        String::from_property(prop)
    }
}

// Specialized implementation for Vec<String> (which contains Primitive)
impl FromInstancePropertyOrTDB for Vec<String> {
    fn from_property_or_tdb(prop: &InstanceProperty) -> anyhow::Result<Self> {
        Vec::<String>::from_property(prop)
    }
}

// Add this specialized implementation for simple enums
// impl<T, Parent> ToInstanceProperty<Parent> for T
// where
//     T: ToTDBSchema + Copy + std::fmt::Debug + 'static, // Ensure T implements ToTDBSchema
//     // Add bounds to help identify simple enums if possible (e.g., Copy, Debug)
// {
//     default fn to_property(self, _field_name: &str, parent: &Schema) -> InstanceProperty {
//         // Check if the schema is actually an Enum type
//         if let Schema::Enum { .. } = T::to_schema() {
//             // If it's an enum, serialize its debug representation as a string
//             // (This assumes the debug representation matches the desired string value, e.g., "Completed")
//             InstanceProperty::Primitive(PrimitiveValue::String(format!("{:?}", self)))
//         } else {
//             // Fallback for non-enum types (or potentially tagged unions)
//             // This might still need refinement if tagged unions are handled differently
//             InstanceProperty::Relation(RelationValue::One(self.to_instance(None)))
//         }
//     }
// }

impl MaybeFromTDBInstance for crate::XSDAnySimpleType {
    fn maybe_from_instance(_instance: &crate::Instance) -> anyhow::Result<Option<Self>> {
        Ok(None) // XSDAnySimpleType is a primitive type
    }
    fn maybe_from_property(_prop: &InstanceProperty) -> anyhow::Result<Option<Self>> {
        Ok(None) // XSDAnySimpleType is a primitive type
    }
}

// Specific implementations for the cases we need: Option and Vec with primitive String
impl MaybeFromTDBInstance for Option<String> {
    fn maybe_from_instance(_instance: &crate::Instance) -> anyhow::Result<Option<Self>> {
        Ok(None) // String is primitive, so Option<String> should use FromInstanceProperty
    }

    fn maybe_from_property(_prop: &InstanceProperty) -> anyhow::Result<Option<Self>> {
        Ok(None) // String is primitive, so Option<String> should use FromInstanceProperty
    }
}

impl MaybeFromTDBInstance for Option<Vec<String>> {
    fn maybe_from_instance(_instance: &crate::Instance) -> anyhow::Result<Option<Self>> {
        Ok(None) // Vec<String> is primitive, so Option<Vec<String>> should use FromInstanceProperty
    }

    fn maybe_from_property(_prop: &InstanceProperty) -> anyhow::Result<Option<Self>> {
        Ok(None) // Vec<String> is primitive, so Option<Vec<String>> should use FromInstanceProperty
    }
}
