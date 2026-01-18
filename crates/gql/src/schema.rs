//! GraphQL schema (SDL) generation from TerminusDB models.
//!
//! This module generates GraphQL Schema Definition Language (SDL) from
//! TerminusDB model definitions. The SDL includes all types, filters,
//! and query operations that TerminusDB would generate.

use std::borrow::Cow;
use std::collections::HashSet;
use terminusdb_community::graphql::frame::{AllFrames, FieldDefinition, FieldKind, GraphQLName, TypeDefinition};
use terminusdb_community::graphql::naming::{collection_filter_name, enum_filter_name, filter_name};
use terminusdb_schema::ToTDBSchemas;

use crate::frames::schemas_to_allframes;

/// Generate a GraphQL schema (SDL) from TerminusDB model definitions.
///
/// This function converts Rust model definitions to a complete GraphQL schema
/// that matches what TerminusDB would generate. The schema includes:
/// - Object types for each model
/// - Filter input types for querying
/// - Enum types
/// - The Query root type
///
/// # Example
///
/// ```ignore
/// use terminusdb_gql::generate_gql_schema;
///
/// let sdl = generate_gql_schema::<(Project, Ticket)>();
/// println!("{}", sdl);
/// ```
pub fn generate_gql_schema<T: ToTDBSchemas>() -> String {
    let frames = schemas_to_allframes::<T>();
    allframes_to_sdl(&frames)
}

/// Generate SDL from AllFrames.
pub fn allframes_to_sdl(frames: &AllFrames) -> String {
    let mut output = String::new();
    let mut used_base_filters: HashSet<String> = HashSet::new();

    // First pass: collect all used base filter types from all class fields
    for (_, typedef) in &frames.frames {
        if let TypeDefinition::Class(c) = typedef {
            for (_, field_def) in c.fields() {
                if let Some(base) = field_def.base_type() {
                    if let Some(ft) = base_type_to_filter_name(base) {
                        used_base_filters.insert(ft);
                    }
                }
            }
        }
    }

    // Add custom scalars
    output.push_str("# Custom Scalars\n");
    output.push_str("scalar BigInt\n");
    output.push_str("scalar BigFloat\n");
    output.push_str("scalar DateTime\n");
    output.push_str("scalar JSON\n");
    output.push_str("\n");

    // Generate enum types
    output.push_str("# Enum Types\n");
    for (name, typedef) in &frames.frames {
        if let TypeDefinition::Enum(e) = typedef {
            output.push_str(&format!("enum {} {{\n", name.as_str()));
            for value in &e.values {
                output.push_str(&format!("  {}\n", value.as_str()));
            }
            output.push_str("}\n\n");
        }
    }

    // Generate object types
    output.push_str("# Object Types\n");
    for (name, typedef) in &frames.frames {
        if let TypeDefinition::Class(c) = typedef {
            output.push_str(&format!("type {} {{\n", name.as_str()));
            output.push_str("  _id: ID!\n");
            output.push_str("  _type: ID!\n");
            output.push_str("  _json: JSON\n");

            for (field_name, field_def) in c.fields() {
                let field_type = field_type_to_sdl(field_def, frames, &mut used_base_filters);
                output.push_str(&format!("  {}: {}\n", field_name.as_str(), field_type));
            }

            output.push_str("}\n\n");
        }
    }

    // Generate base filter types (only the ones we actually use)
    output.push_str("# Base Filter Input Types\n");
    output.push_str(&generate_base_filter_types(&used_base_filters));

    // Generate model-specific filter types
    output.push_str("# Model Filter Input Types\n");
    for (name, typedef) in &frames.frames {
        if let TypeDefinition::Class(c) = typedef {
            output.push_str(&generate_model_filter(name.as_str(), c, frames));
        }
    }

    // Generate enum filter types
    output.push_str("# Enum Filter Input Types\n");
    for (name, typedef) in &frames.frames {
        if let TypeDefinition::Enum(_) = typedef {
            let filter_name = enum_filter_name(name);
            output.push_str(&format!("input {} {{\n", filter_name.as_str()));
            output.push_str(&format!("  eq: {}\n", name.as_str()));
            output.push_str(&format!("  ne: {}\n", name.as_str()));
            output.push_str("}\n\n");
        }
    }

    // Generate Query type
    output.push_str("# Query Root Type\n");
    output.push_str("type Query {\n");
    output.push_str("  _getDocument(id: String!): JSON\n");

    // Add _count with model filter arguments
    output.push_str("  _count(");
    let class_names: Vec<_> = frames
        .frames
        .iter()
        .filter_map(|(name, td)| {
            if matches!(td, TypeDefinition::Class(_)) {
                Some(name.as_str())
            } else {
                None
            }
        })
        .collect();

    for (i, name) in class_names.iter().enumerate() {
        if i > 0 {
            output.push_str(", ");
        }
        let gql_name = GraphQLName(Cow::Borrowed(*name));
        let filter_type = filter_name(&gql_name);
        output.push_str(&format!("{}: {}", name, filter_type.as_str()));
    }
    output.push_str("): Int!\n");

    // Add query field for each class
    for (name, typedef) in &frames.frames {
        if let TypeDefinition::Class(_) = typedef {
            let filter_type = filter_name(name);
            output.push_str(&format!(
                "  {}(id: ID, ids: [ID!], include_children: Boolean, offset: Int, limit: Int, filter: {}, orderBy: {}_Ordering): [{}!]!\n",
                name.as_str(),
                filter_type.as_str(),
                name.as_str(),
                name.as_str()
            ));
        }
    }

    output.push_str("}\n\n");

    // Generate ordering types
    output.push_str("# Ordering Types\n");
    output.push_str("enum Ordering {\n  Asc\n  Desc\n}\n\n");

    for (name, typedef) in &frames.frames {
        if let TypeDefinition::Class(c) = typedef {
            let has_orderable_fields = c.fields().iter().any(|(_, fd)| fd.base_type().is_some());
            if has_orderable_fields {
                output.push_str(&format!("input {}_Ordering {{\n", name.as_str()));
                for (field_name, field_def) in c.fields() {
                    if field_def.base_type().is_some() {
                        output.push_str(&format!("  {}: Ordering\n", field_name.as_str()));
                    }
                }
                output.push_str("}\n\n");
            }
        }
    }

    output
}

