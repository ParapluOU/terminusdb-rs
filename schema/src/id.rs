use crate::json::InstancePropertyFromJson;
use crate::{
    FromInstanceProperty, InstanceProperty, Primitive, PrimitiveValue, Property, Schema,
    TerminusDBModel, ToInstanceProperty, ToSchemaClass, ToSchemaProperty, ToTDBSchema, TypeFamily,
    STRING, URI,
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

impl<T: ToTDBSchema + ToSchemaClass> ToSchemaClass for EntityIDFor<T> {
    fn to_class() -> String {
        T::to_class()
    }
}

// EntityIDFor is a primitive type that serializes to a string
impl<T: ToTDBSchema> Primitive for EntityIDFor<T> {}

// Convert EntityIDFor to PrimitiveValue
impl<T: ToTDBSchema> From<EntityIDFor<T>> for PrimitiveValue {
    fn from(id: EntityIDFor<T>) -> Self {
        PrimitiveValue::String(id.to_string())
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

impl<T: ToTDBSchema + ToSchemaClass, Parent> ToSchemaProperty<Parent> for EntityIDFor<T> {
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

/// A server-managed ID that wraps Option<EntityIDFor<T>> but is read-only for users.
/// This type can only be set by the HTTP client when retrieving models from the server.
#[derive(Debug, Default)]
pub struct ServerIDFor<T: ToTDBSchema> {
    /// The internal ID, which may be None for new instances
    inner: Option<EntityIDFor<T>>,
}

impl<T: ToTDBSchema> ServerIDFor<T> {
    /// Creates a new empty ServerIDFor (for new instances without an ID yet)
    pub fn new() -> Self {
        Self { inner: None }
    }

    /// Creates a ServerIDFor from an existing EntityIDFor
    /// This is marked with doc(hidden) as it should only be used internally
    #[doc(hidden)]
    pub fn from_entity_id(id: EntityIDFor<T>) -> Self {
        Self { inner: Some(id) }
    }

    /// Creates a ServerIDFor from an Option<EntityIDFor>
    /// This is marked with doc(hidden) as it should only be used internally
    #[doc(hidden)]
    pub fn from_option(id: Option<EntityIDFor<T>>) -> Self {
        Self { inner: id }
    }

    /// Sets the ID from the server. This method is intentionally doc(hidden)
    /// as it should only be used by the client crate when retrieving instances.
    /// Users should not be able to modify ServerIDFor values directly.
    #[doc(hidden)]
    pub fn __set_from_server(&mut self, id: EntityIDFor<T>) {
        self.inner = Some(id);
    }

    /// Sets the ID from the server using an Option. This method is intentionally doc(hidden)
    /// as it should only be used by the client crate when retrieving instances.
    #[doc(hidden)]
    pub fn __set_from_server_option(&mut self, id: Option<EntityIDFor<T>>) {
        self.inner = id;
    }

    /// Gets the inner Option<EntityIDFor<T>> for cases where direct access is needed
    pub fn as_option(&self) -> &Option<EntityIDFor<T>> {
        &self.inner
    }

    /// Checks if the ID is set
    pub fn is_some(&self) -> bool {
        self.inner.is_some()
    }

    /// Checks if the ID is not set
    pub fn is_none(&self) -> bool {
        self.inner.is_none()
    }

    /// Gets the ID if present
    pub fn as_ref(&self) -> Option<&EntityIDFor<T>> {
        self.inner.as_ref()
    }
}

// Implement Deref to allow transparent access to the Option<EntityIDFor<T>>
impl<T: ToTDBSchema> Deref for ServerIDFor<T> {
    type Target = Option<EntityIDFor<T>>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

// Clone implementation
impl<T: ToTDBSchema> Clone for ServerIDFor<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

// PartialEq implementation
impl<T: ToTDBSchema> PartialEq for ServerIDFor<T> {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl<T: ToTDBSchema> Eq for ServerIDFor<T> {}

// Hash implementation
impl<T: ToTDBSchema> Hash for ServerIDFor<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.inner.hash(state);
    }
}

// Display implementation
impl<T: ToTDBSchema> fmt::Display for ServerIDFor<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match &self.inner {
            Some(id) => write!(f, "{}", id),
            None => write!(f, "None"),
        }
    }
}

// Serialize implementation - serializes transparently as Option<EntityIDFor<T>>
impl<T: ToTDBSchema> Serialize for ServerIDFor<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.inner.serialize(serializer)
    }
}

