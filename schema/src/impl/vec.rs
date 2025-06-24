use crate::{
    FromInstanceProperty, FromTDBInstance, Instance, InstanceProperty, Primitive, PrimitiveValue,
    Property, RelationValue, Schema, TdbLazy, ToInstanceProperty, ToSchemaClass, ToSchemaProperty,
    ToTDBInstance, ToTDBInstances, ToTDBSchema, TypeFamily,
};
use anyhow::{anyhow, bail};
use std::collections::HashSet;

impl<T: ToTDBSchema> ToTDBSchema for Vec<T> {
    fn to_schema() -> Schema {
        T::to_schema()
    }

    fn to_schema_tree() -> Vec<Schema> {
        T::to_schema_tree()
    }

    fn to_schema_tree_mut(collection: &mut HashSet<Schema>) {
        let schema = <Self as ToTDBSchema>::to_schema();
        let class_name = schema.class_name().clone();

        // Check if we already have a schema with this class name
        if !collection
            .iter()
            .any(|s: &Schema| s.class_name() == &class_name)
        {
            collection.insert(schema);

            // Process the inner type
            T::to_schema_tree_mut(collection);
        }
    }
}

// Implement ToInstanceProperty for Vec<T> where T implements ToTDBInstance
impl<T: ToTDBInstance, S> ToInstanceProperty<S> for Vec<T> {
    default fn to_property(self, field_name: &str, parent: &Schema) -> InstanceProperty {
        InstanceProperty::Relations(
            self.into_iter()
                .map(|item| RelationValue::One(item.to_instance(None)))
                .collect(),
        )
    }
}

impl<T: Primitive, S> ToInstanceProperty<S> for Vec<T> {
    default fn to_property(self, field_name: &str, parent: &Schema) -> InstanceProperty {
        InstanceProperty::Primitives(self.into_iter().map(|item| item.into()).collect())
    }
}

impl<Parent, T: ToSchemaClass> ToSchemaProperty<Parent> for Vec<T> {
    default fn to_property(name: &str) -> Property {
        Property {
            name: name.to_string(),
            r#type: Some(TypeFamily::List),
            class: T::to_class().to_string(),
        }
    }
}

// Implement FromTDBInstance for Vec<T> where T implements FromTDBInstance
impl<T: FromTDBInstance> FromTDBInstance for Vec<T> {
    fn from_instance(instance: &Instance) -> Result<Self, anyhow::Error> {
        // For a Vec, we expect an instance with a single property that is an array
        if instance.properties.len() != 1 {
            return Err(anyhow::anyhow!(
                "Expected single property for Vec deserialization"
            ));
        }

        let (_, prop) = instance.properties.iter().next().unwrap();

        match prop {
            InstanceProperty::Any(values) => {
                let mut result = Vec::new();
                for value in values {
                    match value {
                        InstanceProperty::Relation(RelationValue::One(instance)) => {
                            result.push(T::from_instance(instance)?);
                        }
                        _ => {
                            return Err(anyhow::anyhow!("Unexpected property type for Vec element"))
                        }
                    }
                }
                Ok(result)
            }
            InstanceProperty::Relations(relations) => {
                let mut result = Vec::new();
                for relation in relations {
                    match relation {
                        RelationValue::One(instance) => {
                            result.push(T::from_instance(instance)?);
                        }
                        _ => {
                            return Err(anyhow::anyhow!("Unexpected relation type for Vec element"))
                        }
                    }
                }
                Ok(result)
            }
            _ => Err(anyhow::anyhow!(
                "Unexpected property type for Vec deserialization"
            )),
        }
    }