/// Convert a field definition to SDL type string.
fn field_type_to_sdl(
    field_def: &FieldDefinition,
    frames: &AllFrames,
    used_filters: &mut HashSet<String>,
) -> String {
    let base_type = if let Some(base) = field_def.base_type() {
        // Track which base filters are used
        let filter_type = base_type_to_filter_name(base);
        if let Some(ft) = filter_type {
            used_filters.insert(ft);
        }
        base_type_to_sdl(base)
    } else if let Some(enum_type) = field_def.enum_type(frames) {
        enum_type.to_string()
    } else if let Some(doc_type) = field_def.document_type(frames) {
        doc_type.to_string()
    } else {
        "ID".to_string()
    };

    match field_def.kind() {
        FieldKind::Required => format!("{}!", base_type),
        FieldKind::Optional => base_type,
        FieldKind::Set | FieldKind::List | FieldKind::Array | FieldKind::Cardinality => {
            format!("[{}!]!", base_type)
        }
    }
}

/// Convert a base type to SDL type name.
/// Handles both prefixed (xsd:string) and unprefixed (string) forms.
fn base_type_to_sdl(base_type: &str) -> String {
    match base_type {
        "xsd:string" | "string" | "xsd:anyURI" | "anyURI" | "xsd:language" | "language"
        | "xsd:normalizedString" | "normalizedString" | "xsd:token" | "token"
        | "xsd:NMTOKEN" | "NMTOKEN" | "xsd:Name" | "Name" | "xsd:NCName" | "NCName" => {
            "String".to_string()
        }
        "xsd:boolean" | "boolean" => "Boolean".to_string(),
        "xsd:integer" | "integer" | "xsd:int" | "int" | "xsd:short" | "short" | "xsd:byte"
        | "byte" | "xsd:nonNegativeInteger" | "nonNegativeInteger" | "xsd:positiveInteger"
        | "positiveInteger" | "xsd:nonPositiveInteger" | "nonPositiveInteger"
        | "xsd:negativeInteger" | "negativeInteger" | "xsd:unsignedInt" | "unsignedInt"
        | "xsd:unsignedShort" | "unsignedShort" | "xsd:unsignedByte" | "unsignedByte" => {
            "Int".to_string()
        }
        "xsd:long" | "long" | "xsd:unsignedLong" | "unsignedLong" => "BigInt".to_string(),
        "xsd:float" | "float" | "xsd:double" | "double" => "Float".to_string(),
        "xsd:decimal" | "decimal" => "BigFloat".to_string(),
        "xsd:dateTime" | "dateTime" | "xsd:date" | "date" | "xsd:time" | "time"
        | "xsd:dateTimeStamp" | "dateTimeStamp" => "DateTime".to_string(),
        "sys:JSON" | "JSON" => "JSON".to_string(),
        _ => "String".to_string(), // Default to String for unknown types
    }
}

