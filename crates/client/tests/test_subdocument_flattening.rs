use anyhow::Result;
use serde::{Deserialize, Serialize};
use terminusdb_schema::{ToTDBInstance, ToTDBInstances, ToTDBSchema};
use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};

// A subdocument model - should NOT be flattened when inserted
#[derive(Debug, Clone, Default, TerminusDBModel, FromTDBInstance)]
#[tdb(subdocument = true, key = "value_hash")]
struct Address {
    street: String,
    city: String,
    country: String,
}

// A regular document that can be referenced
#[derive(Debug, Clone, Default, TerminusDBModel, FromTDBInstance)]
struct Company {
    name: String,
}

// A model with both subdocument and regular document references
#[derive(Debug, Clone, Default, TerminusDBModel, FromTDBInstance)]
struct Person {
    name: String,
    // This is a subdocument - should remain nested
    #[tdb(subdocument = true)]
    home_address: Address,
    // This is a regular document reference - should be flattened
    employer: Company,
}

// A subdocument that contains a reference to a regular document
#[derive(Debug, Clone, Default, TerminusDBModel, FromTDBInstance)]
#[tdb(subdocument = true, key = "value_hash")]
struct ContactInfo {
    phone: String,
    preferred_company: Company, // Regular document reference within a subdocument
}

#[derive(Debug, Clone, TerminusDBModel, FromTDBInstance)]
struct Employee {
    name: String,
    #[tdb(subdocument = true)]
    contact: ContactInfo,
}

#[test]
fn test_subdocument_not_flattened() {
    // Create a person with nested address and employer
    let person = Person {
        name: "John Doe".to_string(),
        home_address: Address {
            street: "123 Main St".to_string(),
            city: "Springfield".to_string(),
            country: "USA".to_string(),
        },
        employer: Company {
            name: "ACME Corp".to_string(),
        },
    };

    // Convert to instance tree and flatten
    let instances = person.to_instance_tree_flatten(true);

    // Verify that we have 2 instances: Person and Company (NOT Address)
    assert_eq!(instances.len(), 2);

    // Verify the subdocument (Address) is NOT in the flattened list
    assert!(
        instances
            .iter()
            .find(|i| i.schema.class_name() == "Address")
            .is_none(),
        "Address subdocument should not be in the flattened instance list"
    );

    // Find the Person instance
    let person_instance = instances
        .iter()
        .find(|i| i.schema.class_name() == "Person")
        .unwrap();

    // Verify that home_address is still a nested instance (not flattened)
    let home_address_prop = person_instance.get_property("home_address").unwrap();
    match home_address_prop {
        terminusdb_schema::InstanceProperty::Relation(terminusdb_schema::RelationValue::One(
            addr_instance,
        )) => {
            // Success - the address is still a nested instance
            assert_eq!(addr_instance.schema.class_name(), "Address");
            assert!(addr_instance.schema.is_subdocument());
        }
        _ => panic!("Expected home_address to be a nested instance, not a reference"),
    }
}

#[test]
fn test_subdocument_schema_property() {
    // Verify that the subdocument property is correctly set in the schema
    let address_schema = Address::to_schema();
    println!(
        "Address schema is_subdocument: {}",
        address_schema.is_subdocument()
    );
    assert!(address_schema.is_subdocument());

    let company_schema = Company::to_schema();
    assert!(!company_schema.is_subdocument());

    let person_schema = Person::to_schema();
    assert!(!person_schema.is_subdocument());
}

