use crate::{
    json::{InstanceFromJson, InstancePropertyFromJson},
    FromInstanceProperty, Instance, InstanceProperty, PrimitiveValue, RelationValue,
    ToInstanceProperty, ToTDBInstance,
};
use anyhow::{anyhow, bail, Context, Result};
use serde_json::Value;
use std::marker::PhantomData;
use serde::de::DeserializeOwned;

// Implementations for primitive types
impl<Parent> InstancePropertyFromJson<Parent> for String {
    fn property_from_json(json: Value) -> Result<InstanceProperty> {
        match json {
            Value::String(s) => Ok(InstanceProperty::Primitive(PrimitiveValue::String(s))),
            _ => Err(anyhow!("Expected a string, got {:?}", json)),
        }
    }
}

impl<Parent> InstancePropertyFromJson<Parent> for bool {
    fn property_from_json(json: Value) -> Result<InstanceProperty> {
        match json {
            Value::Bool(b) => Ok(InstanceProperty::Primitive(PrimitiveValue::Bool(b))),
            _ => Err(anyhow!("Expected a boolean, got {:?}", json)),
        }
    }
}

// Integer types
macro_rules! impl_int_deserialization {
    ($ty:ty) => {
        impl<Parent> InstancePropertyFromJson<Parent> for $ty {
            fn property_from_json(json: Value) -> Result<InstanceProperty> {
                match json {
                    Value::Number(n) => {
                        if let Some(i) = n.as_i64() {
                            if i >= <$ty>::MIN as i64 && i <= <$ty>::MAX as i64 {
                                return Ok(InstanceProperty::Primitive(PrimitiveValue::Number(n)));
                            }
                            return Err(anyhow!(
                                "Number {} is out of range for {}",
                                i,
                                stringify!($ty)
                            ));
                        }
                        Err(anyhow!("Number cannot be represented as an integer"))
                    }
                    _ => Err(anyhow!("Expected a number, got {:?}", json)),
                }
            }
        }
    };
}

impl_int_deserialization!(i8);
impl_int_deserialization!(i16);
impl_int_deserialization!(i32);
impl_int_deserialization!(u8);
impl_int_deserialization!(u16);
impl_int_deserialization!(u32);
impl_int_deserialization!(isize);
impl_int_deserialization!(usize);

// Float types
macro_rules! impl_float_deserialization {
    ($ty:ty) => {
        impl<Parent> InstancePropertyFromJson<Parent> for $ty {
            fn property_from_json(json: Value) -> Result<InstanceProperty> {
                match json {
                    Value::Number(n) => {
                        if let Some(f) = n.as_f64() {
                            return Ok(InstanceProperty::Primitive(PrimitiveValue::Number(n)));
                        }
                        Err(anyhow!("Number cannot be represented as a float"))
                    }
                    _ => Err(anyhow!("Expected a number, got {:?}", json)),
                }
            }
        }
    };
}

impl_float_deserialization!(f32);
impl_float_deserialization!(f64);

// Option type
impl<T, Parent> InstancePropertyFromJson<Parent> for Option<T>
where
    T: InstancePropertyFromJson<Parent>,
    T: FromInstanceProperty,
    Option<T>: ToInstanceProperty<Parent>,
{
    fn property_from_json(json: Value) -> Result<InstanceProperty> {
        if json.is_null() {
            return Ok(InstanceProperty::Primitive(PrimitiveValue::Null));
        }
        // If not null, delegate to the inner type
        T::property_from_json(json)
    }

    // Override the default implementation to handle None values specially for Option
    fn property_from_maybe_json(json: Option<Value>) -> Result<InstanceProperty> {
        match json {
            None | Some(Value::Null) => Ok(InstanceProperty::Primitive(PrimitiveValue::Null)),
            Some(value) => T::property_from_json(value),
        }
    }
}

// Vec type
impl<T, Parent> InstancePropertyFromJson<Parent> for Vec<T>
where
    T: InstancePropertyFromJson<Parent>,
    Vec<T>: ToInstanceProperty<Parent>,
{
    fn property_from_json(json: Value) -> Result<InstanceProperty> {
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
                        .collect::<Result<Vec<_>>>()?;

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
                        .collect::<Result<Vec<_>>>()?;

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

// Implementation for complex types that implement InstanceFromJson
impl<T, Parent> InstancePropertyFromJson<Parent> for T
where
    T: InstanceFromJson + ToTDBInstance + DeserializeOwned,
    T: 'static, // Needed to confirm type isn't a primitive which would already have an impl
{
    default fn property_from_json(json: Value) -> Result<InstanceProperty> {
        // todo: it would be cleaner to derive a (marker) trait for enums
        // and then have a separate InstancePropertyFromJson for enums
        // so we wouldn thave this conditional inside the generic
        if T::to_schema().is_enum() {
            return if let Value::String(enum_variant) = json {
                let enm : T = serde_json::from_str(&format!("\"{}\"", &enum_variant))?;
                // Ok(InstanceProperty::Primitive(PrimitiveValue::String(enum_variant)))
                Ok(InstanceProperty::Relation(RelationValue::One(enm.to_instance(None))))
            }
            else {
                bail!("expected String value for Enum")
            }
        }

        // Use the InstanceFromJson implementation to create an Instance
        let instance = T::instance_from_json(json)?;

        // Convert the Instance to a relation property
        Ok(InstanceProperty::Relation(RelationValue::One(instance)))
    }
}
