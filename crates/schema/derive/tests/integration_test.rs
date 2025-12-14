use anyhow::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use terminusdb_schema::{Schema, ToMaybeTDBSchema, ToTDBInstance, ToTDBSchema, TypeFamily};
use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};

// Address type used for nested structures
#[derive(Debug, Clone, TerminusDBModel, FromTDBInstance, Serialize, Deserialize)]
struct Address {
    street: String,
    city: String,
    country: String,
    #[tdb(name = "postalCode")]
    postal_code: String,
}

// Simple enum for status
#[derive(Debug, Clone, PartialEq, TerminusDBModel, FromTDBInstance, Serialize, Deserialize)]
enum UserStatus {
    Active,
    Inactive,
    Suspended,
}

// Contact information tagged union
#[derive(Debug, Clone, TerminusDBModel, FromTDBInstance, Serialize, Deserialize)]
#[tdb(unfoldable = true)]
enum ContactInfo {
    Email(String),
    Phone(String),
    Address {
        // #[tdb(subdocument = true)]
        home: Address,
        #[tdb(subdocument = true)]
        work: Option<Address>,
    },
    None,
}

// Comprehensive user profile with nested types and enums
#[derive(Debug, Clone, TerminusDBModel, FromTDBInstance, Serialize, Deserialize)]
#[tdb(
    base = "http://example.org/users/",
    doc = "Comprehensive user profile with contact information"
)]
struct User {
    #[tdb(name = "userId")]
    id: String,
    #[tdb(name = "fullName")]
    name: String,
    #[tdb(name = "userAge")]
    age: i32,
    status: UserStatus,
    primary_contact: ContactInfo,
    #[tdb(subdocument = true)]
    secondary_contacts: Vec<ContactInfo>,
    #[tdb(name = "profileMetadata", class = "xsd:string")]
    metadata: HashMap<String, String>,
    #[tdb(name = "isVerified")]
    verified: bool,
}

#[test]
fn test_complex_integration() {
    // Get all schemas for the complex example
    let schemas = <User as ToTDBSchema>::to_schema_tree();

    dbg!(&schemas);

    // Find the main schema for User
    let user_schema = schemas
        .iter()
        .find(|s| matches!(s, Schema::Class { id, .. } if id == "User" ))
        .unwrap();

    // Find the ContactInfo schema
    let contact_schema = schemas
        .iter()
        .find(|s| matches!(s, Schema::TaggedUnion { id, .. } if id == "ContactInfo"))
        .unwrap();

    // Find the UserStatus schema
    let status_schema = schemas
        .iter()
        .find(|s| matches!(s, Schema::Enum { id, .. } if id == "UserStatus"))
        .unwrap();

    // Find the Address schema
    let address_schema = schemas
        .iter()
        .find(|s| matches!(s, Schema::Class { id, .. } if id == "Address"))
        .unwrap();

    // Find the ContactInfoAddress virtual struct schema
    let contact_address_schema = schemas
        .iter()
        .find(|s| matches!(s, Schema::Class { id, .. } if id == "ContactInfoAddress"))
        .unwrap();

    // We should have:
    // 1. UserProfile (User) schema
    // 2. UserStatus enum schema
    // 3. ContactInfo tagged union schema
    // 4. ContactInfoAddress virtual struct schema
    // 5. Address struct schema
    assert!(schemas.len() >= 5);

    // Verify User schema properties
    if let Schema::Class { properties, .. } = user_schema {
        let props = properties;

        // Check for all properties
        let prop_names: Vec<String> = props.iter().map(|p| p.name.clone()).collect();
        assert!(prop_names.contains(&"userId".to_string()));
        assert!(prop_names.contains(&"fullName".to_string()));
        assert!(prop_names.contains(&"userAge".to_string()));
        assert!(prop_names.contains(&"status".to_string()));
        assert!(prop_names.contains(&"primary_contact".to_string()));
        assert!(prop_names.contains(&"secondary_contacts".to_string()));
        assert!(prop_names.contains(&"profileMetadata".to_string()));
        assert!(prop_names.contains(&"isVerified".to_string()));

        // Verify types
        let status_prop = props.iter().find(|p| p.name == "status").unwrap();
        assert_eq!(status_prop.class, "UserStatus");

        let contact_prop = props.iter().find(|p| p.name == "primary_contact").unwrap();
        assert_eq!(contact_prop.class, "ContactInfo");

        // Verify Vec<ContactInfo> has TypeFamily::List
        let secondary_contacts_prop = props
            .iter()
            .find(|p| p.name == "secondary_contacts")
            .unwrap();
        assert_eq!(secondary_contacts_prop.class, "ContactInfo");
        assert!(
            matches!(secondary_contacts_prop.r#type, Some(prop_type) if prop_type == TypeFamily::List)
        );

        // Verify HashMap<String, String> property
        let metadata_prop = props.iter().find(|p| p.name == "profileMetadata").unwrap();
        assert!(matches!(metadata_prop.r#type, Some(prop_type) if prop_type.is_set()));
    }

    // Verify UserStatus enum values
    if let Schema::Enum { values, .. } = status_schema {
        let enum_values = values;
        assert_eq!(enum_values.len(), 3);

        let value_names: Vec<String> = enum_values.iter().map(|v| v.to_string()).collect();
        assert!(value_names.contains(&"active".to_string()));
        assert!(value_names.contains(&"inactive".to_string()));
        assert!(value_names.contains(&"suspended".to_string()));
    }

    // Verify ContactInfo union variants
    if let Schema::TaggedUnion { properties, .. } = contact_schema {
        let props = properties;

        // Check for all variant properties
        let prop_names: Vec<String> = props.iter().map(|p| p.name.clone()).collect();
        assert!(prop_names.contains(&"email".to_string()));
        assert!(prop_names.contains(&"phone".to_string()));
        assert!(prop_names.contains(&"address".to_string()));
        assert!(prop_names.contains(&"none".to_string()));

        // Verify address variant points to the virtual struct
        let address_prop = props.iter().find(|p| p.name == "address").unwrap();
        assert_eq!(address_prop.class, "ContactInfoAddress");
    }

    // Verify Address schema properties
    if let Schema::Class { properties, .. } = address_schema {
        let props = properties;

        // Check all properties
        let prop_names: Vec<String> = props.iter().map(|p| p.name.clone()).collect();
        assert!(prop_names.contains(&"street".to_string()));
        assert!(prop_names.contains(&"city".to_string()));
        assert!(prop_names.contains(&"country".to_string()));
        assert!(prop_names.contains(&"postalCode".to_string()));
    }

    // Verify ContactInfoAddress virtual struct properties
    if let Schema::Class { properties, .. } = contact_address_schema {
        let props = properties;

        // Check all properties
        let prop_names: Vec<String> = props.iter().map(|p| p.name.clone()).collect();
        assert!(prop_names.contains(&"home".to_string()));
        assert!(prop_names.contains(&"work".to_string()));

        // Verify property types
        let home_prop = props.iter().find(|p| p.name == "home").unwrap();
        assert_eq!(home_prop.class, "Address");
    }
}
