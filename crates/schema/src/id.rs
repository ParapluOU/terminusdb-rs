use crate::iri::TdbIRI;
use crate::json::InstancePropertyFromJson;
use crate::{
    Class, FromInstanceProperty, InstanceProperty, Primitive, PrimitiveValue, Property, Schema,
    TaggedUnion, TaggedUnionVariant, TerminusDBModel, ToInstanceProperty, ToSchemaClass,
    ToSchemaProperty, ToTDBSchema, TypeFamily, STRING, URI,
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

/// Convenience macro that expands to `EntityIDFor<Self>`.
///
/// This macro enables auto-detection of the `id_field` when used on a field named `id`.
/// The derive macro recognizes this pattern and automatically sets the id_field.
///
/// # Example
/// ```ignore
/// use terminusdb_schema_derive::TerminusDBModel;
/// use terminusdb_schema::PrimaryKey;
///
/// #[derive(TerminusDBModel)]
/// struct Person {
///     id: PrimaryKey!(),  // Expands to EntityIDFor<Self>, auto-detected as id_field
///     name: String,
/// }
/// ```
///
/// This is equivalent to:
/// ```ignore
/// #[derive(TerminusDBModel)]
/// struct Person {
///     id: EntityIDFor<Self>,  // Also auto-detected
///     name: String,
/// }
/// ```
#[macro_export]
macro_rules! PrimaryKey {
    () => {
        $crate::EntityIDFor<Self>
    };
}

#[derive(Debug)]
pub struct EntityIDFor<T: ToTDBSchema> {
    /// The parsed IRI
    iri: TdbIRI,
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

/// Helper function to check if a type name is a valid variant for a TaggedUnion,
/// including recursively checking nested TaggedUnions.
///
/// # Arguments
/// * `schema` - The TaggedUnion schema to check against
/// * `type_name` - The type name from the IRI (e.g., "InnerVariantA")
///
/// # Returns
/// `true` if the type_name is a valid variant (directly or nested), `false` otherwise
fn is_valid_tagged_union_variant<T: ToTDBSchema>(schema: &Schema, type_name: &str) -> bool {
    // Get direct variant classes from this TaggedUnion
    let variant_classes: Vec<String> = if let Schema::TaggedUnion { properties, .. } = schema {
        properties.iter().map(|p| p.class.clone()).collect()
    } else {
        return false; // Not a TaggedUnion
    };

    // Check if it's a direct variant
    if variant_classes.iter().any(|v| v == type_name) {
        return true;
    }

    // Check nested TaggedUnions recursively
    for variant_class in &variant_classes {
        // Look up the schema for this variant
        if let Some(variant_schema) = T::find_schema_by_name(variant_class) {
            // If this variant is itself a TaggedUnion, recursively check it
            if variant_schema.is_tagged_union() {
                if is_valid_tagged_union_variant::<T>(&variant_schema, type_name) {
                    return true;
                }
            }
        }
    }

    false
}

impl<T: ToTDBSchema> EntityIDFor<T> {
    pub fn random() -> Self {
        Self::new_unchecked(&Uuid::new_v4().to_string()).unwrap()
    }

    /// Internal constructor that bypasses type validation.
    /// Used for remapping where we want to preserve the IRI structure but change the type parameter.
    fn from_iri_unchecked(iri: TdbIRI) -> Self {
        Self {
            iri,
            _ty: PhantomData,
        }
    }

    /// Constructor without Class constraint, used by new_variant(), deserialization,
    /// and for validating IDs from external sources (like database responses).
    ///
    /// This method still validates TaggedUnion variant types, but doesn't enforce
    /// the Class trait bound that prevents compile-time construction of TaggedUnion IDs
    /// without specifying the variant type.
    pub fn new_unchecked(iri_or_id: &str) -> anyhow::Result<Self> {
        let schema = T::to_schema();

        // Special handling for TaggedUnion types
        if schema.is_tagged_union() {
            // For TaggedUnions, we require the full typed path with variant type prefix
            // Don't auto-prefix since the ID must point to a specific variant instance
            if !iri_or_id.contains('/') {
                return Err(anyhow!(
                    "TaggedUnion '{}' requires a full typed path with variant type prefix (e.g., 'VariantType/id'), got: '{}'",
                    T::schema_name(),
                    iri_or_id
                ));
            }

            // Parse the IRI
            let iri = TdbIRI::parse(iri_or_id)?;

            // Validate that the type name is a valid variant (including nested variants)
            let type_name = iri.type_name();
            if !is_valid_tagged_union_variant::<T>(&schema, type_name) {
                // Get direct variant names for error message
                let variant_classes: Vec<String> =
                    if let Schema::TaggedUnion { properties, .. } = &schema {
                        properties.iter().map(|p| p.class.clone()).collect()
                    } else {
                        vec![]
                    };

                return Err(anyhow!(
                    "Invalid variant type for TaggedUnion '{}': expected one of [{}] or their nested variants, found '{}' in '{}'",
                    T::schema_name(),
                    variant_classes.join(", "),
                    type_name,
                    iri_or_id
                ));
            }

            Ok(Self {
                iri,
                _ty: Default::default(),
            })
        } else {
            // Regular type handling (non-TaggedUnion)
            // If it's a pure ID (no slashes), create a typed path
            let full_iri = if !iri_or_id.contains('/') {
                format!("{}/{}", T::schema_name(), iri_or_id)
            } else {
                iri_or_id.to_string()
            };

            // Parse the IRI
            let iri = TdbIRI::parse(&full_iri)?;

            // Validate that the type name matches
            if iri.type_name() != T::schema_name() {
                return Err(anyhow!(
                    "Mismatched type in IRI: expected '{}', found '{}' in '{}'",
                    T::schema_name(),
                    iri.type_name(),
                    iri_or_id
                ));
            }

            Ok(Self {
                iri,
                _ty: Default::default(),
            })
        }
    }

    /// Create an EntityIDFor from an IRI or ID string.
    ///
    /// **For regular types (structs and simple enums)**: Accepts bare IDs which are auto-prefixed.
    /// ```ignore
    /// let id: EntityIDFor<Person> = EntityIDFor::new_unchecked("123")?;
    /// // Creates "Person/123"
    /// ```
    ///
    /// **For TaggedUnions**: This method is **not available** (compile error).
    /// Use [`new_variant`](EntityIDFor::new_variant) instead for type-safe variant selection.
    pub fn new(iri_or_id: &str) -> anyhow::Result<Self>
    where
        T: Class,
    {
        Self::new_unchecked(iri_or_id)
    }

    /// Create a TaggedUnion EntityIDFor from a bare ID and variant type.
    ///
    /// This is the **only way** to create EntityIDFor values for TaggedUnions,
    /// as `new()` is constrained to Class types only.
    ///
    /// # Type Parameters
    /// - `V`: The variant type (must implement `TaggedUnionVariant<T>`)
    ///
    /// # Examples
    /// ```ignore
    /// // Type-safe variant selection
    /// let id: EntityIDFor<PaymentMethod> =
    ///     EntityIDFor::new_variant::<CreditCard>("cc_123")?;
    /// // Creates "CreditCard/cc_123"
    ///
    /// // Compiler error if variant doesn't belong to union:
    /// // let id: EntityIDFor<PaymentMethod> =
    /// //     EntityIDFor::new_variant::<UnrelatedType>("123")?; // ❌ Won't compile!
    /// ```
    pub fn new_variant<V>(id: &str) -> anyhow::Result<Self>
    where
        T: TaggedUnion,
        V: TaggedUnionVariant<T> + ToSchemaClass,
    {
        let variant_type = V::to_class();
        let full_path = if id.contains('/') {
            id.to_string()
        } else {
            format!("{}/{}", variant_type, id)
        };
        Self::new_unchecked(&full_path)
    }

    /// Create an EntityIDFor for a variant type from a bare ID.
    ///
    /// Convenience method when T is itself a variant type. This auto-prefixes
    /// the ID with the variant's type name.
    ///
    /// # Examples
    /// ```ignore
    /// let id: EntityIDFor<CreditCardVariant> = EntityIDFor::new_for_variant("123")?;
    /// // Creates "CreditCardVariant/123"
    /// ```
    pub fn new_for_variant(id: &str) -> anyhow::Result<Self>
    where
        T: ToSchemaClass,
    {
        let full_path = if id.contains('/') {
            id.to_string()
        } else {
            format!("{}/{}", T::to_class(), id)
        };
        Self::new_unchecked(&full_path)
    }

    /// Get the IRI with default data prefix applied if none is set.
    /// Returns a TdbIRI to keep things typed.
    pub fn iri(&self) -> TdbIRI {
        self.iri.with_default_base()
    }

    /// Get the full IRI as a string (convenience method).
    pub fn iri_string(&self) -> String {
        self.iri().to_string()
    }

    /// Get the parsed IRI object
    pub fn get_iri(&self) -> &TdbIRI {
        &self.iri
    }

    /// Get the base URI if present
    pub fn get_base_uri(&self) -> Option<&str> {
        self.iri.base_uri()
    }

    /// Get the type name
    pub fn get_type_name(&self) -> &str {
        self.iri.type_name()
    }

    /// return just the identifier part (the final ID in the path)
    pub fn id(&self) -> &str {
        self.iri.id()
    }

    /// return MyType/1234 format
    pub fn typed(&self) -> &str {
        self.iri.typed_path()
    }

    pub fn to_string(&self) -> String {
        self.iri.typed_path().to_string()
    }

    pub fn remap<X: ToTDBSchema>(self) -> EntityIDFor<X>
    where
        Self: EntityIDRemap<X>,
    {
        EntityIDRemap::remap(self)
    }
}

/// Trait for remapping EntityIDFor from one type to another.
/// This trait uses specialization to handle TaggedUnion variants differently.
pub trait EntityIDRemap<To: ToTDBSchema> {
    fn remap(self) -> EntityIDFor<To>;
}

// Default implementation: intelligently remap based on whether the ID contains variant/type information
impl<T: ToTDBSchema, To: ToTDBSchema> EntityIDRemap<To> for EntityIDFor<T> {
    default fn remap(self) -> EntityIDFor<To> {
        let type_name = self.get_type_name();
        let source_schema = T::to_schema();
        let source_type_name = source_schema.class_name().as_str();

        // If the type name in the IRI doesn't match the source type T, it means we have
        // a variant/different type embedded (e.g., EntityIDFor<Union> containing "VariantType/id")
        // In this case, preserve the full IRI structure to maintain that type information
        if type_name != source_type_name {
            EntityIDFor::from_iri_unchecked(self.iri)
        } else {
            // Normal case: the IRI matches the source type, so just extract the ID
            // and let it be re-prefixed with the target type name
            EntityIDFor::new_unchecked(self.id()).unwrap()
        }
    }
}

// Specialized implementation: variant→union remapping preserves full typed path and base URI
impl<T: TaggedUnionVariant<U>, U: TaggedUnion> EntityIDRemap<U> for EntityIDFor<T> {
    fn remap(self) -> EntityIDFor<U> {
        // Preserve the full IRI structure (base URI + variant type + ID)
        EntityIDFor::from_iri_unchecked(self.iri)
    }
}

impl<T: ToTDBSchema> PartialEq<str> for EntityIDFor<T> {
    fn eq(&self, other: &str) -> bool {
        self.iri.typed_path() == other
    }
}

impl<T: ToTDBSchema> PartialEq<&str> for EntityIDFor<T> {
    fn eq(&self, other: &&str) -> bool {
        self.typed() == *other
    }
}

impl<T: ToTDBSchema> PartialEq<String> for EntityIDFor<T> {
    fn eq(&self, other: &String) -> bool {
        self.typed() == other.as_str()
    }
}

impl<T: ToTDBSchema> PartialEq<EntityIDFor<T>> for String {
    fn eq(&self, other: &EntityIDFor<T>) -> bool {
        self.as_str() == other.typed()
    }
}

impl<T: ToTDBSchema> PartialEq<EntityIDFor<T>> for &str {
    fn eq(&self, other: &EntityIDFor<T>) -> bool {
        *self == other.typed()
    }
}

impl<T: ToTDBSchema> Default for EntityIDFor<T> {
    fn default() -> Self {
        Self::random()
    }
}

impl<T: ToTDBSchema> Hash for EntityIDFor<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.iri.to_string().hash(state);
    }
}

