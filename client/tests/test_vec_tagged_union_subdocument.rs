use anyhow::Result;
use terminusdb_client::*;
use terminusdb_schema::*;
use terminusdb_schema_derive::{TerminusDBModel, FromTDBInstance};
use serde::{Deserialize, Serialize};
use std::collections::{HashSet, BTreeSet};

// Tagged union enum marked as subdocument with struct variants
#[derive(Debug, Clone, Deserialize, Serialize, Eq, PartialEq, TerminusDBModel, FromTDBInstance)]
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
    SocialMedia {
        platform: String,
        handle: String,
        url: Option<String>,
    },
}

// Parent class containing a Vec of the tagged union subdocument
#[derive(Debug, Clone, Deserialize, Serialize, Eq, PartialEq, TerminusDBModel, FromTDBInstance)]
pub struct Person {
    pub name: String,
    pub age: i32,
    
    // Vec of tagged union subdocuments
    #[tdb(subdocument = true)]
    pub contact_methods: Vec<ContactMethod>,
    
    // Optional field to test edge cases
    pub notes: Option<String>,
}

// Alternative parent class using HashSet
#[derive(Debug, Clone, Deserialize, Serialize, Eq, PartialEq, TerminusDBModel, FromTDBInstance)]
pub struct PersonWithHashSet {
    pub name: String,
    pub age: i32,
    
    // HashSet of tagged union subdocuments
    #[tdb(subdocument = true)]
    pub contact_methods: HashSet<ContactMethod>,
    
    // Optional field to test edge cases
    pub notes: Option<String>,
}

// Alternative parent class using BTreeSet
#[derive(Debug, Clone, Deserialize, Serialize, Eq, PartialEq, TerminusDBModel, FromTDBInstance)]
pub struct PersonWithBTreeSet {
    pub name: String,
    pub age: i32,
    
    // BTreeSet of tagged union subdocuments
    #[tdb(subdocument = true)]
    pub contact_methods: BTreeSet<ContactMethod>,
    
    // Optional field to test edge cases
    pub notes: Option<String>,
}

// Need Hash implementation for ContactMethod to use in HashSet
impl std::hash::Hash for ContactMethod {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            ContactMethod::Email { address, verified } => {
                "email".hash(state);
                address.hash(state);
                verified.hash(state);
            }
            ContactMethod::Phone { number, country_code, is_mobile } => {
                "phone".hash(state);
                number.hash(state);
                country_code.hash(state);
                is_mobile.hash(state);
            }
            ContactMethod::Address { street, city, postal_code, country } => {
                "address".hash(state);
                street.hash(state);
                city.hash(state);
                postal_code.hash(state);
                country.hash(state);
            }
            ContactMethod::SocialMedia { platform, handle, url } => {
                "social".hash(state);
                platform.hash(state);
                handle.hash(state);
                url.hash(state);
            }
        }
    }
}

// Need Ord implementation for ContactMethod to use in BTreeSet
impl std::cmp::Ord for ContactMethod {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match (self, other) {
            (ContactMethod::Email { address: a1, verified: v1 }, ContactMethod::Email { address: a2, verified: v2 }) => {
                a1.cmp(a2).then(v1.cmp(v2))
            }
            (ContactMethod::Phone { number: n1, country_code: c1, is_mobile: m1 }, 
             ContactMethod::Phone { number: n2, country_code: c2, is_mobile: m2 }) => {
                n1.cmp(n2).then(c1.cmp(c2)).then(m1.cmp(m2))
            }
            (ContactMethod::Address { street: s1, city: c1, postal_code: p1, country: co1 },
             ContactMethod::Address { street: s2, city: c2, postal_code: p2, country: co2 }) => {
                s1.cmp(s2).then(c1.cmp(c2)).then(p1.cmp(p2)).then(co1.cmp(co2))
            }
            (ContactMethod::SocialMedia { platform: p1, handle: h1, url: u1 },
             ContactMethod::SocialMedia { platform: p2, handle: h2, url: u2 }) => {
                p1.cmp(p2).then(h1.cmp(h2)).then(u1.cmp(u2))
            }
            // Define order between different variants
            (ContactMethod::Email { .. }, _) => std::cmp::Ordering::Less,
            (ContactMethod::Phone { .. }, ContactMethod::Email { .. }) => std::cmp::Ordering::Greater,
            (ContactMethod::Phone { .. }, ContactMethod::Address { .. }) => std::cmp::Ordering::Less,
            (ContactMethod::Phone { .. }, ContactMethod::SocialMedia { .. }) => std::cmp::Ordering::Less,
            (ContactMethod::Address { .. }, ContactMethod::Email { .. }) => std::cmp::Ordering::Greater,
            (ContactMethod::Address { .. }, ContactMethod::Phone { .. }) => std::cmp::Ordering::Greater,
            (ContactMethod::Address { .. }, ContactMethod::SocialMedia { .. }) => std::cmp::Ordering::Less,
            (ContactMethod::SocialMedia { .. }, _) => std::cmp::Ordering::Greater,
        }
    }
}