// Deserialize implementation - deserializes from Option<EntityIDFor<T>>
impl<'de, T: ToTDBSchema> Deserialize<'de> for ServerIDFor<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let inner = Option::<EntityIDFor<T>>::deserialize(deserializer)?;
        Ok(Self { inner })
    }
}

// ToTDBSchema implementation - delegates to EntityIDFor
impl<T: ToTDBSchema> ToTDBSchema for ServerIDFor<T> {
    fn to_schema_tree() -> Vec<Schema> {
        EntityIDFor::<T>::to_schema_tree()
    }

    fn to_schema_tree_mut(collection: &mut HashSet<Schema>) {
        EntityIDFor::<T>::to_schema_tree_mut(collection);
    }

    fn id() -> Option<String> {
        // ServerIDFor doesn't have its own ID, it delegates to T
        T::id()
    }

    fn properties() -> Option<Vec<Property>> {
        EntityIDFor::<T>::properties()
    }

    fn values() -> Option<Vec<URI>> {
        EntityIDFor::<T>::values()
    }
}

// ToSchemaProperty implementation - treats it as an optional string property
impl<T: ToTDBSchema, Parent> ToSchemaProperty<Parent> for ServerIDFor<T> {
    fn to_property(prop_name: &str) -> Property {
        // Same as Option<EntityIDFor<T>> would be - an optional string
        Property {
            name: prop_name.to_string(),
            r#type: Some(TypeFamily::Optional),
            class: STRING.to_string(),
        }
    }
}

// ToInstanceProperty implementation
impl<T: ToTDBSchema, Parent> ToInstanceProperty<Parent> for ServerIDFor<T> {
    fn to_property(self, field_name: &str, parent: &Schema) -> InstanceProperty {
        match self.inner {
            Some(id) => {
                <EntityIDFor<T> as ToInstanceProperty<Parent>>::to_property(id, field_name, parent)
            }
            None => InstanceProperty::Primitive(PrimitiveValue::Null),
        }
    }
}

// FromInstanceProperty implementation
impl<T: ToTDBSchema> FromInstanceProperty for ServerIDFor<T> {
    fn from_property(prop: &InstanceProperty) -> anyhow::Result<Self> {
        match prop {
            InstanceProperty::Primitive(PrimitiveValue::Null) => Ok(Self { inner: None }),
            InstanceProperty::Primitive(PrimitiveValue::String(id)) => {
                let entity_id = EntityIDFor::new(id)?;
                Ok(Self {
                    inner: Some(entity_id),
                })
            }
            _ => bail!(
                "expected InstanceProperty::Primitive(PrimitiveValue::Null) or InstanceProperty::Primitive(PrimitiveValue::String), got: {:#?}",
                prop
            ),
        }
    }
}

impl<T: ToTDBSchema, Parent> InstancePropertyFromJson<Parent> for ServerIDFor<T> {
    fn property_from_json(json: Value) -> anyhow::Result<InstanceProperty> {
        match &json {
            Value::Null => Ok(InstanceProperty::Primitive(PrimitiveValue::Null)),
            Value::String(id) => Ok(InstanceProperty::Primitive(PrimitiveValue::String(
                id.clone(),
            ))),
            _ => bail!("expected null or String for ServerIDFor, got: {:#?}", json),
        }
    }
}

// Rocket FromParam implementation
impl<T: ToTDBSchema> FromParam<'_> for ServerIDFor<T> {
    type Error = anyhow::Error;

    fn from_param(param: &'_ str) -> Result<Self, Self::Error> {
        // URL decode the parameter first
        let decoded = urlencoding::decode(param)
            .map_err(|e| anyhow!("Failed to URL decode parameter: {}", e))?;

        // Parse as EntityIDFor
        let entity_id = EntityIDFor::new(&decoded)?;
        Ok(Self {
            inner: Some(entity_id),
        })
    }
}