/// Get the filter type name for a base type.
/// Handles both prefixed (xsd:string) and unprefixed (string) forms.
fn base_type_to_filter_name(base_type: &str) -> Option<String> {
    match base_type {
        "xsd:string" | "string" | "xsd:anyURI" | "anyURI" | "xsd:language" | "language"
        | "xsd:normalizedString" | "normalizedString" | "xsd:token" | "token"
        | "xsd:NMTOKEN" | "NMTOKEN" | "xsd:Name" | "Name" | "xsd:NCName" | "NCName" => {
            Some("String".to_string())
        }
        "xsd:boolean" | "boolean" => Some("Boolean".to_string()),
        "xsd:integer" | "integer" | "xsd:int" | "int" | "xsd:short" | "short" | "xsd:byte"
        | "byte" | "xsd:nonNegativeInteger" | "nonNegativeInteger" | "xsd:positiveInteger"
        | "positiveInteger" | "xsd:nonPositiveInteger" | "nonPositiveInteger"
        | "xsd:negativeInteger" | "negativeInteger" | "xsd:unsignedInt" | "unsignedInt"
        | "xsd:unsignedShort" | "unsignedShort" | "xsd:unsignedByte" | "unsignedByte" => {
            Some("Int".to_string())
        }
        "xsd:long" | "long" | "xsd:unsignedLong" | "unsignedLong" => Some("BigInt".to_string()),
        "xsd:float" | "float" | "xsd:double" | "double" => Some("Float".to_string()),
        "xsd:decimal" | "decimal" => Some("BigFloat".to_string()),
        "xsd:dateTime" | "dateTime" | "xsd:date" | "date" | "xsd:time" | "time"
        | "xsd:dateTimeStamp" | "dateTimeStamp" => Some("DateTime".to_string()),
        _ => None,
    }
}

/// Generate base filter input types.
fn generate_base_filter_types(used: &HashSet<String>) -> String {
    let mut output = String::new();

    if used.contains("String") {
        output.push_str(
            r#"input StringFilter {
  eq: String
  ne: String
  lt: String
  le: String
  gt: String
  ge: String
  regex: String
  startsWith: String
  allOfTerms: [String!]
  anyOfTerms: [String!]
}

input CollectionStringFilter {
  someHave: StringFilter
  allHave: StringFilter
}

"#,
        );
    }

    if used.contains("Int") {
        output.push_str(
            r#"input IntFilter {
  eq: Int
  ne: Int
  lt: Int
  le: Int
  gt: Int
  ge: Int
}

input CollectionIntFilter {
  someHave: IntFilter
  allHave: IntFilter
}

"#,
        );
    }

    if used.contains("BigInt") {
        output.push_str(
            r#"input BigIntFilter {
  eq: BigInt
  ne: BigInt
  lt: BigInt
  le: BigInt
  gt: BigInt
  ge: BigInt
}

input CollectionBigIntFilter {
  someHave: BigIntFilter
  allHave: BigIntFilter
}

"#,
        );
    }

    if used.contains("Float") {
        output.push_str(
            r#"input FloatFilter {
  eq: Float
  ne: Float
  lt: Float
  le: Float
  gt: Float
  ge: Float
}

input CollectionFloatFilter {
  someHave: FloatFilter
  allHave: FloatFilter
}

"#,
        );
    }

    if used.contains("BigFloat") {
        output.push_str(
            r#"input BigFloatFilter {
  eq: BigFloat
  ne: BigFloat
  lt: BigFloat
  le: BigFloat
  gt: BigFloat
  ge: BigFloat
}

input CollectionBigFloatFilter {
  someHave: BigFloatFilter
  allHave: BigFloatFilter
}

"#,
        );
    }

    if used.contains("Boolean") {
        output.push_str(
            r#"input BooleanFilter {
  eq: Boolean
  ne: Boolean
}

input CollectionBooleanFilter {
  someHave: BooleanFilter
  allHave: BooleanFilter
}

"#,
        );
    }

    if used.contains("DateTime") {
        output.push_str(
            r#"input DateTimeFilter {
  eq: DateTime
  ne: DateTime
  lt: DateTime
  le: DateTime
  gt: DateTime
  ge: DateTime
}

input CollectionDateTimeFilter {
  someHave: DateTimeFilter
  allHave: DateTimeFilter
}

"#,
        );
    }

    // Always include ID filter
    output.push_str(
        r#"input IdFilter {
  _id: ID
  _ids: [ID!]
}

