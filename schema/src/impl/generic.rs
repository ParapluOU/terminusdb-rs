use crate::{json::ToJson, Instance, InstanceProperty, PrimitiveValue, Property, RelationValue, Schema, ToInstanceProperty, ToMaybeTDBSchema, ToSchemaClass, ToSchemaProperty, ToSchemaPropertyJsonValue, ToSchemaPropertyName, ToTDBInstance, ToTDBInstances, ToTDBSchema};
use serde_json::{Map, Value};
use std::collections::HashSet;

impl<T> ToMaybeTDBSchema for T {
    default fn to_schema() -> Option<Schema> {
        None
    }

    default fn to_schema_tree() -> Vec<Schema> {
        vec![]
    }

    default fn to_schema_tree_mut(collection: &mut HashSet<Schema>) {
        // Do nothing by default - this type might not have a schema
    }
}

impl<T: ToTDBSchema> ToMaybeTDBSchema for T {
    fn to_schema() -> Option<Schema> {
        Some(<Self as ToTDBSchema>::to_schema())
    }

    fn to_schema_tree() -> Vec<Schema> {
        <Self as ToTDBSchema>::to_schema_tree()
    }

    fn to_schema_tree_mut(collection: &mut HashSet<Schema>) {
        <Self as ToTDBSchema>::to_schema_tree_mut(collection)
    }
}

// Custom implementation for struct types that implement ToTDBInstance
impl<T: ToTDBInstance, S> ToInstanceProperty<S> for T
where
    T: ToTDBInstance,
    // Don't add extra constraints - just check that it's a ToTDBInstance
{
    default fn to_property(self, field_name: &str, parent: &Schema) -> InstanceProperty {
        let inst = self.to_instance(None);
        if Self::to_schema().is_enum() {
            return InstanceProperty::Primitive(PrimitiveValue::String(inst.enum_value().expect("enum shoujld have the variant property")))
        }
        InstanceProperty::Relation(RelationValue::One(inst))
    }
}

// Implement ToTDBInstances for references to types that already implement ToTDBInstances
impl<'a, T: ToTDBInstances+Sync> ToTDBInstances for &'a T {
    fn to_instance_tree(&self) -> Vec<Instance> {
        (*self).to_instance_tree()
    }
}

impl<T: ToSchemaPropertyName + ToSchemaPropertyJsonValue> ToJson for T {
    default fn to_map(&self) -> Map<String, Value> {
        let mut map = serde_json::Map::new();
        map.insert(self.to_property_name(), self.to_property_value());
        map
    }
}

impl<Parent, T: ToSchemaClass> ToSchemaProperty<Parent> for T {
    default fn to_property(prop_name: &str) -> Property {
        Property {
            name: prop_name.to_string(),
            class: T::to_class().to_string(),
            r#type: None,
        }
    }
}
