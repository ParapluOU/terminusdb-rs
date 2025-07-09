use crate::json::InstancePropertyFromJson;
use crate::{
    FromInstanceProperty, InstanceProperty, PrimitiveValue, Property, Schema, TerminusDBModel,
    ToInstanceProperty, ToSchemaProperty, ToTDBSchema, STRING, URI,
};
use anyhow::{anyhow, bail};
use rocket::form::{self, FromFormField, ValueField};
use rocket::request::FromParam;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use std::collections::HashSet;
use std::convert::TryInto;
use std::fmt;
use std::fmt::Formatter;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::ops::Deref;
use uuid::Uuid;
// todo: needs unit tests

#[derive(Debug)]
pub struct EntityIDFor<T: ToTDBSchema> {
    /// base IRI
    base: Option<String>,
    /// ID with the type name. not very purist, but then we can nicely deref to something valuable
    typed_id: String,
    /// allows strong typing
    _ty: PhantomData<T>,
}

impl<T: ToTDBSchema> ToTDBSchema for EntityIDFor<T> {
    fn to_schema_tree() -> Vec<Schema> {
        vec![T::to_schema()]
    }

    // Change to_schema_tree_mut to be a static method
    fn to_schema_tree_mut(collection: &mut HashSet<Schema>) {
        T::to_schema_tree_mut(collection);
    }

    fn id() -> Option<String> {
        T::id()
    }

    fn properties() -> Option<Vec<Property>> {
        T::properties()
    }

    fn values() -> Option<Vec<URI>> {
        T::values()
    }
}

impl<T: ToTDBSchema> EntityIDFor<T> {
    pub fn random() -> Self {
        Self::new(&Uuid::new_v4().to_string()).unwrap()
    }

    pub fn new(iri_or_id: &str) -> anyhow::Result<Self> {
        // if its a pure ID
        Ok(if !iri_or_id.contains("/") {
            Self {
                base: None,
                typed_id: format!("{}/{}", T::schema_name(), iri_or_id),
                _ty: Default::default(),
            }
        }
        // IRI: e.g., terminusdb://data#TestEntity/91011 or terminusdb:///data/TestEntity/91011
        else if iri_or_id.contains("://") {
            // Check if it's fragment-based (contains #) or path-based
            if iri_or_id.contains('#') {
                // Fragment-based IRI: terminusdb://data#TestEntity/91011
                let parts: Vec<&str> = iri_or_id.split('#').collect();
                if parts.len() != 2 {
                    return Err(anyhow!("Invalid IRI format: missing '#': '{}'", iri_or_id));
                }
                let base = parts[0];
                let typed_id_part = parts[1];

                let typed_parts: Vec<&str> = typed_id_part.split('/').collect();
                if typed_parts.len() != 2 {
                    return Err(anyhow!(
                        "Invalid IRI format: missing '/' after '#': '{}'",
                        iri_or_id
                    ));
                }
                let type_name = typed_parts[0];
                let id = typed_parts[1];

                // Validate type name
                if type_name != T::schema_name() {
                    return Err(anyhow!(
                        "Mismatched type in IRI: expected '{}', found '{}' in '{}'",
                        T::schema_name(),
                        type_name,
                        iri_or_id
                    ));
                }

                Self {
                    base: Some(base.to_string()),
                    typed_id: typed_id_part.to_string(),
                    _ty: Default::default(),
                }
            } else {
                // Path-based IRI: terminusdb:///data/TestEntity/91011
                // Split by / and look for the type name and ID in the path
                let path_parts: Vec<&str> = iri_or_id.split('/').collect();

                // Find the type name and ID - they should be the last two components
                if path_parts.len() < 2 {
                    return Err(anyhow!(
                        "Invalid path-based IRI format: not enough path components in '{}'",
                        iri_or_id
                    ));
                }

                // Get the last two path components (should be Type/ID)
                let type_name = path_parts[path_parts.len() - 2];
                let id = path_parts[path_parts.len() - 1];

                // Validate type name
                if type_name != T::schema_name() {
                    return Err(anyhow!(
                        "Mismatched type in IRI: expected '{}', found '{}' in '{}'",
                        T::schema_name(),
                        type_name,
                        iri_or_id
                    ));
                }

                // Extract base by removing the Type/ID part
                let base_end = iri_or_id
                    .rfind(&format!("/{}/{}", type_name, id))
                    .ok_or_else(|| anyhow!("Failed to extract base from IRI: '{}'", iri_or_id))?;
                let base = &iri_or_id[..base_end];

                Self {
                    base: Some(base.to_string()),
                    typed_id: format!("{}/{}", type_name, id),
                    _ty: Default::default(),
                }
            }
        }
        // type/id: e.g., TestEntity/5678
        else {
            let parts: Vec<&str> = iri_or_id.split('/').collect();
            if parts.len() != 2 {
                return Err(anyhow!("Invalid typed ID format: '{}'", iri_or_id));
            }
            let type_name = parts[0];
            let id = parts[1];

            // Validate type name
            if type_name != T::schema_name() {
                return Err(anyhow!(
                    "Mismatched type in typed ID: expected '{}', found '{}' in '{}'",
                    T::schema_name(),
                    type_name,
                    iri_or_id
                ));
            }

            Self {
                base: None,
                typed_id: iri_or_id.to_string(),
                _ty: Default::default(),
            }
        })
    }

