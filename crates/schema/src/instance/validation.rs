use crate::{Instance, InstanceProperty, PrimitiveValue, RelationValue, Schema, SetCardinality, TypeFamily};
use std::collections::{BTreeMap, BTreeSet};

/// Errors that can occur during instance validation against a schema
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationError {
    /// Property exists in instance but not defined in schema
    UnknownProperty {
        property: String,
        class: String,
    },
    /// Required property missing from instance
    MissingProperty {
        property: String,
        class: String,
    },
    /// Property type doesn't match schema (e.g., primitive vs relation)
    PropertyTypeMismatch {
        property: String,
        expected: String,
        actual: String,
    },
    /// Invalid enum value
    InvalidEnumValue {
        class: String,
        value: String,
        valid_values: Vec<String>,
    },
    /// Set cardinality constraint violation
    SetCardinalityViolation {
        property: String,
        constraint: String,
        actual_count: usize,
    },
    /// Array dimension mismatch
    ArrayDimensionMismatch {
        property: String,
        expected_dimensions: usize,
        actual: String,
    },
    /// Type family mismatch (e.g., single value when List expected)
    TypeFamilyMismatch {
        property: String,
        expected_family: String,
        actual: String,
    },
    /// Nested instance validation failed
    NestedInstanceError {
        property: String,
        errors: Vec<ValidationError>,
    },
    /// Schema mismatch between instance.schema and actual data
    SchemaMismatch {
        expected: String,
        actual: String,
    },
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationError::UnknownProperty { property, class } => {
                write!(f, "Unknown property '{}' for class '{}'", property, class)
            }
            ValidationError::MissingProperty { property, class } => {
                write!(f, "Missing required property '{}' for class '{}'", property, class)
            }
            ValidationError::PropertyTypeMismatch { property, expected, actual } => {
                write!(f, "Property '{}': expected {}, got {}", property, expected, actual)
            }
            ValidationError::InvalidEnumValue { class, value, valid_values } => {
                write!(f, "Invalid enum value '{}' for enum '{}'. Valid values: {:?}", value, class, valid_values)
            }
            ValidationError::SetCardinalityViolation { property, constraint, actual_count } => {
                write!(f, "Property '{}': set cardinality {} violated (actual count: {})", property, constraint, actual_count)
            }
            ValidationError::ArrayDimensionMismatch { property, expected_dimensions, actual } => {
                write!(f, "Property '{}': expected {}-dimensional array, got {}", property, expected_dimensions, actual)
            }
            ValidationError::TypeFamilyMismatch { property, expected_family, actual } => {
                write!(f, "Property '{}': expected type family {}, got {}", property, expected_family, actual)
            }
            ValidationError::NestedInstanceError { property, errors } => {
                write!(f, "Property '{}': nested validation errors: {:?}", property, errors)
            }
            ValidationError::SchemaMismatch { expected, actual } => {
                write!(f, "Schema mismatch: expected '{}', instance has '{}'", expected, actual)
            }
        }
    }
}

impl std::error::Error for ValidationError {}

/// Result type for validation operations
pub type ValidationResult = Result<(), Vec<ValidationError>>;

