//! Tests for header-aware GET methods that return commit IDs

use serde::{Deserialize, Serialize};
use terminusdb_client::*;
use terminusdb_schema::*;
use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};

/// Test model for header-aware GET operations
#[derive(
    Debug, Clone, PartialEq, Serialize, Deserialize, Default, TerminusDBModel, FromTDBInstance,
)]
#[tdb(id_field = "id")]
struct TestProduct {
    id: EntityIDFor<Self>,
    name: String,
    price: f64,
    in_stock: bool,
}

/// Test setup: create a test database and client
async fn setup_test() -> anyhow::Result<(TerminusDBHttpClient, BranchSpec)> {
    let client = TerminusDBHttpClient::local_node_test().await?;
    let spec = BranchSpec::from("test");

    // Insert schema
    let args = DocumentInsertArgs::from(spec.clone());
    client.insert_entity_schema::<TestProduct>(args).await.ok();

    Ok((client, spec))
}

#[tokio::test]
#[ignore = "requires running TerminusDB instance"]
async fn test_get_instance_with_headers() -> anyhow::Result<()> {
    let (client, spec) = setup_test().await?;

    // Create a test product
    let product = TestProduct {
        id: EntityIDFor::new("laptop001")?,
        name: "Gaming Laptop".to_string(),
        price: 1299.99,
        in_stock: true,
    };

    // Insert the product
    let args = DocumentInsertArgs::from(spec.clone());
    let insert_result = client.save_instance(&product, args).await?;
    assert!(insert_result.commit_id.is_some());

    // Retrieve the product with headers
    let mut deserializer = crate::deserialize::DefaultTDBDeserializer;
    let result = client
        .get_instance_with_headers::<TestProduct>("laptop001", &spec, &mut deserializer)
        .await?;

    // Access the product via Deref
    let retrieved_product = &*result;

    // Verify the product was retrieved correctly
    assert_eq!(retrieved_product.name, "Gaming Laptop");
    assert_eq!(retrieved_product.price, 1299.99);
    assert_eq!(retrieved_product.in_stock, true);

    // Verify we got a commit ID
    let commit_id = result.extract_commit_id();
    assert!(
        commit_id.is_some(),
        "Expected commit ID in response headers"
    );
    let commit_id = commit_id.unwrap();
    assert!(
        commit_id.starts_with("ValidCommit/"),
        "Expected ValidCommit/ prefix, got: {}",
        commit_id
    );

    println!("Retrieved product from commit: {}", commit_id);

    Ok(())
}

#[tokio::test]
#[ignore = "requires running TerminusDB instance"]
async fn test_get_instances_with_headers() -> anyhow::Result<()> {
    let (client, spec) = setup_test().await?;

    // Create multiple test products
    let products = vec![
        TestProduct {
            id: EntityIDFor::new("mouse001")?,
            name: "Wireless Mouse".to_string(),
            price: 29.99,
            in_stock: true,
        },
        TestProduct {
            id: EntityIDFor::new("keyboard001")?,
            name: "Mechanical Keyboard".to_string(),
            price: 79.99,
            in_stock: false,
        },
        TestProduct {
            id: EntityIDFor::new("monitor001")?,
            name: "4K Monitor".to_string(),
            price: 399.99,
            in_stock: true,
        },
    ];

    // Insert all products
    let args = DocumentInsertArgs::from(spec.clone());
    let insert_result = client.insert_instances(products, args).await?;
    assert_eq!(insert_result.len(), 3);

    // Retrieve specific products with headers
    let ids = vec!["mouse001".to_string(), "keyboard001".to_string()];
    let opts = GetOpts::default();
    let mut deserializer = crate::deserialize::DefaultTDBDeserializer;

    let result = client
        .get_instances_with_headers::<TestProduct>(ids, &spec, opts, &mut deserializer)
        .await?;

    // Access the products via Deref
    let retrieved_products = &*result;

    // Verify we got the right products
    assert_eq!(retrieved_products.len(), 2);

    let mouse = retrieved_products
        .iter()
        .find(|p| p.name == "Wireless Mouse")
        .unwrap();
    assert_eq!(mouse.price, 29.99);

    let keyboard = retrieved_products
        .iter()
        .find(|p| p.name == "Mechanical Keyboard")
        .unwrap();
    assert_eq!(keyboard.price, 79.99);

    // Verify we got a commit ID
    let commit_id = result.extract_commit_id();
    assert!(
        commit_id.is_some(),
        "Expected commit ID in response headers"
    );
    let commit_id = commit_id.unwrap();
    println!(
        "Retrieved {} products from commit: {}",
        retrieved_products.len(),
        commit_id
    );

    Ok(())
}