#[test]
fn test_subdocument_not_flattened_with_ids() {
    // Create instances with explicit IDs to test flattening behavior
    let person_with_ids = Person {
        name: "John Doe".to_string(),
        home_address: Address {
            street: "123 Main St".to_string(),
            city: "Springfield".to_string(),
            country: "USA".to_string(),
        },
        employer: Company {
            name: "ACME Corp".to_string(),
        },
    };

    // Create instance with ID
    let person_inst = person_with_ids.to_instance(Some("Person/john-doe".to_string()));

    // Convert to flattened tree
    let instances = person_inst.to_instance_tree_flatten(true);

    // Should have 2 instances: Person and Company (NOT Address, even though it might have an ID)
    assert_eq!(instances.len(), 2);

    // Find Company - it won't have an ID unless explicitly provided
    let company = instances
        .iter()
        .find(|i| i.schema.class_name() == "Company")
        .unwrap();
    assert!(
        company.id.is_none(),
        "Company doesn't have an ID unless explicitly provided"
    );

    // Find Person instance
    let person_instance = instances
        .iter()
        .find(|i| i.schema.class_name() == "Person")
        .unwrap();

    // Since Company doesn't have an ID, it can't be flattened to a reference
    let employer_prop = person_instance.get_property("employer").unwrap();
    match employer_prop {
        terminusdb_schema::InstanceProperty::Relation(terminusdb_schema::RelationValue::One(
            inst,
        )) => {
            // Company remains as nested instance since it has no ID
            assert!(
                inst.id.is_none(),
                "Employer without ID remains as nested instance"
            );
            assert_eq!(inst.schema.class_name(), "Company");
        }
        _ => panic!("Expected employer to be a nested instance when it has no ID"),
    }

    // Verify home_address is still nested (subdocuments are never flattened)
    let home_address_prop = person_instance.get_property("home_address").unwrap();
    match home_address_prop {
        terminusdb_schema::InstanceProperty::Relation(terminusdb_schema::RelationValue::One(
            addr_instance,
        )) => {
            // Success - the address is still a nested instance
            assert_eq!(addr_instance.schema.class_name(), "Address");
            assert!(addr_instance.schema.is_subdocument());
        }
        _ => panic!("Expected home_address to be a nested instance, not a reference"),
    }
}

#[test]
fn test_nested_document_in_subdocument_is_flattened() {
    let company = Company {
        name: "Tech Corp".to_string(),
    };

    let employee = Employee {
        name: "Jane Smith".to_string(),
        contact: ContactInfo {
            phone: "555-1234".to_string(),
            preferred_company: company,
        },
    };

    // Get the instance tree and flatten it
    let instance = employee.to_instance(None);
    let mut instances = instance.to_instance_tree_flatten(true);

    // We should have 2 instances: Employee and Company (ContactInfo is subdocument, so not separate)
    assert_eq!(instances.len(), 2);

    // Find the Employee instance
    let employee_instance = instances
        .iter()
        .find(|i| i.schema.class_name() == "Employee")
        .unwrap();

    // Verify that contact is still nested
    let contact_prop = employee_instance.get_property("contact").unwrap();
    match contact_prop {
        terminusdb_schema::InstanceProperty::Relation(terminusdb_schema::RelationValue::One(
            contact_instance,
        )) => {
            // The contact subdocument should still be nested
            assert_eq!(contact_instance.schema.class_name(), "ContactInfo");

            // Within the subdocument, the company remains nested (no ID, can't be flattened)
            let company_prop = contact_instance.get_property("preferred_company").unwrap();
            match company_prop {
                terminusdb_schema::InstanceProperty::Relation(
                    terminusdb_schema::RelationValue::One(company_inst),
                ) => {
                    // Company remains nested since it has no ID
                    assert_eq!(company_inst.schema.class_name(), "Company");
                    assert!(
                        company_inst.id.is_none(),
                        "Company without ID remains nested"
                    );
                }
                _ => panic!("Expected company within subdocument to be a nested instance"),
            }
        }
        _ => panic!("Expected contact to be a nested instance"),
    }
}

