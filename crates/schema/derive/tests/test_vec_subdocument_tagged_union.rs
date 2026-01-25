use serde::{Deserialize, Serialize};
use terminusdb_schema::{Schema, ToTDBInstance, ToTDBSchema, TypeFamily};
use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};

// Tagged union marked as subdocument
#[derive(Debug, Clone, TerminusDBModel, FromTDBInstance)]
#[tdb(subdocument = true)]
pub enum ContactMethod {
    Email {
        address: String,
        verified: bool,
    },
    Phone {
        number: String,
        country_code: String,
        is_mobile: bool,
    },
    Address {
        street: String,
        city: String,
        postal_code: String,
        country: String,
    },
}

// Parent struct with Vec of subdocument tagged union
#[derive(Debug, Clone, TerminusDBModel, FromTDBInstance)]
pub struct Person {
    pub name: String,
    #[tdb(subdocument = true)]
    pub contact_methods: Vec<ContactMethod>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vec_subdocument_tagged_union_schema() {
        let schemas = <Person as ToTDBSchema>::to_schema_tree();

        // Should have Person class and ContactMethod tagged union,
        // plus variant structs for Email, Phone, Address
        assert!(schemas.len() >= 5);

        // Find Person schema
        let person_schema = schemas
            .iter()
            .find(|s| matches!(s, Schema::Class { id, .. } if id == "Person"))
            .expect("Should find Person schema");

        // Find ContactMethod tagged union
        let contact_method_schema = schemas
            .iter()
            .find(|s| matches!(s, Schema::TaggedUnion { id, .. } if id == "ContactMethod"))
            .expect("Should find ContactMethod schema");

        // Verify Person has contact_methods field
        if let Schema::Class { properties, .. } = person_schema {
            let contact_prop = properties
                .iter()
                .find(|p| p.name == "contact_methods")
                .expect("Should have contact_methods property");

            assert_eq!(contact_prop.class, "ContactMethod");
            assert_eq!(contact_prop.r#type, Some(TypeFamily::List));
        }

        // Verify variant structs are marked as subdocuments
        for schema in &schemas {
            if let Schema::Class {
                id, subdocument, ..
            } = schema
            {
                if id.starts_with("ContactMethod") && id != "ContactMethod" {
                    assert!(
                        *subdocument,
                        "ContactMethod variant {} should be marked as subdocument",
                        id
                    );
                }
            }
        }
    }

    #[test]
    fn test_contact_method_tagged_union_properties() {
        let schemas = <ContactMethod as ToTDBSchema>::to_schema_tree();

        // Find the main tagged union
        let union_schema = schemas
            .iter()
            .find(|s| matches!(s, Schema::TaggedUnion { id, .. } if id == "ContactMethod"))
            .expect("Should find ContactMethod TaggedUnion");

        if let Schema::TaggedUnion { properties, .. } = union_schema {
            assert_eq!(properties.len(), 3);

            let email_prop = properties.iter().find(|p| p.name == "email").unwrap();
            assert_eq!(email_prop.class, "ContactMethodEmail");

            let phone_prop = properties.iter().find(|p| p.name == "phone").unwrap();
            assert_eq!(phone_prop.class, "ContactMethodPhone");

            let address_prop = properties.iter().find(|p| p.name == "address").unwrap();
            assert_eq!(address_prop.class, "ContactMethodAddress");
        }

        // Verify Email variant struct
        let email_schema = schemas
            .iter()
            .find(|s| matches!(s, Schema::Class { id, .. } if id == "ContactMethodEmail"))
            .expect("Should find ContactMethodEmail schema");

        if let Schema::Class {
            properties,
            subdocument,
            ..
        } = email_schema
        {
            assert!(*subdocument, "Email variant should be subdocument");
            assert_eq!(properties.len(), 2);

            let address_prop = properties.iter().find(|p| p.name == "address").unwrap();
            assert_eq!(address_prop.class, "xsd:string");

            let verified_prop = properties.iter().find(|p| p.name == "verified").unwrap();
            assert_eq!(verified_prop.class, "xsd:boolean");
        }
    }
}