input CollectionIdFilter {
  someHave: IdFilter
  allHave: IdFilter
}

"#,
    );

    output
}

/// Generate a filter input type for a model class.
fn generate_model_filter(
    class_name: &str,
    class_def: &terminusdb_community::graphql::frame::ClassDefinition,
    frames: &AllFrames,
) -> String {
    let gql_name = GraphQLName(Cow::Borrowed(class_name));
    let filter_type_name = filter_name(&gql_name);
    let mut output = format!("input {} {{\n", filter_type_name.as_str());

    // Add fields based on the class definition
    for (field_name, field_def) in class_def.fields() {
        let filter_type = field_to_filter_type(field_def, frames);
        output.push_str(&format!("  {}: {}\n", field_name.as_str(), filter_type));
    }

    // Add standard filter fields
    output.push_str("  _id: ID\n");
    output.push_str("  _ids: [ID!]\n");
    output.push_str(&format!("  _and: [{}]\n", filter_type_name.as_str()));
    output.push_str(&format!("  _or: [{}]\n", filter_type_name.as_str()));
    output.push_str(&format!("  _not: {}\n", filter_type_name.as_str()));

    output.push_str("}\n\n");

    // Generate collection filter if needed
    let collection_filter_name = collection_filter_name(&gql_name);
    output.push_str(&format!(
        "input {} {{\n",
        collection_filter_name.as_str()
    ));
    output.push_str(&format!(
        "  someHave: {}\n",
        filter_type_name.as_str()
    ));
    output.push_str(&format!(
        "  allHave: {}\n",
        filter_type_name.as_str()
    ));
    output.push_str("}\n\n");

    output
}

/// Get the filter type for a field.
fn field_to_filter_type(field_def: &FieldDefinition, frames: &AllFrames) -> String {
    let is_collection = field_def.kind().is_collection();

    if let Some(base_type) = field_def.base_type() {
        let base_filter = match base_type_to_sdl(base_type).as_str() {
            "String" => "StringFilter",
            "Int" => "IntFilter",
            "BigInt" => "BigIntFilter",
            "Float" => "FloatFilter",
            "BigFloat" => "BigFloatFilter",
            "Boolean" => "BooleanFilter",
            "DateTime" => "DateTimeFilter",
            _ => "StringFilter",
        };

        if is_collection {
            format!("Collection{}", base_filter)
        } else {
            base_filter.to_string()
        }
    } else if let Some(enum_type) = field_def.enum_type(frames) {
        let filter_name = enum_filter_name(&enum_type);
        if is_collection {
            format!("Collection{}", filter_name.as_str())
        } else {
            filter_name.to_string()
        }
    } else if let Some(doc_type) = field_def.document_type(frames) {
        let filter_name = filter_name(&doc_type);
        if is_collection {
            collection_filter_name(&doc_type).to_string()
        } else {
            filter_name.to_string()
        }
    } else {
        // Foreign reference
        if is_collection {
            "CollectionIdFilter".to_string()
        } else {
            "IdFilter".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frames::schemas_vec_to_allframes;
    use terminusdb_schema::{Key, Property, Schema, TypeFamily};

    #[test]
    fn test_generate_sdl_basic() {
        let schemas = vec![
            Schema::Class {
                id: "Project".to_string(),
                base: None,
                key: Key::Lexical(vec!["name".to_string()]),
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
                        name: "description".to_string(),
                        r#type: Some(TypeFamily::Optional),
                        class: "xsd:string".to_string(),
                    },
                ],
            },
            Schema::Enum {
                id: "Priority".to_string(),
                base: None,
                values: vec!["Low".to_string(), "Medium".to_string(), "High".to_string()],
                documentation: None,
            },
        ];

        let frames = schemas_vec_to_allframes(&schemas);
        let sdl = allframes_to_sdl(&frames);

        // Check that the SDL contains expected elements
        assert!(sdl.contains("type Project"), "Should contain Project type");
        assert!(sdl.contains("enum Priority"), "Should contain Priority enum");
        assert!(sdl.contains("input Project_Filter"), "Should contain Project filter");
        assert!(sdl.contains("type Query"), "Should contain Query type");
        assert!(sdl.contains("scalar BigInt"), "Should contain custom scalars");
        assert!(sdl.contains("input StringFilter"), "Should contain StringFilter");
    }
}