impl<T: ToTDBSchema> PartialEq<EntityIDFor<T>> for EntityIDFor<T> {
    fn eq(&self, other: &Self) -> bool {
        self.iri == other.iri
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
        self.iri.to_string().cmp(&other.iri.to_string())
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
        Self::new_unchecked(&value.to_string()).unwrap()
    }
}

impl<T: ToTDBSchema> TryInto<EntityIDFor<T>> for String {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<EntityIDFor<T>, Self::Error> {
        EntityIDFor::new_unchecked(&self)
    }
}

impl<T: ToTDBSchema> TryInto<EntityIDFor<T>> for &String {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<EntityIDFor<T>, Self::Error> {
        EntityIDFor::new_unchecked(self)
    }
}

impl<T: ToTDBSchema> TryInto<EntityIDFor<T>> for &str {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<EntityIDFor<T>, Self::Error> {
        EntityIDFor::new_unchecked(self)
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
        Self::new_unchecked(&s).map_err(serde::de::Error::custom)
    }
}

impl<T: ToTDBSchema> Clone for EntityIDFor<T> {
    fn clone(&self) -> Self {
        Self {
            iri: self.iri.clone(),
            _ty: Default::default(),
        }
    }
}

impl<T: ToTDBSchema> Deref for EntityIDFor<T> {
    type Target = str;

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
            InstanceProperty::Primitive(PrimitiveValue::String(id)) => Self::new_unchecked(id),
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
        Self::new_unchecked(&decoded)
    }
}