    // todo: stronger typing than string
    /// name with terminusdb://data#MyType/1234
    pub fn iri(&self) -> String {
        todo!()
    }

    /// return just the identifier part
    pub fn id(&self) -> &str {
        self.typed_id.split("/").last().unwrap()
    }

    /// return MyType/1234 format
    pub fn typed(&self) -> &String {
        &self.typed_id
    }

    pub fn to_string(&self) -> String {
        self.typed_id.clone()
    }

    pub fn remap<X: ToTDBSchema>(self) -> EntityIDFor<X> {
        EntityIDFor::new(self.id()).unwrap()
    }
}

impl<T: ToTDBSchema> PartialEq<str> for EntityIDFor<T> {
    fn eq(&self, other: &str) -> bool {
        self.typed_id == other
    }
}

impl<T: ToTDBSchema> Default for EntityIDFor<T> {
    fn default() -> Self {
        Self::random()
    }
}

impl<T: ToTDBSchema> Hash for EntityIDFor<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.base.hash(state);
        self.typed_id.hash(state);
    }
}

impl<T: ToTDBSchema> PartialEq<EntityIDFor<T>> for EntityIDFor<T> {
    fn eq(&self, other: &Self) -> bool {
        self.typed_id == other.typed_id && self.base == other.base
    }
}

impl<T: ToTDBSchema> Eq for EntityIDFor<T> {}

impl<T: ToTDBSchema> PartialOrd for EntityIDFor<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: ToTDBSchema> Ord for EntityIDFor<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // First compare by typed_id, then by base if needed
        match self.typed_id.cmp(&other.typed_id) {
            std::cmp::Ordering::Equal => self.base.cmp(&other.base),
            other => other,
        }
    }
}

impl<T: ToTDBSchema + Clone> TryInto<EntityIDFor<T>> for &EntityIDFor<T> {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<EntityIDFor<T>, Self::Error> {
        Ok(self.clone())
    }
}

impl<T: ToTDBSchema> Into<String> for EntityIDFor<T> {
    fn into(self) -> String {
        self.to_string()
    }
}

impl<T: ToTDBSchema> From<Uuid> for EntityIDFor<T> {
    fn from(value: Uuid) -> Self {
        Self::new(&value.to_string()).unwrap()
    }
}

impl<T: ToTDBSchema> TryInto<EntityIDFor<T>> for String {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<EntityIDFor<T>, Self::Error> {
        EntityIDFor::new(&self)
    }
}

impl<T: ToTDBSchema> TryInto<EntityIDFor<T>> for &String {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<EntityIDFor<T>, Self::Error> {
        EntityIDFor::new(self)
    }
}

impl<T: ToTDBSchema> TryInto<EntityIDFor<T>> for &str {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<EntityIDFor<T>, Self::Error> {
        EntityIDFor::new(self)
    }
}

impl<T: ToTDBSchema> fmt::Display for EntityIDFor<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(&self.typed())
    }
}

impl<T: ToTDBSchema> Serialize for EntityIDFor<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.typed())
    }
}

