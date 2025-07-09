use serde::{Deserialize, Serialize};
use terminusdb_client::*;
use terminusdb_schema::*;
use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};

/// Test models for multi-schema insertion
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TerminusDBModel, FromTDBInstance)]
#[tdb(id_field = "id")]
struct Person {
    id: EntityIDFor<Self>,
    name: String,
    age: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TerminusDBModel, FromTDBInstance)]
#[tdb(id_field = "id")]
struct Company {
    id: EntityIDFor<Self>,
    name: String,
    founded: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TerminusDBModel, FromTDBInstance)]
#[tdb(id_field = "id")]
struct Product {
    id: EntityIDFor<Self>,
    name: String,
    price: f64,
}

/// Test setup
async fn setup_test_client() -> anyhow::Result<(TerminusDBHttpClient, BranchSpec)> {
    let client = TerminusDBHttpClient::local_node_test().await?;
    let spec = BranchSpec::from("test_multi_schema");

    Ok((client, spec))
}

#[ignore] // Requires running TerminusDB instance
#[tokio::test]
async fn test_schemas_macro() -> anyhow::Result<()> {
    let (client, spec) = setup_test_client().await?;

    println!("=== Testing schemas! macro ===");

    // Use the schemas! macro to generate multiple schemas
    let schema_vec = schemas!(Person, Company, Product);

    println!("Generated {} schemas using macro", schema_vec.len());
    assert_eq!(schema_vec.len(), 3, "Should generate 3 schemas");

    // Insert the schemas using the macro result
    let args = DocumentInsertArgs::from(spec.clone()).as_schema();
    client
        .insert_documents(schema_vec.iter().collect(), args)
        .await?;

    println!("✓ Successfully inserted schemas using macro approach");

    // Verify we can create instances after schema insertion
    let person = Person {
        id: EntityIDFor::new("alice").unwrap(),
        name: "Alice".to_string(),
        age: 30,
    };

    let company = Company {
        id: EntityIDFor::new("acme").unwrap(),
        name: "ACME Corp".to_string(),
        founded: 1990,
    };

    let product = Product {
        id: EntityIDFor::new("widget").unwrap(),
        name: "Super Widget".to_string(),
        price: 29.99,
    };

    let instance_args = DocumentInsertArgs::from(spec.clone());
    client
        .create_instance(&person, instance_args.clone())
        .await?;
    client
        .create_instance(&company, instance_args.clone())
        .await?;
    client
        .create_instance(&product, instance_args.clone())
        .await?;

    println!("✓ Successfully created instances after schema insertion");

    Ok(())
}

#[ignore] // Requires running TerminusDB instance
#[tokio::test]
async fn test_insert_schemas_tuple() -> anyhow::Result<()> {
    let (client, spec) = setup_test_client().await?;

    println!("=== Testing insert_schemas() with tuples ===");

    // Test with 2-tuple
    let args = DocumentInsertArgs::from(spec.clone());
    client
        .insert_schemas::<(Person, Company)>(args.clone())
        .await?;
    println!("✓ Successfully inserted 2 schemas using tuple approach");

    // Test with 3-tuple
    client
        .insert_schemas::<(Person, Company, Product)>(args.clone())
        .await?;
    println!("✓ Successfully inserted 3 schemas using tuple approach");

    // Verify we can create instances
    let person = Person {
        id: EntityIDFor::new("bob").unwrap(),
        name: "Bob".to_string(),
        age: 25,
    };

    client.create_instance(&person, args.clone()).await?;
    println!("✓ Successfully created instance after tuple schema insertion");

    Ok(())
}

#[ignore] // Requires running TerminusDB instance
#[tokio::test]
async fn test_schemas_macro_empty() -> anyhow::Result<()> {
    println!("=== Testing empty schemas! macro ===");

    // Test empty macro
    let empty_schemas: Vec<Schema> = schemas!();
    assert_eq!(
        empty_schemas.len(),
        0,
        "Empty macro should return empty vec"
    );
    println!("✓ Empty schemas! macro works correctly");

    Ok(())
}

#[tokio::test]
async fn test_schemas_macro_compilation() -> anyhow::Result<()> {
    println!("=== Testing schemas! macro compilation ===");

    // Test that the macro compiles correctly (no DB needed)
    let _schemas_1 = schemas!(Person);
    let _schemas_2 = schemas!(Person, Company);
    let _schemas_3 = schemas!(Person, Company, Product);
    let _schemas_trailing_comma = schemas!(Person, Company, Product,);
    let _schemas_empty: Vec<Schema> = schemas!();

    println!("✓ All macro variations compile correctly");

    Ok(())
}

#[tokio::test]
async fn test_tuple_schemas_compilation() -> anyhow::Result<()> {
    println!("=== Testing ToTDBSchemas trait compilation ===");

    // Test that tuple trait works (no DB needed)
    let _schemas_1 = <(Person,)>::to_schemas();
    let _schemas_2 = <(Person, Company)>::to_schemas();
    let _schemas_3 = <(Person, Company, Product)>::to_schemas();

    // Check lengths
    assert_eq!(_schemas_1.len(), 1);
    assert_eq!(_schemas_2.len(), 2);
    assert_eq!(_schemas_3.len(), 3);

    println!("✓ All tuple variations work correctly");

    Ok(())
}

#[ignore] // Requires running TerminusDB instance
#[tokio::test]
async fn test_mixed_approaches_comparison() -> anyhow::Result<()> {
    let (client, spec) = setup_test_client().await?;

    println!("=== Testing macro vs tuple approach comparison ===");

    // Generate schemas using both approaches
    let macro_schemas = schemas!(Person, Company, Product);
    let tuple_schemas = <(Person, Company, Product)>::to_schemas();

    // Both should generate the same number of schemas
    assert_eq!(macro_schemas.len(), tuple_schemas.len());
    println!("✓ Both approaches generate same number of schemas");

    // Both should work for insertion
    let args = DocumentInsertArgs::from(spec.clone());

    // Test macro approach
    client
        .insert_documents(macro_schemas.iter().collect(), args.clone().as_schema())
        .await?;
    println!("✓ Macro approach insertion works");

    // Test tuple approach
    client
        .insert_schemas::<(Person, Company, Product)>(args.clone())
        .await?;
    println!("✓ Tuple approach insertion works");

    Ok(())
}