    fn from_instance_tree(instances: &[Instance]) -> Result<Self, anyhow::Error> {
        if instances.is_empty() {
            return Ok(Vec::new());
        }

        // For instance tree, we assume the first instance contains references to other instances
        let root = &instances[0];

        // We need to extract the relation values and find the corresponding instances
        let mut result = Vec::new();

        for (_, prop) in &root.properties {
            match prop {
                InstanceProperty::Any(values) => {
                    for value in values {
                        if let InstanceProperty::Relation(RelationValue::ExternalReference(id)) =
                            value
                        {
                            // Find the instance with this id
                            if let Some(instance) =
                                instances.iter().find(|i| i.id.as_ref() == Some(id))
                            {
                                result.push(T::from_instance(instance)?);
                            }
                        }
                    }
                }
                InstanceProperty::Relations(relations) => {
                    for relation in relations {
                        if let RelationValue::ExternalReference(id) = relation {
                            // Find the instance with this id
                            if let Some(instance) =
                                instances.iter().find(|i| i.id.as_ref() == Some(id))
                            {
                                result.push(T::from_instance(instance)?);
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        Ok(result)
    }
}

impl<I: ToTDBInstance> ToTDBInstances for Vec<I> {
    default fn to_instance_tree(&self) -> Vec<Instance> {
        self.iter()
            .map(|v| v.to_instance_tree())
            .flatten()
            .collect()
    }
}

impl<T: FromInstanceProperty + FromTDBInstance + ToTDBSchema> FromInstanceProperty for Vec<T> {
    fn from_property(prop: &InstanceProperty) -> anyhow::Result<Self> {
        match prop {
            InstanceProperty::Primitives(arr) => {
                if arr.is_empty() {
                    return Ok(vec![]);
                }

                todo!()
            }
            InstanceProperty::Relations(arr) => {
                let mut targets = Vec::with_capacity(arr.len());
                for relation in arr {
                    match relation {
                        RelationValue::One(instance) => {
                            let target = T::from_instance(instance)?;
                            targets.push(target);
                        }

                        RelationValue::More(instances) => {
                            for instance in instances {
                                let target = T::from_instance(instance)?;
                                targets.push(target);
                            }
                        }

                        RelationValue::ExternalReference(r) => {
                            let target = T::from_instance(&Instance::new_reference::<T>(r))?;
                            targets.push(target);
                        }
                        RelationValue::ExternalReferences(_) => {
                            todo!("external refs")
                        }
                        RelationValue::TransactionRef(_) => {
                            todo!("txref")
                        }
                        RelationValue::TransactionRefs(_) => {
                            todo!("txrefs")
                        }
                    }
                }
                Ok(targets)
            }
            InstanceProperty::Any(_) => {
                todo!()
            }
            _ => unimplemented!(),
        }
    }
}

// impl<T: FromInstanceProperty+FromTDBInstance> FromInstanceProperty for Vec<TdbLazy<T>> {
//     default fn from_property(prop: &InstanceProperty) -> anyhow::Result<Self> {
//         match prop {
//             InstanceProperty::Primitives(arr) => {todo!()}
//             InstanceProperty::Relations(arr) => {
//                 let mut targets = Vec::with_capacity(arr.len());
//                 for relation in arr {
//                     match relation {
//                         RelationValue::One(instance) => {
//                             let target = <TdbLazy<T>>::from_instance(instance)?;
//                             targets.push(target);
//                         }
//                         RelationValue::More(instances) => {
//                             for instance in instances {
//                                 let target = <TdbLazy<T>>::from_instance(instance)?;
//                                 targets.push(target);
//                             }
//                         }
//
//                         RelationValue::ExternalReference(r) => {targets.push(TdbLazy::new(r, None));}
//                         RelationValue::ExternalReferences(refs) => {for r in refs {
//                             targets.push(TdbLazy::new(r, None));
//                         }}
//                         RelationValue::TransactionRef(_) => {todo!("txref")}
//                         RelationValue::TransactionRefs(_) => {todo!("txrefs")}
//                     }
//                 }
//                 Ok(targets)
//             }
//             InstanceProperty::Any(_) => {todo!()}
//             _ => unimplemented!()
//         }
//     }
// }

impl FromInstanceProperty for Vec<String> {
    default fn from_property(prop: &InstanceProperty) -> anyhow::Result<Self> {
        match prop {
            InstanceProperty::Primitives(arr) => {
                let mut targets = Vec::with_capacity(arr.len());
                for primitive in arr {
                    targets.push(match primitive {
                        PrimitiveValue::String(s) => s.clone(),
                        _ => {
                            bail!("Expected PrimitiveValue::String, got: {:#?}", primitive)
                        }
                    });
                }
                Ok(targets)
            }
            _ => unimplemented!(),
        }
    }
}

impl FromInstanceProperty for Vec<f32> {
    default fn from_property(prop: &InstanceProperty) -> anyhow::Result<Self> {
        match prop {
            InstanceProperty::Primitives(arr) => {
                let mut targets = Vec::with_capacity(arr.len());
                for primitive in arr {
                    targets.push(match primitive {
                        PrimitiveValue::Number(num) => num
                            .as_f64()
                            .ok_or(anyhow!(" Number cannot be converted to f64"))?
                            as f32,
                        _ => {
                            bail!("Expected PrimitiveValue::Number, got: {:#?}", primitive)
                        }
                    });
                }
                Ok(targets)
            }
            _ => unimplemented!(),
        }
    }
}

impl FromInstanceProperty for Vec<i32> {
    default fn from_property(prop: &InstanceProperty) -> anyhow::Result<Self> {
        match prop {
            InstanceProperty::Primitives(arr) => {
                let mut targets = Vec::with_capacity(arr.len());
                for primitive in arr {
                    targets.push(match primitive {
                        PrimitiveValue::Number(num) => num
                            .as_i64()
                            .ok_or(anyhow!(" Number cannot be converted to f64"))?
                            as i32,
                        _ => {
                            bail!("Expected PrimitiveValue::Number, got: {:#?}", primitive)
                        }
                    });
                }
                Ok(targets)
            }
            _ => unimplemented!(),
        }
    }
}

impl FromInstanceProperty for Vec<f64> {
    default fn from_property(prop: &InstanceProperty) -> anyhow::Result<Self> {
        match prop {
            InstanceProperty::Primitives(arr) => {
                let mut targets = Vec::with_capacity(arr.len());
                for prim in arr {
                    if let PrimitiveValue::Number(num) = prim {
                        targets.push(num.as_f64().ok_or_else(|| {
                            anyhow!("Failed to convert number to f64: {:?}", num)
                        })?);
                    } else {
                        return Err(anyhow!("Expected number primitive, got {:?}", prim));
                    }
                }
                Ok(targets)
            }
            _ => Err(anyhow!("Expected primitives array, got {:?}", prop)),
        }
    }
}

// Add Vec<u8> support
impl FromInstanceProperty for Vec<u8> {
    fn from_property(prop: &InstanceProperty) -> anyhow::Result<Self> {
        match prop {
            InstanceProperty::Primitives(arr) => {
                let mut targets = Vec::with_capacity(arr.len());
                for prim in arr {
                    match prim {
                        PrimitiveValue::Number(num) => {
                            let byte_val = num.as_u64().ok_or_else(|| {
                                anyhow!("Failed to convert number to u64: {:?}", num)
                            })?;
                            if byte_val > 255 {
                                return Err(anyhow!("Byte value out of range: {}", byte_val));
                            }
                            targets.push(byte_val as u8);
                        }
                        _ => {
                            return Err(anyhow!("Expected number primitive for u8, got {:?}", prim))
                        }
                    }
                }
                Ok(targets)
            }
            _ => Err(anyhow!(
                "Expected primitives array for Vec<u8>, got {:?}",
                prop
            )),
        }
    }
}