/// Validates an instance against a schema
///
/// Returns `Ok(())` if the instance is valid, or `Err(Vec<ValidationError>)` containing
/// all validation errors found.
///
/// # Examples
///
/// ```rust,ignore
/// use terminusdb_schema::{validate_instance, Instance, Schema};
///
/// let schema = MyType::to_schema();
/// let instance = my_value.to_tdb_instance();
///
/// match validate_instance(&instance, &schema) {
///     Ok(()) => println!("Instance is valid!"),
///     Err(errors) => {
///         for error in errors {
///             eprintln!("Validation error: {}", error);
///         }
///     }
/// }
/// ```
pub fn validate_instance(instance: &Instance, schema: &Schema) -> ValidationResult {
    let mut errors = Vec::new();

    // Validate schema match
    if instance.schema.class_name() != schema.class_name() {
        errors.push(ValidationError::SchemaMismatch {
            expected: schema.class_name().to_string(),
            actual: instance.schema.class_name().to_string(),
        });
        return Err(errors);
    }

    // Validate based on schema type
    match schema {
        Schema::Enum { id, values, .. } => {
            validate_enum_instance(instance, id, values, &mut errors);
        }
        Schema::Class { properties, .. } | Schema::TaggedUnion { properties, .. } => {
            validate_class_instance(instance, properties, schema, &mut errors);
        }
        Schema::OneOfClass { classes, .. } => {
            validate_oneof_instance(instance, classes, schema, &mut errors);
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Validates an enum instance
fn validate_enum_instance(
    instance: &Instance,
    class_name: &str,
    valid_values: &[String],
    errors: &mut Vec<ValidationError>,
) {
    if let Some(enum_val) = instance.enum_value() {
        if !valid_values.contains(&enum_val.to_string()) {
            errors.push(ValidationError::InvalidEnumValue {
                class: class_name.to_string(),
                value: enum_val.to_string(),
                valid_values: valid_values.to_vec(),
            });
        }
    } else {
        errors.push(ValidationError::PropertyTypeMismatch {
            property: "@value".to_string(),
            expected: "enum value".to_string(),
            actual: "not an enum".to_string(),
        });
    }
}

/// Validates a class or tagged union instance
fn validate_class_instance(
    instance: &Instance,
    properties: &[crate::Property],
    schema: &Schema,
    errors: &mut Vec<ValidationError>,
) {
    let class_name = schema.class_name();
    let property_map: BTreeMap<&str, &crate::Property> =
        properties.iter().map(|p| (p.name.as_str(), p)).collect();

    // Check for unknown properties
    for prop_name in instance.properties.keys() {
        if !property_map.contains_key(prop_name.as_str()) {
            errors.push(ValidationError::UnknownProperty {
                property: prop_name.clone(),
                class: class_name.to_string(),
            });
        }
    }

    // Check required properties and validate property types
    for property in properties {
        let is_optional = matches!(property.r#type, Some(TypeFamily::Optional));

        match instance.properties.get(&property.name) {
            None if !is_optional => {
                errors.push(ValidationError::MissingProperty {
                    property: property.name.clone(),
                    class: class_name.to_string(),
                });
            }
            Some(instance_prop) => {
                validate_property(instance_prop, property, errors);
            }
            _ => {}
        }
    }
}

/// Validates a OneOfClass instance
fn validate_oneof_instance(
    instance: &Instance,
    classes: &[BTreeSet<crate::Property>],
    schema: &Schema,
    errors: &mut Vec<ValidationError>,
) {
    let class_name = schema.class_name();
    let mut matched_any = false;

    // Check if instance matches any of the OneOf variants
    for variant_props in classes {
        let variant_vec: Vec<_> = variant_props.iter().cloned().collect();
        let mut variant_errors = Vec::new();
        validate_class_instance(instance, &variant_vec, schema, &mut variant_errors);

        if variant_errors.is_empty() {
            matched_any = true;
            break;
        }
    }

    if !matched_any {
        errors.push(ValidationError::PropertyTypeMismatch {
            property: "instance".to_string(),
            expected: format!("one of {} variants", classes.len()),
            actual: "none matched".to_string(),
        });
    }
}

/// Validates a single property value against its schema definition
fn validate_property(
    instance_prop: &InstanceProperty,
    property: &crate::Property,
    errors: &mut Vec<ValidationError>,
) {
    let prop_name = &property.name;
    let type_family = &property.r#type;

    match (type_family, instance_prop) {
        // Optional type family
        (Some(TypeFamily::Optional), _) => {
            // Optional can be any value or missing, so we validate the inner value
            validate_property_type(instance_prop, property, errors);
        }

        // List type family
        (Some(TypeFamily::List), InstanceProperty::Primitives(vals)) => {
            for _ in vals {
                // Could add type checking here if needed
            }
        }
        (Some(TypeFamily::List), InstanceProperty::Relations(vals)) => {
            for rel in vals {
                validate_relation_value(rel, prop_name, errors);
            }
        }
        (Some(TypeFamily::List), _) if !matches!(instance_prop, InstanceProperty::Primitive(_) | InstanceProperty::Relation(_)) => {
            // Single values are not allowed for List
            errors.push(ValidationError::TypeFamilyMismatch {
                property: prop_name.clone(),
                expected_family: "List".to_string(),
                actual: format!("{:?}", instance_prop),
            });
        }

        // Set type family
        (Some(TypeFamily::Set(cardinality)), InstanceProperty::Primitives(vals)) => {
            validate_set_cardinality(prop_name, cardinality, vals.len(), errors);
        }
        (Some(TypeFamily::Set(cardinality)), InstanceProperty::Relations(vals)) => {
            validate_set_cardinality(prop_name, cardinality, vals.len(), errors);
            for rel in vals {
                validate_relation_value(rel, prop_name, errors);
            }
        }
        (Some(TypeFamily::Set(_)), _) if !matches!(instance_prop, InstanceProperty::Primitive(_) | InstanceProperty::Relation(_)) => {
            errors.push(ValidationError::TypeFamilyMismatch {
                property: prop_name.clone(),
                expected_family: "Set".to_string(),
                actual: format!("{:?}", instance_prop),
            });
        }

        // Array type family
        (Some(TypeFamily::Array(dimensions)), _) => {
            validate_array_dimensions(instance_prop, prop_name, *dimensions, errors);
        }

        // No type family - simple single value
        (None, InstanceProperty::Primitive(_)) => {
            validate_property_type(instance_prop, property, errors);
        }
        (None, InstanceProperty::Relation(rel)) => {
            validate_property_type(instance_prop, property, errors);
            validate_relation_value(rel, prop_name, errors);
        }
        (None, InstanceProperty::Primitives(_)) | (None, InstanceProperty::Relations(_)) => {
            errors.push(ValidationError::TypeFamilyMismatch {
                property: prop_name.clone(),
                expected_family: "single value".to_string(),
                actual: "multiple values".to_string(),
            });
        }

        // Allow List with single value (common case)
        (Some(TypeFamily::List), InstanceProperty::Primitive(_)) => {
            // Single primitive in a List is OK
        }
        (Some(TypeFamily::List), InstanceProperty::Relation(rel)) => {
            validate_relation_value(rel, prop_name, errors);
        }

        _ => {
            // Other type families or mismatches
            validate_property_type(instance_prop, property, errors);
        }
    }
}

/// Validates the property type matches the schema class
fn validate_property_type(
    instance_prop: &InstanceProperty,
    property: &crate::Property,
    errors: &mut Vec<ValidationError>,
) {
    let prop_name = &property.name;
    let expected_class = &property.class;

    // Check if it's a primitive type
    let is_primitive_class = expected_class.starts_with("xsd:") ||
                            expected_class == "xdd:json" ||
                            expected_class == "sys:Unit";

    match (is_primitive_class, instance_prop) {
        (true, InstanceProperty::Primitive(_)) | (true, InstanceProperty::Primitives(_)) => {
            // Valid: primitive property with primitive value
        }
        (false, InstanceProperty::Relation(_)) | (false, InstanceProperty::Relations(_)) => {
            // Valid: relation property with relation value
        }
        (true, InstanceProperty::Relation(_)) | (true, InstanceProperty::Relations(_)) => {
            errors.push(ValidationError::PropertyTypeMismatch {
                property: prop_name.clone(),
                expected: format!("primitive ({})", expected_class),
                actual: "relation".to_string(),
            });
        }
        (false, InstanceProperty::Primitive(_)) | (false, InstanceProperty::Primitives(_)) => {
            errors.push(ValidationError::PropertyTypeMismatch {
                property: prop_name.clone(),
                expected: format!("relation ({})", expected_class),
                actual: "primitive".to_string(),
            });
        }
        _ => {
            // Any type or other cases
        }
    }
}

/// Validates set cardinality constraints
fn validate_set_cardinality(
    prop_name: &str,
    cardinality: &SetCardinality,
    count: usize,
    errors: &mut Vec<ValidationError>,
) {
    let violation = match cardinality {
        SetCardinality::Exact(n) => count != *n,
        SetCardinality::Min(n) => count < *n,
        SetCardinality::Max(n) => count > *n,
        SetCardinality::Range { min, max } => count < *min || count > *max,
        SetCardinality::None => false,
    };

    if violation {
        errors.push(ValidationError::SetCardinalityViolation {
            property: prop_name.to_string(),
            constraint: format!("{:?}", cardinality),
            actual_count: count,
        });
    }
}

/// Validates array dimensions
fn validate_array_dimensions(
    instance_prop: &InstanceProperty,
    prop_name: &str,
    expected_dims: usize,
    errors: &mut Vec<ValidationError>,
) {
    // For now, we do a basic check that it's a primitive array
    // Full multidimensional array validation would require recursive depth checking
    match instance_prop {
        InstanceProperty::Primitives(_) if expected_dims == 1 => {
            // Valid 1D array
        }
        _ => {
            errors.push(ValidationError::ArrayDimensionMismatch {
                property: prop_name.to_string(),
                expected_dimensions: expected_dims,
                actual: format!("{:?}", instance_prop),
            });
        }
    }
}

/// Validates a relation value, checking nested instances if present
fn validate_relation_value(
    rel_value: &RelationValue,
    prop_name: &str,
    errors: &mut Vec<ValidationError>,
) {
    match rel_value {
        RelationValue::One(nested_instance) => {
            if let Err(nested_errors) = validate_instance(nested_instance, &nested_instance.schema) {
                errors.push(ValidationError::NestedInstanceError {
                    property: prop_name.to_string(),
                    errors: nested_errors,
                });
            }
        }
        RelationValue::More(nested_instances) => {
            for nested_instance in nested_instances {
                if let Err(nested_errors) = validate_instance(nested_instance, &nested_instance.schema) {
                    errors.push(ValidationError::NestedInstanceError {
                        property: prop_name.to_string(),
                        errors: nested_errors,
                    });
                }
            }
        }
        // References don't need validation here - they're just IDs
        _ => {}
    }
}

/// Extension trait for Instance to add validation methods
pub trait InstanceValidation {
    /// Validate this instance against its embedded schema
    fn validate(&self) -> ValidationResult;

    /// Validate this instance against a specific schema
    fn validate_with_schema(&self, schema: &Schema) -> ValidationResult;
}

impl InstanceValidation for Instance {
    fn validate(&self) -> ValidationResult {
        validate_instance(self, &self.schema)
    }

    fn validate_with_schema(&self, schema: &Schema) -> ValidationResult {
        validate_instance(self, schema)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Property, Schema, Instance, InstanceProperty, PrimitiveValue, Key};

    #[test]
    fn test_valid_simple_class() {
        let schema = Schema::Class {
            id: "Person".to_string(),
            base: None,
            key: Key::Random,
            documentation: None,
            subdocument: false,
            r#abstract: false,
            inherits: vec![],
            unfoldable: false,
            properties: vec![
                Property {
                    name: "name".to_string(),
                    r#type: None,
                    class: "xsd:string".to_string(),
                },
                Property {
                    name: "age".to_string(),
                    r#type: None,
                    class: "xsd:integer".to_string(),
                },
            ],
        };

        let mut properties = BTreeMap::new();
        properties.insert("name".to_string(), InstanceProperty::Primitive(PrimitiveValue::String("Alice".to_string())));
        properties.insert("age".to_string(), InstanceProperty::Primitive(PrimitiveValue::Number(30.into())));

        let instance = Instance {
            schema: schema.clone(),
            id: Some("Person/alice".to_string()),
            capture: false,
            ref_props: false,
            properties,
        };

        assert!(validate_instance(&instance, &schema).is_ok());
    }

    #[test]
    fn test_missing_required_property() {
        let schema = Schema::Class {
            id: "Person".to_string(),
            base: None,
            key: Key::Random,
            documentation: None,
            subdocument: false,
            r#abstract: false,
            inherits: vec![],
            unfoldable: false,
            properties: vec![
                Property {
                    name: "name".to_string(),
                    r#type: None,
                    class: "xsd:string".to_string(),
                },
            ],
        };

        let instance = Instance {
            schema: schema.clone(),
            id: Some("Person/bob".to_string()),
            capture: false,
            ref_props: false,
            properties: BTreeMap::new(),
        };

        let result = validate_instance(&instance, &schema);
        assert!(result.is_err());

        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1);
        assert!(matches!(errors[0], ValidationError::MissingProperty { .. }));
    }

    #[test]
    fn test_unknown_property() {
        let schema = Schema::Class {
            id: "Person".to_string(),
            base: None,
            key: Key::Random,
            documentation: None,
            subdocument: false,
            r#abstract: false,
            inherits: vec![],
            unfoldable: false,
            properties: vec![],
        };

        let mut properties = BTreeMap::new();
        properties.insert("unknown".to_string(), InstanceProperty::Primitive(PrimitiveValue::String("value".to_string())));

        let instance = Instance {
            schema: schema.clone(),
            id: Some("Person/test".to_string()),
            capture: false,
            ref_props: false,
            properties,
        };

        let result = validate_instance(&instance, &schema);
        assert!(result.is_err());

        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1);
        assert!(matches!(errors[0], ValidationError::UnknownProperty { .. }));
    }

    #[test]
    fn test_optional_property() {
        let schema = Schema::Class {
            id: "Person".to_string(),
            base: None,
            key: Key::Random,
            documentation: None,
            subdocument: false,
            r#abstract: false,
            inherits: vec![],
            unfoldable: false,
            properties: vec![
                Property {
                    name: "nickname".to_string(),
                    r#type: Some(TypeFamily::Optional),
                    class: "xsd:string".to_string(),
                },
            ],
        };

        let instance = Instance {
            schema: schema.clone(),
            id: Some("Person/test".to_string()),
            capture: false,
            ref_props: false,
            properties: BTreeMap::new(),
        };

        // Should be valid because property is optional
        assert!(validate_instance(&instance, &schema).is_ok());
    }

    #[test]
    fn test_enum_validation() {
        let schema = Schema::Enum {
            id: "Status".to_string(),
            values: vec!["active".to_string(), "inactive".to_string()],
            documentation: None,
        };

        // Valid enum - property key is the variant name, value is Unit
        let mut properties = BTreeMap::new();
        properties.insert("active".to_string(), InstanceProperty::Primitive(PrimitiveValue::Unit));

        let instance = Instance {
            schema: schema.clone(),
            id: None,
            capture: false,
            ref_props: false,
            properties,
        };

        assert!(validate_instance(&instance, &schema).is_ok());

        // Invalid enum value - variant name doesn't exist in schema
        let mut properties = BTreeMap::new();
        properties.insert("invalid".to_string(), InstanceProperty::Primitive(PrimitiveValue::Unit));

        let instance = Instance {
            schema: schema.clone(),
            id: None,
            capture: false,
            ref_props: false,
            properties,
        };

        let result = validate_instance(&instance, &schema);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err()[0], ValidationError::InvalidEnumValue { .. }));
    }

    #[test]
    fn test_set_cardinality() {
        let schema = Schema::Class {
            id: "Team".to_string(),
            base: None,
            key: Key::Random,
            documentation: None,
            subdocument: false,
            r#abstract: false,
            inherits: vec![],
            unfoldable: false,
            properties: vec![
                Property {
                    name: "members".to_string(),
                    r#type: Some(TypeFamily::Set(SetCardinality::Min(2))),
                    class: "xsd:string".to_string(),
                },
            ],
        };

        // Too few members
        let mut properties = BTreeMap::new();
        properties.insert("members".to_string(), InstanceProperty::Primitives(vec![
            PrimitiveValue::String("Alice".to_string()),
        ]));

        let instance = Instance {
            schema: schema.clone(),
            id: Some("Team/1".to_string()),
            capture: false,
            ref_props: false,
            properties,
        };

        let result = validate_instance(&instance, &schema);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err()[0], ValidationError::SetCardinalityViolation { .. }));
    }

    #[test]
    fn test_type_family_list() {
        let schema = Schema::Class {
            id: "TodoList".to_string(),
            base: None,
            key: Key::Random,
            documentation: None,
            subdocument: false,
            r#abstract: false,
            inherits: vec![],
            unfoldable: false,
            properties: vec![
                Property {
                    name: "items".to_string(),
                    r#type: Some(TypeFamily::List),
                    class: "xsd:string".to_string(),
                },
            ],
        };

        // Valid list
        let mut properties = BTreeMap::new();
        properties.insert("items".to_string(), InstanceProperty::Primitives(vec![
            PrimitiveValue::String("Item 1".to_string()),
            PrimitiveValue::String("Item 2".to_string()),
        ]));

        let instance = Instance {
            schema: schema.clone(),
            id: Some("TodoList/1".to_string()),
            capture: false,
            ref_props: false,
            properties,
        };

        assert!(validate_instance(&instance, &schema).is_ok());
    }

    #[test]
    fn test_property_type_mismatch() {
        let schema = Schema::Class {
            id: "User".to_string(),
            base: None,
            key: Key::Random,
            documentation: None,
            subdocument: false,
            r#abstract: false,
            inherits: vec![],
            unfoldable: false,
            properties: vec![
                Property {
                    name: "name".to_string(),
                    r#type: None,
                    class: "xsd:string".to_string(),
                },
            ],
        };

        // Provide a relation where a primitive is expected
        let mut properties = BTreeMap::new();
        properties.insert("name".to_string(), InstanceProperty::Relation(
            RelationValue::ExternalReference("User/123".to_string())
        ));

        let instance = Instance {
            schema: schema.clone(),
            id: Some("User/1".to_string()),
            capture: false,
            ref_props: false,
            properties,
        };

        let result = validate_instance(&instance, &schema);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err()[0], ValidationError::PropertyTypeMismatch { .. }));
    }

    #[test]
    fn test_nested_instance_validation() {
        // Create a nested instance with validation errors
        let address_schema = Schema::Class {
            id: "Address".to_string(),
            base: None,
            key: Key::Random,
            documentation: None,
            subdocument: true,
            r#abstract: false,
            inherits: vec![],
            unfoldable: false,
            properties: vec![
                Property {
                    name: "street".to_string(),
                    r#type: None,
                    class: "xsd:string".to_string(),
                },
            ],
        };

        let person_schema = Schema::Class {
            id: "Person".to_string(),
            base: None,
            key: Key::Random,
            documentation: None,
            subdocument: false,
            r#abstract: false,
            inherits: vec![],
            unfoldable: false,
            properties: vec![
                Property {
                    name: "address".to_string(),
                    r#type: None,
                    class: "Address".to_string(),
                },
            ],
        };

        // Create a nested address instance that's missing the required street property
        let nested_instance = Instance {
            schema: address_schema.clone(),
            id: None,
            capture: false,
            ref_props: false,
            properties: BTreeMap::new(), // Missing "street" property
        };

        let mut properties = BTreeMap::new();
        properties.insert("address".to_string(), InstanceProperty::Relation(
            RelationValue::One(nested_instance)
        ));

        let instance = Instance {
            schema: person_schema.clone(),
            id: Some("Person/1".to_string()),
            capture: false,
            ref_props: false,
            properties,
        };

        let result = validate_instance(&instance, &person_schema);
        assert!(result.is_err());

        let errors = result.unwrap_err();
        assert!(matches!(errors[0], ValidationError::NestedInstanceError { .. }));
    }

    #[test]
    fn test_validation_with_multiple_errors() {
        let schema = Schema::Class {
            id: "Product".to_string(),
            base: None,
            key: Key::Random,
            documentation: None,
            subdocument: false,
            r#abstract: false,
            inherits: vec![],
            unfoldable: false,
            properties: vec![
                Property {
                    name: "name".to_string(),
                    r#type: None,
                    class: "xsd:string".to_string(),
                },
                Property {
                    name: "price".to_string(),
                    r#type: None,
                    class: "xsd:decimal".to_string(),
                },
            ],
        };

        // Instance with unknown property and missing required properties
        let mut properties = BTreeMap::new();
        properties.insert("unknown_field".to_string(), InstanceProperty::Primitive(
            PrimitiveValue::String("value".to_string())
        ));

        let instance = Instance {
            schema: schema.clone(),
            id: Some("Product/1".to_string()),
            capture: false,
            ref_props: false,
            properties,
        };

        let result = validate_instance(&instance, &schema);
        assert!(result.is_err());

        let errors = result.unwrap_err();
        // Should have at least 3 errors: unknown property + 2 missing required properties
        assert!(errors.len() >= 3);
    }

    #[test]
    fn test_instance_validation_trait() {
        let schema = Schema::Class {
            id: "Simple".to_string(),
            base: None,
            key: Key::Random,
            documentation: None,
            subdocument: false,
            r#abstract: false,
            inherits: vec![],
            unfoldable: false,
            properties: vec![],
        };

        let instance = Instance {
            schema: schema.clone(),
            id: Some("Simple/1".to_string()),
            capture: false,
            ref_props: false,
            properties: BTreeMap::new(),
        };

        // Test the trait method
        assert!(instance.validate().is_ok());
        assert!(instance.validate_with_schema(&schema).is_ok());
    }
}