#[test]
fn test_flattening_continues_through_subdocuments() {
    // Test that documents nested within subdocuments are properly flattened
    // when they have IDs

    // Create a Company instance with an explicit ID
    let company_with_id = Company {
        name: "Tech Corp".to_string(),
    }
    .to_instance(Some("Company/tech-corp".to_string()));

    // Create a subdocument that contains a reference to the Company
    let contact_with_company = ContactInfo {
        phone: "555-1234".to_string(),
        preferred_company: Company {
            name: "Tech Corp".to_string(),
        },
    };

    // Create the contact instance and manually set the company with ID
    let mut contact_inst = contact_with_company.to_instance(None);
    contact_inst.properties.insert(
        "preferred_company".to_string(),
        terminusdb_schema::InstanceProperty::Relation(terminusdb_schema::RelationValue::One(
            company_with_id.clone(),
        )),
    );

    // Create employee with the contact subdocument
    let mut employee_inst = Employee {
        name: "Jane Smith".to_string(),
        contact: ContactInfo::default(),
    }
    .to_instance(Some("Employee/jane".to_string()));

    employee_inst.properties.insert(
        "contact".to_string(),
        terminusdb_schema::InstanceProperty::Relation(terminusdb_schema::RelationValue::One(
            contact_inst,
        )),
    );

    // Get the instance tree and flatten it
    println!("\nBefore flattening:");
    let tree_before = employee_inst.to_instance_tree();
    for (i, inst) in tree_before.iter().enumerate() {
        println!(
            "  {}: {} (subdoc={}, id={:?})",
            i,
            inst.schema.class_name(),
            inst.schema.is_subdocument(),
            inst.id
        );
    }

    let instances = employee_inst.to_instance_tree_flatten(true);

    println!("\nAfter flattening:");
    for (i, inst) in instances.iter().enumerate() {
        println!(
            "  {}: {} (subdoc={}, id={:?})",
            i,
            inst.schema.class_name(),
            inst.schema.is_subdocument(),
            inst.id
        );
    }

    // We should have 2 instances: Employee and Company
    // ContactInfo is a subdocument so it's not in the list
    assert_eq!(
        instances.len(),
        2,
        "Should have Employee and Company, not ContactInfo"
    );

    // Verify ContactInfo is not in the flattened list
    assert!(
        instances
            .iter()
            .find(|i| i.schema.class_name() == "ContactInfo")
            .is_none(),
        "ContactInfo subdocument should not be in the flattened list"
    );

    // Find the Employee instance
    let employee = instances
        .iter()
        .find(|i| i.schema.class_name() == "Employee")
        .unwrap();

    // Verify that contact is still nested
    let contact_prop = employee.get_property("contact").unwrap();
    match contact_prop {
        terminusdb_schema::InstanceProperty::Relation(terminusdb_schema::RelationValue::One(
            contact_instance,
        )) => {
            assert_eq!(contact_instance.schema.class_name(), "ContactInfo");

            // IMPORTANT: Verify that the Company within the subdocument was flattened to a reference
            let company_prop = contact_instance.get_property("preferred_company").unwrap();
            println!("Company property in subdocument: {:?}", company_prop);
            match company_prop {
                terminusdb_schema::InstanceProperty::Relation(
                    terminusdb_schema::RelationValue::TransactionRef(id),
                )
                | terminusdb_schema::InstanceProperty::Relation(
                    terminusdb_schema::RelationValue::ExternalReference(id),
                ) => {
                    // Success! The company was flattened to a reference even though it's inside a subdocument
                    assert_eq!(id, "Company/tech-corp");
                    println!(
                        "✓ Company within subdocument was correctly flattened to reference: {}",
                        id
                    );
                }
                terminusdb_schema::InstanceProperty::Relation(
                    terminusdb_schema::RelationValue::One(inst),
                ) => {
                    println!("Company remains as instance with id: {:?}", inst.id);
                    panic!("Company with ID should be flattened to a reference, even within a subdocument");
                }
                _ => panic!(
                    "Expected company to be a reference, got: {:?}",
                    company_prop
                ),
            }
        }
        _ => panic!("Expected contact to be a nested instance"),
    }

    // Verify the Company is in the flattened list
    let company = instances
        .iter()
        .find(|i| i.schema.class_name() == "Company");
    assert!(
        company.is_some(),
        "Company should be in the flattened instance list"
    );
    assert_eq!(company.unwrap().id.as_ref().unwrap(), "Company/tech-corp");
}