// Rocket FromFormField implementation
impl<'r, T: ToTDBSchema + Send> FromFormField<'r> for ServerIDFor<T> {
    fn from_value(field: ValueField<'r>) -> form::Result<'r, Self> {
        // Try to parse as EntityIDFor
        match EntityIDFor::<T>::new(field.value) {
            Ok(entity_id) => Ok(Self {
                inner: Some(entity_id),
            }),
            Err(e) => Err(form::Error::validation(format!(
                "Invalid ServerIDFor<{}>: {}",
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

    // ServerIDFor tests
    #[test]
    fn test_server_id_for_new() {
        let server_id: ServerIDFor<TestEntity> = ServerIDFor::new();
        assert!(server_id.is_none());
        assert!(!server_id.is_some());
        assert_eq!(server_id.as_ref(), None);
    }

    #[test]
    fn test_server_id_for_from_entity_id() {
        let entity_id = EntityIDFor::<TestEntity>::new("123").unwrap();
        let server_id = ServerIDFor::from_entity_id(entity_id.clone());
        assert!(server_id.is_some());
        assert_eq!(server_id.as_ref(), Some(&entity_id));
        assert_eq!(server_id.as_ref().unwrap().id(), "123");
    }

    #[test]
    fn test_server_id_for_from_option() {
        // Test with Some
        let entity_id = EntityIDFor::<TestEntity>::new("456").unwrap();
        let server_id = ServerIDFor::from_option(Some(entity_id.clone()));
        assert!(server_id.is_some());
        assert_eq!(server_id.as_ref(), Some(&entity_id));

        // Test with None
        let server_id_none: ServerIDFor<TestEntity> = ServerIDFor::from_option(None);
        assert!(server_id_none.is_none());
    }

    #[test]
    fn test_server_id_for_deref() {
        let entity_id = EntityIDFor::<TestEntity>::new("789").unwrap();
        let server_id = ServerIDFor::from_entity_id(entity_id.clone());

        // Test deref
        let deref_result: &Option<EntityIDFor<TestEntity>> = &*server_id;
        assert_eq!(deref_result, &Some(entity_id));
    }

    #[test]
    fn test_server_id_for_clone() {
        let entity_id = EntityIDFor::<TestEntity>::new("abc").unwrap();
        let server_id = ServerIDFor::from_entity_id(entity_id);
        let cloned = server_id.clone();
        assert_eq!(server_id, cloned);
    }

    #[test]
    fn test_server_id_for_default() {
        let server_id: ServerIDFor<TestEntity> = Default::default();
        assert!(server_id.is_none());
    }

    #[test]
    fn test_server_id_for_equality() {
        let entity_id1 = EntityIDFor::<TestEntity>::new("123").unwrap();
        let entity_id2 = EntityIDFor::<TestEntity>::new("123").unwrap();
        let entity_id3 = EntityIDFor::<TestEntity>::new("456").unwrap();

        let server_id1 = ServerIDFor::from_entity_id(entity_id1);
        let server_id2 = ServerIDFor::from_entity_id(entity_id2);
        let server_id3 = ServerIDFor::from_entity_id(entity_id3);

        assert_eq!(server_id1, server_id2);
        assert_ne!(server_id1, server_id3);
    }

    #[test]
    fn test_server_id_for_display() {
        let entity_id = EntityIDFor::<TestEntity>::new("display-test").unwrap();
        let server_id = ServerIDFor::from_entity_id(entity_id);
        assert_eq!(server_id.to_string(), "TestEntity/display-test");

        let server_id_none: ServerIDFor<TestEntity> = ServerIDFor::new();
        assert_eq!(server_id_none.to_string(), "None");
    }

    #[test]
    fn test_server_id_for_serialize_deserialize() {
        use serde_json;

        // Test with Some
        let entity_id = EntityIDFor::<TestEntity>::new("ser-test").unwrap();
        let server_id = ServerIDFor::from_entity_id(entity_id);

        let json = serde_json::to_value(&server_id).unwrap();
        assert_eq!(json, serde_json::json!("TestEntity/ser-test"));

        let deserialized: ServerIDFor<TestEntity> = serde_json::from_value(json).unwrap();
        assert_eq!(server_id, deserialized);

        // Test with None
        let server_id_none: ServerIDFor<TestEntity> = ServerIDFor::new();
        let json_none = serde_json::to_value(&server_id_none).unwrap();
        assert_eq!(json_none, serde_json::json!(null));

        let deserialized_none: ServerIDFor<TestEntity> = serde_json::from_value(json_none).unwrap();
        assert_eq!(server_id_none, deserialized_none);
    }

    #[test]
    fn test_server_id_for_from_instance_property() {
        // Test with string
        let prop =
            InstanceProperty::Primitive(PrimitiveValue::String("TestEntity/prop-test".to_string()));
        let server_id = ServerIDFor::<TestEntity>::from_property(&prop).unwrap();
        assert!(server_id.is_some());
        assert_eq!(server_id.as_ref().unwrap().id(), "prop-test");

        // Test with null
        let prop_null = InstanceProperty::Primitive(PrimitiveValue::Null);
        let server_id_null = ServerIDFor::<TestEntity>::from_property(&prop_null).unwrap();
        assert!(server_id_null.is_none());
    }

    #[test]
    fn test_server_id_for_to_instance_property() {
        let entity_id = EntityIDFor::<TestEntity>::new("to-prop").unwrap();
        let server_id = ServerIDFor::from_entity_id(entity_id);
        let schema = Schema::Class {
            id: "TestEntity".to_string(),
            base: None,
            key: Key::Random,
            documentation: None,
            subdocument: false,
            r#abstract: false,
            inherits: vec![],
            unfoldable: false,
            properties: vec![],
        };

        let prop = <ServerIDFor<TestEntity> as ToInstanceProperty<TestEntity>>::to_property(
            server_id, "id", &schema,
        );
        match prop {
            InstanceProperty::Primitive(PrimitiveValue::String(s)) => {
                assert_eq!(s, "TestEntity/to-prop");
            }
            _ => panic!("Expected string property"),
        }

        // Test with None
        let server_id_none: ServerIDFor<TestEntity> = ServerIDFor::new();
        let prop_none = <ServerIDFor<TestEntity> as ToInstanceProperty<TestEntity>>::to_property(
            server_id_none,
            "id",
            &schema,
        );
        assert_eq!(prop_none, InstanceProperty::Primitive(PrimitiveValue::Null));
    }

    #[test]
    fn test_server_id_for_from_param() {
        use rocket::request::FromParam;

        // Simple ID
        let result = ServerIDFor::<TestEntity>::from_param("simple-id");
        assert!(result.is_ok());
        let server_id = result.unwrap();
        assert_eq!(server_id.as_ref().unwrap().id(), "simple-id");

        // Typed ID
        let result_typed = ServerIDFor::<TestEntity>::from_param("TestEntity/typed-id");
        assert!(result_typed.is_ok());
        let server_id_typed = result_typed.unwrap();
        assert_eq!(server_id_typed.as_ref().unwrap().id(), "typed-id");

        // URL encoded
        let encoded = urlencoding::encode("TestEntity/with space");
        let result_encoded = ServerIDFor::<TestEntity>::from_param(&encoded);
        assert!(result_encoded.is_ok());
        let server_id_encoded = result_encoded.unwrap();
        assert_eq!(server_id_encoded.as_ref().unwrap().id(), "with space");
    }

    #[test]
    fn test_server_id_for_from_form_field() {
        use rocket::form::FromFormField;
        use rocket::form::ValueField;

        // Simple ID
        let field = ValueField {
            name: rocket::form::name::NameView::new("id"),
            value: "form-id",
        };

        let result = ServerIDFor::<TestEntity>::from_value(field);
        assert!(result.is_ok());
        let server_id = result.unwrap();
        assert_eq!(server_id.as_ref().unwrap().id(), "form-id");

        // Wrong type
        let field_wrong = ValueField {
            name: rocket::form::name::NameView::new("id"),
            value: "WrongType/123",
        };

        let result_wrong = ServerIDFor::<TestEntity>::from_value(field_wrong);
        assert!(result_wrong.is_err());
    }
}