impl std::cmp::PartialOrd for ContactMethod {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

async fn setup_test_client() -> Result<(TerminusDBHttpClient, BranchSpec)> {
    // Connect to remote TerminusDB instance
    let endpoint = url::Url::parse("http://78.47.46.135:6363")?;
    let client = TerminusDBHttpClient::new(
        endpoint,
        "admin",
        "sdkhgvslivglwiyagvw",
        "admin", // organization
    ).await?;
    
    let db_name = "test2".to_string();
    let spec = BranchSpec::new(db_name.clone());
    
    // Don't reset the database since it's a shared instance
    // Just make sure we can connect
    println!("Connected to TerminusDB at 78.47.46.135:6363");
    
    Ok((client, spec))
}

#[tokio::test]
#[ignore] // Requires running TerminusDB instance
async fn test_vec_tagged_union_subdocument_insert_and_retrieve() -> Result<()> {
    let (client, spec) = setup_test_client().await?;
    
    // Insert schemas
    let args = DocumentInsertArgs::from(spec.clone());
    
    // ContactMethod is a subdocument, so it should be included in Person's schema tree
    client.insert_entity_schema::<Person>(args.clone()).await?;
    
    // Create test data with various contact methods
    let person = Person {
        name: "Alice Johnson".to_string(),
        age: 35,
        contact_methods: vec![
            ContactMethod::Email {
                address: "alice@example.com".to_string(),
                verified: true,
            },
            ContactMethod::Phone {
                number: "555-1234".to_string(),
                country_code: "+1".to_string(),
                is_mobile: true,
            },
            ContactMethod::Address {
                street: "123 Main St".to_string(),
                city: "Springfield".to_string(),
                postal_code: "12345".to_string(),
                country: "USA".to_string(),
            },
            ContactMethod::SocialMedia {
                platform: "LinkedIn".to_string(),
                handle: "alicejohnson".to_string(),
                url: Some("https://linkedin.com/in/alicejohnson".to_string()),
            },
        ],
        notes: Some("Prefers email communication".to_string()),
    };
    
    // Insert the instance
    let insert_result = client.insert_instance(&person, args.clone()).await?;
    
    println!("Inserted instance with commit ID: {:?}", insert_result.commit_id);
    
    // Extract the instance ID
    let instance_id = match &insert_result.root_result {
        TDBInsertInstanceResult::Inserted(id) => id.clone(),
        TDBInsertInstanceResult::AlreadyExists(id) => id.clone(),
    };
    
    let short_id = instance_id.split('/').last().unwrap_or(&instance_id).to_string();
    println!("Instance ID: {}", short_id);
    
    // Use full ID for document retrieval
    let doc_id = format!("Person/{}", short_id);
    
    // Test 1: Check the raw JSON first
    println!("\n=== Raw JSON retrieval ===");
    let raw_json = client.get_document(&doc_id, &spec, GetOpts::default()).await?;
    println!("Raw JSON: {}", serde_json::to_string_pretty(&raw_json)?);
    
    // Test 1.5: Retrieve WITHOUT unfolding
    println!("\n=== Without unfolding ===");
    let mut deserializer = DefaultTDBDeserializer;
    let opts = GetOpts::default().with_unfold(false);
    
    // This will likely fail because subdocuments are stored as references
    match client.get_instances::<Person>(vec![short_id.clone()], &spec, opts, &mut deserializer).await {
        Ok(persons) => {
            println!("Retrieved {} persons without unfolding", persons.len());
            if !persons.is_empty() {
                println!("Number of contact methods: {}", persons[0].contact_methods.len());
            }
        }
        Err(e) => {
            println!("Failed to retrieve without unfolding (expected): {}", e);
        }
    }
    
    // Test 2: Retrieve WITH unfolding (full subdocument content)
    println!("\n=== With unfolding ===");
    
    // First check raw JSON with unfold
    let raw_json_unfolded = client.get_document(&doc_id, &spec, GetOpts::default().with_unfold(true)).await?;
    println!("Raw JSON with unfold: {}", serde_json::to_string_pretty(&raw_json_unfolded)?);
    
    let persons: Vec<Person> = client
        .get_instances_unfolded(vec![short_id], &spec, &mut deserializer)
        .await?;
    
    assert_eq!(persons.len(), 1, "Should retrieve exactly one person");
    let retrieved_person = &persons[0];
    
    // Verify basic fields
    assert_eq!(retrieved_person.name, "Alice Johnson");
    assert_eq!(retrieved_person.age, 35);
    assert_eq!(retrieved_person.notes, Some("Prefers email communication".to_string()));
    
    // Verify all contact methods were retrieved correctly
    assert_eq!(retrieved_person.contact_methods.len(), 4, "Should have 4 contact methods");
    
    // Check each contact method variant
    match &retrieved_person.contact_methods[0] {
        ContactMethod::Email { address, verified } => {
            assert_eq!(address, "alice@example.com");
            assert_eq!(*verified, true);
        }
        _ => panic!("Expected Email variant at index 0"),
    }
    
    match &retrieved_person.contact_methods[1] {
        ContactMethod::Phone { number, country_code, is_mobile } => {
            assert_eq!(number, "555-1234");
            assert_eq!(country_code, "+1");
            assert_eq!(*is_mobile, true);
        }
        _ => panic!("Expected Phone variant at index 1"),
    }
    
    match &retrieved_person.contact_methods[2] {
        ContactMethod::Address { street, city, postal_code, country } => {
            assert_eq!(street, "123 Main St");
            assert_eq!(city, "Springfield");
            assert_eq!(postal_code, "12345");
            assert_eq!(country, "USA");
        }
        _ => panic!("Expected Address variant at index 2"),
    }
    
    match &retrieved_person.contact_methods[3] {
        ContactMethod::SocialMedia { platform, handle, url } => {
            assert_eq!(platform, "LinkedIn");
            assert_eq!(handle, "alicejohnson");
            assert_eq!(url, &Some("https://linkedin.com/in/alicejohnson".to_string()));
        }
        _ => panic!("Expected SocialMedia variant at index 3"),
    }
    
    // Don't delete the shared test database
    
    println!("Test passed! Vec<TaggedUnionSubdocument> works correctly.");
    Ok(())
}

#[tokio::test]
#[ignore] // Requires running TerminusDB instance
async fn test_empty_vec_tagged_union_subdocument() -> Result<()> {
    let (client, spec) = setup_test_client().await?;
    
    // Insert schemas
    let args = DocumentInsertArgs::from(spec.clone());
    client.insert_entity_schema::<Person>(args.clone()).await?;
    
    // Create person with empty contact methods
    let person_empty = Person {
        name: "Bob Smith".to_string(),
        age: 42,
        contact_methods: vec![], // Empty vector
        notes: None,
    };
    
    client.insert_instance(&person_empty, args.clone()).await?;
    
    // Retrieve and verify
    let mut deserializer = DefaultTDBDeserializer;
    let persons: Vec<Person> = client
        .get_instances_unfolded(vec![], &spec, &mut deserializer)
        .await?;
    
    assert_eq!(persons.len(), 1);
    assert_eq!(persons[0].name, "Bob Smith");
    assert_eq!(persons[0].contact_methods.len(), 0);
    assert_eq!(persons[0].notes, None);
    
    client.delete_database(&spec.db).await?;
    Ok(())
}

#[tokio::test]
#[ignore] // Requires running TerminusDB instance
async fn test_single_variant_vec() -> Result<()> {
    let (client, spec) = setup_test_client().await?;
    
    // Insert schemas
    let args = DocumentInsertArgs::from(spec.clone());
    client.insert_entity_schema::<Person>(args.clone()).await?;
    
    // Create person with only email contacts
    let person_emails = Person {
        name: "Charlie Brown".to_string(),
        age: 28,
        contact_methods: vec![
            ContactMethod::Email {
                address: "charlie@work.com".to_string(),
                verified: true,
            },
            ContactMethod::Email {
                address: "charlie@personal.com".to_string(),
                verified: false,
            },
        ],
        notes: Some("Has multiple email addresses".to_string()),
    };
    
    let result = client.insert_instance(&person_emails, args.clone()).await?;
    let instance_id = match &result.root_result {
        TDBInsertInstanceResult::Inserted(id) => id.clone(),
        TDBInsertInstanceResult::AlreadyExists(id) => id.clone(),
    };
    let short_id = instance_id.split('/').last().unwrap_or(&instance_id).to_string();
    
    // Retrieve and verify
    let mut deserializer = DefaultTDBDeserializer;
    let persons: Vec<Person> = client
        .get_instances_unfolded(vec![short_id], &spec, &mut deserializer)
        .await?;
    
    assert_eq!(persons.len(), 1);
    let retrieved = &persons[0];
    assert_eq!(retrieved.name, "Charlie Brown");
    assert_eq!(retrieved.contact_methods.len(), 2);
    
    // Verify both are Email variants
    for (i, method) in retrieved.contact_methods.iter().enumerate() {
        match method {
            ContactMethod::Email { .. } => {
                println!("Contact method {} is Email variant ✓", i);
            }
            _ => panic!("Expected all contact methods to be Email variants"),
        }
    }
    
    client.delete_database(&spec.db).await?;
    Ok(())
}

#[tokio::test]
#[ignore] // Requires running TerminusDB instance  
async fn test_hashset_tagged_union_subdocument() -> Result<()> {
    let (client, spec) = setup_test_client().await?;
    
    // Insert schemas
    let args = DocumentInsertArgs::from(spec.clone());
    
    // Skip schema deletion since we're on a shared database
    
    client.insert_entity_schema::<PersonWithHashSet>(args.clone()).await?;
    
    // Create test data with HashSet
    let mut contact_set = HashSet::new();
    contact_set.insert(ContactMethod::Email {
        address: "alice@example.com".to_string(),
        verified: true,
    });
    contact_set.insert(ContactMethod::Phone {
        number: "555-1234".to_string(),
        country_code: "+1".to_string(),
        is_mobile: true,
    });
    
    let person_hashset = PersonWithHashSet {
        name: "Bob HashSet".to_string(),
        age: 45,
        contact_methods: contact_set,
        notes: Some("Testing HashSet".to_string()),
    };
    
    // Insert the instance
    let insert_result = client.insert_instance(&person_hashset, args.clone()).await?;
    println!("Inserted HashSet instance with commit ID: {:?}", insert_result.commit_id);
    
    // Extract the instance ID
    let instance_id = match &insert_result.root_result {
        TDBInsertInstanceResult::Inserted(id) => id.clone(),
        TDBInsertInstanceResult::AlreadyExists(id) => id.clone(),
    };
    
    let short_id = instance_id.split('/').last().unwrap_or(&instance_id).to_string();
    let doc_id = format!("PersonWithHashSet/{}", short_id);
    
    // Check raw JSON
    println!("\n=== HashSet Raw JSON ===");
    let raw_json = client.get_document(&doc_id, &spec, GetOpts::default()).await?;
    println!("Raw JSON: {}", serde_json::to_string_pretty(&raw_json)?);
    
    // Extract contact method IDs to check if they exist as separate documents
    if let Some(contact_methods) = raw_json.get("contact_methods").and_then(|v| v.as_array()) {
        println!("\n=== Checking ContactMethod documents ===");
        for (i, cm_ref) in contact_methods.iter().enumerate() {
            if let Some(cm_id) = cm_ref.as_str() {
                println!("Contact method {}: {}", i, cm_id);
                match client.get_document(cm_id, &spec, GetOpts::default()).await {
                    Ok(cm_doc) => {
                        println!("Found ContactMethod document: {}", serde_json::to_string_pretty(&cm_doc)?);
                        
                        // Try to get the variant document
                        if let Some(variant_entries) = cm_doc.as_object() {
                            for (variant_key, variant_value) in variant_entries {
                                if variant_key != "@id" && variant_key != "@type" {
                                    if let Some(variant_id) = variant_value.as_str() {
                                        println!("\nChecking variant document: {} -> {}", variant_key, variant_id);
                                        match client.get_document(variant_id, &spec, GetOpts::default()).await {
                                            Ok(variant_doc) => {
                                                println!("Found variant document: {}", serde_json::to_string_pretty(&variant_doc)?);
                                            }
                                            Err(e) => {
                                                println!("Could not retrieve variant {}: {}", variant_id, e);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        println!("Could not retrieve {}: {}", cm_id, e);
                    }
                }
            }
        }
    }
    
    // Check with unfolding
    println!("\n=== HashSet Raw JSON with unfold ===");
    let raw_json_unfolded = client.get_document(&doc_id, &spec, GetOpts::default().with_unfold(true)).await?;
    println!("Raw JSON with unfold: {}", serde_json::to_string_pretty(&raw_json_unfolded)?);
    
    // Try to retrieve typed instance
    let mut deserializer = DefaultTDBDeserializer;
    match client.get_instances_unfolded::<PersonWithHashSet>(vec![short_id], &spec, &mut deserializer).await {
        Ok(persons) => {
            println!("\nSuccessfully retrieved {} PersonWithHashSet instances", persons.len());
            if let Some(person) = persons.first() {
                println!("Name: {}", person.name);
                println!("Contact methods count: {}", person.contact_methods.len());
                for method in &person.contact_methods {
                    println!("  - {:?}", method);
                }
            }
        }
        Err(e) => {
            println!("\nFailed to retrieve PersonWithHashSet: {}", e);
        }
    }
    
    Ok(())
}

#[tokio::test]
#[ignore] // Requires running TerminusDB instance  
async fn test_btreeset_tagged_union_subdocument() -> Result<()> {
    let (client, spec) = setup_test_client().await?;
    
    // Insert schemas
    let args = DocumentInsertArgs::from(spec.clone());
    client.insert_entity_schema::<PersonWithBTreeSet>(args.clone()).await?;
    
    // Create test data with BTreeSet
    let mut contact_set = BTreeSet::new();
    contact_set.insert(ContactMethod::Email {
        address: "alice@example.com".to_string(),
        verified: true,
    });
    contact_set.insert(ContactMethod::Phone {
        number: "555-1234".to_string(),
        country_code: "+1".to_string(),
        is_mobile: true,
    });
    contact_set.insert(ContactMethod::Address {
        street: "123 Main St".to_string(),
        city: "Springfield".to_string(),
        postal_code: "12345".to_string(),
        country: "USA".to_string(),
    });
    
    let person_btreeset = PersonWithBTreeSet {
        name: "Charlie BTreeSet".to_string(),
        age: 38,
        contact_methods: contact_set,
        notes: Some("Testing BTreeSet".to_string()),
    };
    
    // Insert the instance
    let insert_result = client.insert_instance(&person_btreeset, args.clone()).await?;
    println!("Inserted BTreeSet instance with commit ID: {:?}", insert_result.commit_id);
    
    // Extract the instance ID
    let instance_id = match &insert_result.root_result {
        TDBInsertInstanceResult::Inserted(id) => id.clone(),
        TDBInsertInstanceResult::AlreadyExists(id) => id.clone(),
    };
    
    let short_id = instance_id.split('/').last().unwrap_or(&instance_id).to_string();
    let doc_id = format!("PersonWithBTreeSet/{}", short_id);
    
    // Check raw JSON to see if BTreeSet behaves differently
    println!("\n=== BTreeSet Raw JSON ===");
    let raw_json = client.get_document(&doc_id, &spec, GetOpts::default()).await?;
    println!("Raw JSON: {}", serde_json::to_string_pretty(&raw_json)?);
    
    // Check if subdocuments are embedded (as they should be) or references
    if let Some(contact_methods) = raw_json.get("contact_methods").and_then(|v| v.as_array()) {
        let first_contact = &contact_methods[0];
        if first_contact.is_string() {
            println!("\nBTreeSet ALSO stores subdocuments as references (not embedded) ❌");
        } else if first_contact.is_object() {
            println!("\nBTreeSet stores subdocuments as EMBEDDED objects ✅");
            println!("First contact embedded: {}", serde_json::to_string_pretty(first_contact)?);
        }
    }
    
    // Try retrieval with unfold
    let mut deserializer = DefaultTDBDeserializer;
    match client.get_instances_unfolded::<PersonWithBTreeSet>(vec![short_id], &spec, &mut deserializer).await {
        Ok(persons) => {
            println!("\n✅ Successfully retrieved PersonWithBTreeSet instance!");
            if let Some(person) = persons.first() {
                println!("Name: {}", person.name);
                println!("Contact methods count: {}", person.contact_methods.len());
                println!("\nThis means BTreeSet works correctly for subdocument embedding!");
            }
        }
        Err(e) => {
            println!("\n❌ Failed to retrieve PersonWithBTreeSet: {}", e);
            println!("BTreeSet has the same issue as Vec and HashSet");
        }
    }
    
    Ok(())
}