impl<'de, T: ToTDBSchema> Deserialize<'de> for EntityIDFor<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::new(&s).map_err(serde::de::Error::custom)
    }
}

impl<T: ToTDBSchema> Clone for EntityIDFor<T> {
    fn clone(&self) -> Self {
        Self {
            base: self.base.clone(),
            typed_id: self.typed_id.clone(),
            _ty: Default::default(),
        }
    }
}

impl<T: ToTDBSchema> Deref for EntityIDFor<T> {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        self.typed()
    }
}

impl<T: ToTDBSchema, Parent> ToSchemaProperty<Parent> for EntityIDFor<T> {
    fn to_property(prop_name: &str) -> Property {
        Property {
            name: prop_name.to_string(),
            r#type: None,
            class: STRING.to_string(),
        }
    }
}

impl<T: ToTDBSchema, Parent> ToInstanceProperty<Parent> for EntityIDFor<T> {
    fn to_property(self, field_name: &str, parent: &Schema) -> InstanceProperty {
        InstanceProperty::Primitive(PrimitiveValue::String(self.to_string()))
    }
}

impl<T: ToTDBSchema> FromInstanceProperty for EntityIDFor<T> {
    fn from_property(prop: &InstanceProperty) -> anyhow::Result<Self> {
        match prop {
            InstanceProperty::Primitive(PrimitiveValue::String(id)) => Self::new(id),
            _ => bail!(
                "expected InstanceProperty::Primitive(PrimitiveValue::String), got: {:#?}",
                prop
            ),
        }
    }
}

impl<T: ToTDBSchema, Parent> InstancePropertyFromJson<Parent> for EntityIDFor<T> {
    fn property_from_json(json: Value) -> anyhow::Result<InstanceProperty> {
        match &json {
            Value::String(id) => Ok(InstanceProperty::Primitive(PrimitiveValue::String(
                id.clone(),
            ))),
            _ => bail!("expected String for EntityIDFor, got: {:#?}", json),
        }
    }
}

// todo: put behind rocket-specific feature
impl<T: ToTDBSchema> FromParam<'_> for EntityIDFor<T> {
    type Error = anyhow::Error;

    fn from_param(param: &'_ str) -> Result<Self, Self::Error> {
        // The param is the raw URL segment - could be:
        // 1. Just an ID: "123"
        // 2. Type/ID: "Person/123"
        // 3. Full IRI: "terminusdb://data#Person/123" (though URL encoding might affect this)

        // URL decode the parameter first in case it contains encoded characters
        let decoded = urlencoding::decode(param)
            .map_err(|e| anyhow!("Failed to URL decode parameter: {}", e))?;

        // Use the existing constructor which handles all formats
        Self::new(&decoded)
    }
}