// Implement FromFormField for form submissions
impl<'r, T: ToTDBSchema + Send> FromFormField<'r> for EntityIDFor<T> {
    fn from_value(field: ValueField<'r>) -> form::Result<'r, Self> {
        // Use the existing new() method to parse the value
        match Self::new_unchecked(field.value) {
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
                let entity_id = EntityIDFor::new_unchecked(id)?;
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
        let entity_id = EntityIDFor::new_unchecked(&decoded)?;
        Ok(Self {
            inner: Some(entity_id),
        })
    }
}

// Rocket FromFormField implementation
impl<'r, T: ToTDBSchema + Send + Class> FromFormField<'r> for ServerIDFor<T> {
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
    use crate as terminusdb_schema;
    use crate::*;
    use serde::{Deserialize, Serialize};
    use terminusdb_schema_derive::TerminusDBModel;

    // Define a dummy struct for testing
    #[derive(Clone, Debug, Default, TerminusDBModel)]
    struct TestEntity {
        nothing: String,
    }

    #[test]
    fn test_parse_simple_id() {
        let entity_id: EntityIDFor<TestEntity> = EntityIDFor::new_unchecked("1234").unwrap();
        assert_eq!(entity_id.id(), "1234");
        assert_eq!(entity_id.typed(), "TestEntity/1234");
        assert_eq!(entity_id.get_base_uri(), None);
    }

    #[test]
    fn test_parse_typed_id() {
        let entity_id: EntityIDFor<TestEntity> = EntityIDFor::new_unchecked("TestEntity/5678").unwrap();
        assert_eq!(entity_id.id(), "5678");
        assert_eq!(entity_id.typed(), "TestEntity/5678");
        assert_eq!(entity_id.get_base_uri(), None);
    }

    // Test case for IRI
    #[test]
    // #[should_panic] // Expected to panic until implemented
    fn test_parse_iri() {
        let iri = "terminusdb://data#TestEntity/91011";
        let entity_id: EntityIDFor<TestEntity> = EntityIDFor::new_unchecked(iri).unwrap();
        // Add assertions here once IRI parsing is implemented
        assert_eq!(entity_id.id(), "91011");
        assert_eq!(entity_id.get_base_uri(), Some("terminusdb://data"));
        assert_eq!(entity_id.typed(), "TestEntity/91011"); // typed() should ignore base
    }

    #[test]
    fn test_parse_iri_wrong_type() {
        let iri = "terminusdb://data#WrongType/91011";
        let result: Result<EntityIDFor<TestEntity>, _> = EntityIDFor::new_unchecked(iri);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Mismatched type in IRI"));
    }

    #[test]
    fn test_parse_typed_id_wrong_type() {
        let typed_id = "WrongType/5678";
        let result: Result<EntityIDFor<TestEntity>, _> = EntityIDFor::new_unchecked(typed_id);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Mismatched type in IRI"));
    }

    #[test]
    fn test_parse_path_based_iri() {
        let iri = "terminusdb:///data/TestEntity/7aec78a3-9749-457e-a113-273df5edf156";
        let entity_id: EntityIDFor<TestEntity> = EntityIDFor::new_unchecked(iri).unwrap();
        assert_eq!(entity_id.id(), "7aec78a3-9749-457e-a113-273df5edf156");
        assert_eq!(
            entity_id.typed(),
            "TestEntity/7aec78a3-9749-457e-a113-273df5edf156"
        );
        assert_eq!(entity_id.get_base_uri(), Some("terminusdb:///data"));
    }

    #[test]
    fn test_parse_path_based_iri_wrong_type() {
        let iri = "terminusdb:///data/WrongType/7aec78a3-9749-457e-a113-273df5edf156";
        let result: Result<EntityIDFor<TestEntity>, _> = EntityIDFor::new_unchecked(iri);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Mismatched type in IRI"));
    }

    #[test]
    fn test_iri_returns_typed_iri_with_base() {
        // EntityIDFor created from bare ID should return TdbIRI with default base
        let entity_id: EntityIDFor<TestEntity> = EntityIDFor::new("123").unwrap();
        let iri = entity_id.iri();
        assert_eq!(iri.base_uri(), Some("terminusdb:///data"));
        assert_eq!(iri.to_string(), "terminusdb:///data/TestEntity/123");

        // EntityIDFor from full IRI should preserve original base
        let entity_id2: EntityIDFor<TestEntity> =
            EntityIDFor::new_unchecked("terminusdb:///data/TestEntity/456").unwrap();
        let iri2 = entity_id2.iri();
        assert_eq!(iri2.base_uri(), Some("terminusdb:///data"));
        assert_eq!(iri2.to_string(), "terminusdb:///data/TestEntity/456");
    }

    #[test]
    fn test_iri_string_convenience_method() {
        let entity_id: EntityIDFor<TestEntity> = EntityIDFor::new("789").unwrap();
        // iri_string() should return the full IRI as a String
        assert_eq!(
            entity_id.iri_string(),
            "terminusdb:///data/TestEntity/789"
        );
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

    #[test]
    fn test_parse_subdocument_path() {
        use crate as terminusdb_schema;

        // Define a subdocument type for testing
        #[derive(Clone, Debug, TerminusDBModel)]
        #[tdb(subdocument)]
        struct ReviewSessionAssignment {
            _dummy: String,
        }

        // Test simple subdocument path
        let path = "ReviewSession/e31e8079-6d1a-4ffc-ae85-73b4d298bb3f/review_assignments/ReviewSessionAssignment/IQ79UxtoVR6W0ASI";
        let entity_id: EntityIDFor<ReviewSessionAssignment> = EntityIDFor::new_unchecked(path).unwrap();
        assert_eq!(entity_id.id(), "IQ79UxtoVR6W0ASI");
        assert_eq!(entity_id.typed(), path);
        assert_eq!(entity_id.get_base_uri(), None);
    }

    #[test]
    fn test_parse_nested_subdocument_path() {
        // Test deeply nested subdocument path
        let path = "Parent/123/child_prop/Child/456/grandchild_prop/TestEntity/789";
        let entity_id: EntityIDFor<TestEntity> = EntityIDFor::new_unchecked(path).unwrap();
        assert_eq!(entity_id.id(), "789");
        assert_eq!(entity_id.typed(), path);
        assert_eq!(entity_id.get_base_uri(), None);
    }

    #[test]
    fn test_parse_subdocument_path_wrong_type() {
        // Should fail if final type doesn't match
        let path = "ReviewSession/e31e8079/review_assignments/WrongType/IQ79Ux";
        let result: Result<EntityIDFor<TestEntity>, _> = EntityIDFor::new_unchecked(path);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Mismatched type in IRI"));
    }

    #[test]
    fn test_parse_subdocument_iri_fragment() {
        use crate as terminusdb_schema;

        // Define the subdoc type
        #[derive(Clone, Debug, TerminusDBModel)]
        #[tdb(subdocument)]
        struct SubDoc {
            _dummy: String,
        }

        let iri = "terminusdb://data#Parent/123/prop/SubDoc/456";
        let entity_id: EntityIDFor<SubDoc> = EntityIDFor::new_unchecked(iri).unwrap();
        assert_eq!(entity_id.id(), "456");
        assert_eq!(entity_id.typed(), "Parent/123/prop/SubDoc/456");
        assert_eq!(entity_id.get_base_uri(), Some("terminusdb://data"));
    }

    #[test]
    fn test_parse_subdocument_iri_path() {
        let iri = "terminusdb:///data/ReviewSession/123/assignments/TestEntity/789";
        let entity_id: EntityIDFor<TestEntity> = EntityIDFor::new_unchecked(iri).unwrap();
        assert_eq!(entity_id.id(), "789");
        assert_eq!(
            entity_id.typed(),
            "ReviewSession/123/assignments/TestEntity/789"
        );
        assert_eq!(entity_id.get_base_uri(), Some("terminusdb:///data"));
    }

    // Define a TaggedUnion for testing
    #[derive(Clone, Debug, TerminusDBModel)]
    enum TestTaggedUnion {
        VariantA { value: String },
        VariantB { count: i32 },
    }

    #[test]
    fn test_new_variant_creates_valid_id() {
        let result = EntityIDFor::<TestTaggedUnion>::new_variant::<TestTaggedUnionVariantA>("123");
        assert!(result.is_ok());
        let entity_id = result.unwrap();
        assert_eq!(entity_id.id(), "123");
        assert_eq!(entity_id.get_type_name(), "TestTaggedUnionVariantA");
    }

    #[test]
    fn test_new_variant_other_variant() {
        let result = EntityIDFor::<TestTaggedUnion>::new_variant::<TestTaggedUnionVariantB>("456");
        assert!(result.is_ok());
        let entity_id = result.unwrap();
        assert_eq!(entity_id.id(), "456");
        assert_eq!(entity_id.get_type_name(), "TestTaggedUnionVariantB");
    }

    #[test]
    fn test_new_variant_accepts_full_path() {
        let result = EntityIDFor::<TestTaggedUnion>::new_variant::<TestTaggedUnionVariantA>("TestTaggedUnionVariantA/123");
        assert!(result.is_ok());
        let entity_id = result.unwrap();
        assert_eq!(entity_id.id(), "123");
        assert_eq!(entity_id.get_type_name(), "TestTaggedUnionVariantA");
    }

    #[test]
    fn test_new_variant_with_iri() {
        let result = EntityIDFor::<TestTaggedUnion>::new_variant::<TestTaggedUnionVariantA>("terminusdb://data#TestTaggedUnionVariantA/789");
        assert!(result.is_ok());
        let entity_id = result.unwrap();
        assert_eq!(entity_id.id(), "789");
        assert_eq!(entity_id.get_type_name(), "TestTaggedUnionVariantA");
        assert_eq!(entity_id.get_base_uri(), Some("terminusdb://data"));
    }

    #[test]
    fn test_new_unchecked_still_validates_tagged_unions() {
        // new_unchecked should still validate variant types for TaggedUnions
        let result = EntityIDFor::<TestTaggedUnion>::new_unchecked("123");
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("TaggedUnion"));
    }

    #[test]
    fn test_new_unchecked_accepts_valid_variant() {
        let result = EntityIDFor::<TestTaggedUnion>::new_unchecked("TestTaggedUnionVariantA/123");
        assert!(result.is_ok());
        let entity_id = result.unwrap();
        assert_eq!(entity_id.id(), "123");
        assert_eq!(entity_id.get_type_name(), "TestTaggedUnionVariantA");
    }

    // TestTaggedUnionVariantA and TestTaggedUnionVariantB are now auto-generated by the derive macro
    // TaggedUnion and TaggedUnionVariant marker traits are now auto-implemented by the derive macro

    #[test]
    fn test_remap_regular_types() {
        use crate as terminusdb_schema;

        // Test that regular type remapping still works (backward compatibility)
        let entity_id: EntityIDFor<TestEntity> = EntityIDFor::new_unchecked("TestEntity/123").unwrap();

        // Define another regular type
        #[derive(Clone, Debug, TerminusDBModel)]
        struct OtherEntity {
            _dummy: String,
        }

        // Remap to another type - should use just the ID
        let remapped: EntityIDFor<OtherEntity> = entity_id.remap();
        assert_eq!(remapped.id(), "123");
        assert_eq!(remapped.typed(), "OtherEntity/123");
    }

    #[test]
    fn test_remap_variant_to_union() {
        // Create an EntityIDFor a variant type with full typed path
        let variant_id: EntityIDFor<TestTaggedUnionVariantA> =
            EntityIDFor::new_unchecked("TestTaggedUnionVariantA/456").unwrap();

        assert_eq!(variant_id.id(), "456");
        assert_eq!(variant_id.typed(), "TestTaggedUnionVariantA/456");

        // Remap to the union type - should preserve the full typed path
        let union_id: EntityIDFor<TestTaggedUnion> = variant_id.remap();

        // The union ID should preserve the variant type in the path
        assert_eq!(union_id.id(), "456");
        assert_eq!(union_id.typed(), "TestTaggedUnionVariantA/456");
        assert_eq!(union_id.get_type_name(), "TestTaggedUnionVariantA");
    }

    #[test]
    fn test_remap_variant_to_union_with_base_uri() {
        // Test remapping with full IRI including base URI
        let variant_id: EntityIDFor<TestTaggedUnionVariantA> =
            EntityIDFor::new_unchecked("terminusdb://data#TestTaggedUnionVariantA/789").unwrap();

        assert_eq!(variant_id.id(), "789");
        assert_eq!(variant_id.get_base_uri(), Some("terminusdb://data"));

        // Remap to union
        let union_id: EntityIDFor<TestTaggedUnion> = variant_id.remap();

        // Should preserve full path including base URI
        assert_eq!(union_id.id(), "789");
        assert_eq!(union_id.get_type_name(), "TestTaggedUnionVariantA");
        assert_eq!(union_id.get_base_uri(), Some("terminusdb://data"));
    }

    #[test]
    fn test_remap_union_to_regular_type_preserves_variant() {
        use crate as terminusdb_schema;

        // Create a TaggedUnion ID that contains a variant type
        let union_id: EntityIDFor<TestTaggedUnion> =
            EntityIDFor::new_unchecked("TestTaggedUnionVariantA/789").unwrap();

        assert_eq!(union_id.typed(), "TestTaggedUnionVariantA/789");
        assert_eq!(union_id.id(), "789");
        assert_eq!(union_id.get_type_name(), "TestTaggedUnionVariantA");

        // Define another regular type
        #[derive(Clone, Debug, TerminusDBModel)]
        struct TargetType {
            _dummy: String,
        }

        // Remap to another type - should preserve the full variant path
        let remapped: EntityIDFor<TargetType> = union_id.remap();

        // Should preserve the full variant path, not just the ID
        assert_eq!(remapped.typed(), "TestTaggedUnionVariantA/789");
        assert_eq!(remapped.id(), "789");
        assert_eq!(remapped.get_type_name(), "TestTaggedUnionVariantA");
    }

    #[test]
    fn test_remap_union_with_base_uri_to_regular_type() {
        use crate as terminusdb_schema;

        // Create a TaggedUnion ID with base URI
        let union_id: EntityIDFor<TestTaggedUnion> =
            EntityIDFor::new_unchecked("terminusdb://data#TestTaggedUnionVariantB/456").unwrap();

        assert_eq!(union_id.get_base_uri(), Some("terminusdb://data"));
        assert_eq!(union_id.get_type_name(), "TestTaggedUnionVariantB");

        #[derive(Clone, Debug, TerminusDBModel)]
        struct AnotherType {
            _dummy: String,
        }

        // Remap to another type
        let remapped: EntityIDFor<AnotherType> = union_id.remap();

        // Should preserve base URI and variant type
        assert_eq!(
            remapped.iri().to_string(),
            "terminusdb://data#TestTaggedUnionVariantB/456"
        );
        assert_eq!(remapped.get_base_uri(), Some("terminusdb://data"));
        assert_eq!(remapped.get_type_name(), "TestTaggedUnionVariantB");
        assert_eq!(remapped.id(), "456");
    }

    #[test]
    fn test_remap_union_to_union_preserves_variant() {
        use crate as terminusdb_schema;

        // Create a union with a variant type embedded
        let union1_id: EntityIDFor<TestTaggedUnion> =
            EntityIDFor::new_unchecked("TestTaggedUnionVariantA/999").unwrap();

        #[derive(Clone, Debug, TerminusDBModel)]
        enum AnotherUnion {
            X { x: String },
            Y { y: String },
        }

        // Remap to a different union type
        let union2_id: EntityIDFor<AnotherUnion> = union1_id.remap();

        // Should preserve the original variant type from the first union
        assert_eq!(union2_id.typed(), "TestTaggedUnionVariantA/999");
        assert_eq!(union2_id.get_type_name(), "TestTaggedUnionVariantA");
        assert_eq!(union2_id.id(), "999");
    }

    #[test]
    fn test_partial_eq_with_string_and_str() {
        let entity_id: EntityIDFor<TestEntity> = EntityIDFor::new_unchecked("1234").unwrap();

        // Test comparison with &str
        assert_eq!(entity_id, "TestEntity/1234");
        assert_eq!("TestEntity/1234", entity_id);

        // Test comparison with String
        let owned_string = String::from("TestEntity/1234");
        assert_eq!(entity_id, owned_string);
        assert_eq!(owned_string, entity_id);

        // Test inequality
        assert_ne!(entity_id, "TestEntity/5678");
        assert_ne!("TestEntity/5678", entity_id);
        assert_ne!(entity_id, String::from("TestEntity/5678"));
        assert_ne!(String::from("TestEntity/5678"), entity_id);
    }

    // ========== Nested TaggedUnion Tests ==========

    #[derive(Clone, Debug, TerminusDBModel)]
    struct InnerVariantA {
        value_a: String,
    }

    #[derive(Clone, Debug, TerminusDBModel)]
    struct InnerVariantB {
        value_b: String,
    }

    #[derive(Clone, Debug, TerminusDBModel)]
    enum InnerUnion {
        A(InnerVariantA),
        B(InnerVariantB),
    }

    #[derive(Clone, Debug, TerminusDBModel)]
    struct OuterVariantC {
        value_c: String,
    }

    #[derive(Clone, Debug, TerminusDBModel)]
    enum OuterUnion {
        Inner(InnerUnion),
        C(OuterVariantC),
    }

    #[test]
    fn test_nested_tagged_union_new_variant_direct() {
        // Test that direct variants work with new_variant()
        let entity_id = EntityIDFor::<OuterUnion>::new_variant::<OuterVariantC>("test-id").unwrap();
        assert_eq!(entity_id.id(), "test-id");
        assert_eq!(entity_id.typed(), "OuterVariantC/test-id");
    }

    #[test]
    fn test_nested_tagged_union_new_variant_inner() {
        // For nested unions, variants only implement TaggedUnionVariant for their direct parent union.
        // InnerVariantA implements TaggedUnionVariant<InnerUnion>, but NOT TaggedUnionVariant<OuterUnion>.
        // So you must use the InnerUnion type when creating IDs with new_variant().
        let entity_id = EntityIDFor::<InnerUnion>::new_variant::<InnerVariantA>("test-id-a").unwrap();
        assert_eq!(entity_id.id(), "test-id-a");
        assert_eq!(entity_id.typed(), "InnerVariantA/test-id-a");

        // The InnerUnion ID can then be remapped to OuterUnion
        let outer_id: EntityIDFor<OuterUnion> = entity_id.remap();
        assert_eq!(outer_id.id(), "test-id-a");
        assert_eq!(outer_id.typed(), "InnerVariantA/test-id-a");
    }

    #[test]
    fn test_nested_tagged_union_direct_variant() {
        // Test that direct variants still work with new_unchecked
        let entity_id: EntityIDFor<OuterUnion> =
            EntityIDFor::new_unchecked("OuterVariantC/test-id").unwrap();
        assert_eq!(entity_id.id(), "test-id");
        assert_eq!(entity_id.typed(), "OuterVariantC/test-id");
    }

    #[test]
    fn test_nested_tagged_union_inner_variant_a() {
        // Test that nested variant A is accepted
        let entity_id: EntityIDFor<OuterUnion> =
            EntityIDFor::new_unchecked("InnerVariantA/test-id-a").unwrap();
        assert_eq!(entity_id.id(), "test-id-a");
        assert_eq!(entity_id.typed(), "InnerVariantA/test-id-a");
    }

    #[test]
    fn test_nested_tagged_union_inner_variant_b() {
        // Test that nested variant B is accepted
        let entity_id: EntityIDFor<OuterUnion> =
            EntityIDFor::new_unchecked("InnerVariantB/test-id-b").unwrap();
        assert_eq!(entity_id.id(), "test-id-b");
        assert_eq!(entity_id.typed(), "InnerVariantB/test-id-b");
    }

    #[test]
    fn test_nested_tagged_union_with_iri() {
        // Test nested variants work with full IRIs
        let iri = "terminusdb:///data/InnerVariantA/abc-123";
        let entity_id: EntityIDFor<OuterUnion> = EntityIDFor::new_unchecked(iri).unwrap();
        assert_eq!(entity_id.id(), "abc-123");
        assert_eq!(entity_id.typed(), "InnerVariantA/abc-123");
        assert_eq!(entity_id.get_base_uri(), Some("terminusdb:///data"));
    }

    #[test]
    fn test_nested_tagged_union_remap() {
        // Test that remapping works with nested unions
        let inner_iri = "terminusdb:///data/InnerVariantA/remap-test";

        // Create as InnerUnion
        let inner_id: EntityIDFor<InnerUnion> = EntityIDFor::new_unchecked(inner_iri).unwrap();
        assert_eq!(inner_id.id(), "remap-test");

        // Remap to OuterUnion should work
        let outer_id: EntityIDFor<OuterUnion> = inner_id.remap();
        assert_eq!(outer_id.id(), "remap-test");
        assert_eq!(outer_id.typed(), "InnerVariantA/remap-test");
    }

    #[test]
    fn test_nested_tagged_union_invalid_variant() {
        // Test that truly invalid variants are still rejected
        let result: Result<EntityIDFor<OuterUnion>, _> =
            EntityIDFor::new_unchecked("CompletelyInvalidType/test-id");
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Invalid variant type"));
        assert!(err_msg.contains("CompletelyInvalidType"));
    }

    #[test]
    fn test_nested_tagged_union_unprefixed_rejected() {
        // TaggedUnions should still require typed paths
        let result: Result<EntityIDFor<OuterUnion>, _> = EntityIDFor::new_unchecked("just-an-id");
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("requires a full typed path"));
    }

    // Test with three levels of nesting
    #[derive(Clone, Debug, TerminusDBModel)]
    struct DeepVariantX {
        value_x: String,
    }

    #[derive(Clone, Debug, TerminusDBModel)]
    struct DeepVariantY {
        value_y: String,
    }

    #[derive(Clone, Debug, TerminusDBModel)]
    enum DeepInnerUnion {
        X(DeepVariantX),
        Y(DeepVariantY),
    }

    #[derive(Clone, Debug, TerminusDBModel)]
    enum DeepMiddleUnion {
        Deep(DeepInnerUnion),
    }

    #[derive(Clone, Debug, TerminusDBModel)]
    enum DeepOuterUnion {
        Middle(DeepMiddleUnion),
    }

    #[test]
    fn test_three_level_nested_tagged_union() {
        // Test that three levels of nesting works
        let entity_id: EntityIDFor<DeepOuterUnion> =
            EntityIDFor::new_unchecked("DeepVariantX/deep-test").unwrap();
        assert_eq!(entity_id.id(), "deep-test");
        assert_eq!(entity_id.typed(), "DeepVariantX/deep-test");

        // Also test the other variant
        let entity_id_y: EntityIDFor<DeepOuterUnion> =
            EntityIDFor::new_unchecked("DeepVariantY/deep-test-y").unwrap();
        assert_eq!(entity_id_y.id(), "deep-test-y");
        assert_eq!(entity_id_y.typed(), "DeepVariantY/deep-test-y");
    }

    #[test]
    fn test_three_level_nested_remap() {
        // Test remapping across three levels
        let iri = "terminusdb:///data/DeepVariantX/triple-remap";

        // Start from innermost
        let inner_id: EntityIDFor<DeepInnerUnion> = EntityIDFor::new_unchecked(iri).unwrap();

        // Remap to middle
        let middle_id: EntityIDFor<DeepMiddleUnion> = inner_id.remap();
        assert_eq!(middle_id.id(), "triple-remap");

        // Remap to outer
        let outer_id: EntityIDFor<DeepOuterUnion> = middle_id.remap();
        assert_eq!(outer_id.id(), "triple-remap");
        assert_eq!(outer_id.typed(), "DeepVariantX/triple-remap");
    }
}