#[tokio::test]
#[ignore = "requires running TerminusDB instance"]
async fn test_get_all_instances_with_headers() -> anyhow::Result<()> {
    let (client, spec) = setup_test().await?;

    // Create test products
    let products = vec![
        TestProduct {
            id: EntityIDFor::new("headset001")?,
            name: "Gaming Headset".to_string(),
            price: 89.99,
            in_stock: true,
        },
        TestProduct {
            id: EntityIDFor::new("webcam001")?,
            name: "HD Webcam".to_string(),
            price: 59.99,
            in_stock: true,
        },
    ];

    // Insert products
    let args = DocumentInsertArgs::from(spec.clone());
    client.insert_instances(products, args).await?;

    // Retrieve ALL products of this type with headers
    let empty_ids = vec![]; // empty means get all
    let opts = GetOpts::filtered_by_type::<TestProduct>();
    let mut deserializer = crate::deserialize::DefaultTDBDeserializer;

    let result = client
        .get_instances_with_headers::<TestProduct>(empty_ids, &spec, opts, &mut deserializer)
        .await?;

    // Access the products via Deref
    let all_products = &*result;

    // Verify we got products (should be at least the 2 we just inserted)
    assert!(
        all_products.len() >= 2,
        "Expected at least 2 products, got {}",
        all_products.len()
    );

    // Verify we got a commit ID
    let commit_id = result.extract_commit_id();
    assert!(
        commit_id.is_some(),
        "Expected commit ID in response headers"
    );
    let commit_id = commit_id.unwrap();
    println!(
        "Retrieved {} total products from commit: {}",
        all_products.len(),
        commit_id
    );

    Ok(())
}

#[tokio::test]
#[ignore = "requires running TerminusDB instance"]
async fn test_simplified_get_latest_version() -> anyhow::Result<()> {
    let (client, spec) = setup_test().await?;

    // Create and insert a product
    let product = TestProduct {
        id: EntityIDFor::new("tablet001")?,
        name: "Android Tablet".to_string(),
        price: 299.99,
        in_stock: true,
    };

    let args = DocumentInsertArgs::from(spec.clone());
    client.save_instance(&product, args.clone()).await?;

    // Get the latest version using the simplified method
    let latest_commit = client
        .get_latest_version::<TestProduct>("tablet001", &spec)
        .await?;

    assert!(
        latest_commit.starts_with("ValidCommit/"),
        "Expected ValidCommit/ prefix, got: {}",
        latest_commit
    );
    println!("Latest version commit ID: {}", latest_commit);

    // Update the product
    let updated_product = TestProduct {
        id: EntityIDFor::new("tablet001")?,
        name: "Android Tablet Pro".to_string(),
        price: 399.99,
        in_stock: false,
    };

    client.save_instance(&updated_product, args).await?;

    // Get the new latest version
    let new_latest_commit = client
        .get_latest_version::<TestProduct>("tablet001", &spec)
        .await?;

    // Verify it's a different commit
    assert_ne!(
        latest_commit, new_latest_commit,
        "Expected different commit ID after update"
    );
    println!("New latest version commit ID: {}", new_latest_commit);

    Ok(())
}

#[tokio::test]
#[ignore = "requires running TerminusDB instance"]
async fn test_commit_id_consistency() -> anyhow::Result<()> {
    let (client, spec) = setup_test().await?;

    // Create a product
    let product = TestProduct {
        id: EntityIDFor::new("speaker001")?,
        name: "Bluetooth Speaker".to_string(),
        price: 49.99,
        in_stock: true,
    };

    // Insert and get the commit ID from the insert response
    let args = DocumentInsertArgs::from(spec.clone());
    let (result, insert_commit_id) = client
        .insert_instance_with_commit_id(&product, args)
        .await?;

    println!("Insert commit ID: {}", insert_commit_id);

    // Retrieve the same instance and get commit ID from GET response
    let mut deserializer = crate::deserialize::DefaultTDBDeserializer;
    let result = client
        .get_instance_with_headers::<TestProduct>("speaker001", &spec, &mut deserializer)
        .await?;

    let get_commit_id = result
        .extract_commit_id()
        .expect("Expected commit ID from GET");
    println!("GET commit ID: {}", get_commit_id);

    // The commit IDs should match since we're on the same branch and no other commits were made
    assert_eq!(
        insert_commit_id, get_commit_id,
        "Commit ID from insert should match commit ID from GET on the same branch"
    );

    Ok(())
}