// Implement FromFormField for form submissions
impl<'r, T: ToTDBSchema + Send> FromFormField<'r> for EntityIDFor<T> {
    fn from_value(field: ValueField<'r>) -> form::Result<'r, Self> {
        // Use the existing new() method to parse the value
        match Self::new(field.value) {
            Ok(entity_id) => Ok(entity_id),
            Err(e) => Err(form::Error::validation(format!(
                "Invalid EntityIDFor<{}>: {}",
                T::schema_name(),
                e
            )))?,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;
    use terminusdb_schema_derive::TerminusDBModel;

    // Define a dummy struct for testing
    #[derive(Clone, Debug, Default)]
    struct TestEntity {
        nothing: String,
    }

    impl ToTDBSchema for TestEntity {
        fn schema_name() -> String {
            "TestEntity".to_string()
        }

        fn to_schema_tree() -> Vec<Schema> {
            vec![Schema::Class {
                id: Self::schema_name(),
                base: None,
                key: crate::Key::Random,
                documentation: None,
                subdocument: false,
                r#abstract: false,
                inherits: vec![],
                unfoldable: false,
                properties: vec![],
            }]
        }
    }

    #[test]
    fn test_parse_simple_id() {
        let entity_id: EntityIDFor<TestEntity> = EntityIDFor::new("1234").unwrap();
        assert_eq!(entity_id.id(), "1234");
        assert_eq!(entity_id.typed(), "TestEntity/1234");
        assert_eq!(entity_id.base, None);
    }

    #[test]
    fn test_parse_typed_id() {
        let entity_id: EntityIDFor<TestEntity> = EntityIDFor::new("TestEntity/5678").unwrap();
        assert_eq!(entity_id.id(), "5678");
        assert_eq!(entity_id.typed(), "TestEntity/5678");
        assert_eq!(entity_id.base, None);
    }

    // Test case for IRI
    #[test]
    // #[should_panic] // Expected to panic until implemented
    fn test_parse_iri() {
        let iri = "terminusdb://data#TestEntity/91011";
        let entity_id: EntityIDFor<TestEntity> = EntityIDFor::new(iri).unwrap();
        // Add assertions here once IRI parsing is implemented
        assert_eq!(entity_id.id(), "91011");
        assert_eq!(entity_id.base, Some("terminusdb://data".to_string()));
        assert_eq!(entity_id.typed(), "TestEntity/91011"); // typed() should ignore base
    }

    #[test]
    fn test_parse_iri_wrong_type() {
        let iri = "terminusdb://data#WrongType/91011";
        let result: Result<EntityIDFor<TestEntity>, _> = EntityIDFor::new(iri);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Mismatched type in IRI"));
    }

    #[test]
    fn test_parse_typed_id_wrong_type() {
        let typed_id = "WrongType/5678";
        let result: Result<EntityIDFor<TestEntity>, _> = EntityIDFor::new(typed_id);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Mismatched type in typed ID"));
    }

    #[test]
    fn test_parse_path_based_iri() {
        let iri = "terminusdb:///data/TestEntity/7aec78a3-9749-457e-a113-273df5edf156";
        let entity_id: EntityIDFor<TestEntity> = EntityIDFor::new(iri).unwrap();
        assert_eq!(entity_id.id(), "7aec78a3-9749-457e-a113-273df5edf156");
        assert_eq!(
            entity_id.typed(),
            "TestEntity/7aec78a3-9749-457e-a113-273df5edf156"
        );
        assert_eq!(entity_id.base, Some("terminusdb:///data".to_string()));
    }

    #[test]
    fn test_parse_path_based_iri_wrong_type() {
        let iri = "terminusdb:///data/WrongType/7aec78a3-9749-457e-a113-273df5edf156";
        let result: Result<EntityIDFor<TestEntity>, _> = EntityIDFor::new(iri);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Mismatched type in IRI"));
    }

    // Test FromFormField implementation
    #[test]
    fn test_from_form_field_simple_id() {
        use rocket::form::ValueField;

        let field = ValueField {
            name: rocket::form::name::NameView::new("id"),
            value: "1234",
        };

        let result = <EntityIDFor<TestEntity> as FromFormField>::from_value(field);
        assert!(result.is_ok());
        let entity_id = result.unwrap();
        assert_eq!(entity_id.id(), "1234");
        assert_eq!(entity_id.typed(), "TestEntity/1234");
    }

    #[test]
    fn test_from_form_field_typed_id() {
        use rocket::form::ValueField;

        let field = ValueField {
            name: rocket::form::name::NameView::new("id"),
            value: "TestEntity/5678",
        };

        let result = <EntityIDFor<TestEntity> as FromFormField>::from_value(field);
        assert!(result.is_ok());
        let entity_id = result.unwrap();
        assert_eq!(entity_id.id(), "5678");
        assert_eq!(entity_id.typed(), "TestEntity/5678");
    }

    #[test]
    fn test_from_form_field_invalid_type() {
        use rocket::form::ValueField;

        let field = ValueField {
            name: rocket::form::name::NameView::new("id"),
            value: "WrongType/1234",
        };

        let result = <EntityIDFor<TestEntity> as FromFormField>::from_value(field);
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error
            .to_string()
            .contains("Invalid EntityIDFor<TestEntity>"));
    }

    #[test]
    fn test_from_form_field_iri() {
        use rocket::form::ValueField;

        let field = ValueField {
            name: rocket::form::name::NameView::new("id"),
            value: "terminusdb://data#TestEntity/91011",
        };

        let result = <EntityIDFor<TestEntity> as FromFormField>::from_value(field);
        assert!(result.is_ok());
        let entity_id = result.unwrap();
        assert_eq!(entity_id.id(), "91011");
        assert_eq!(entity_id.typed(), "TestEntity/91011");
    }
}
