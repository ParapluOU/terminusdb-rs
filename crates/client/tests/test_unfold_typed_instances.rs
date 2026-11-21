//! End-to-end roundtrip coverage for **nested models** — a document that links to
//! other *separate* documents (graph edges / `Ref<T>`, not owned subdocuments),
//! inserted and then read back so the nested models come inline.
//!
//! Shape: `Company --ceo/employees--> Person --address--> Address`, i.e. two levels
//! of `Ref<T>` links. This complements `e2e_model_roundtrip.rs`, whose link case
//! only checks that the *id* survives — here we assert the nested models' **fields**
//! come back.
//!
//! ## What actually drives unfolding (TerminusDB v12)
//!
//! Expansion of a **linked** document is *schema-driven*, not forced by the API
//! `unfold` flag. A `Ref<T>` field comes back inline only when the **target class**
//! is `#[tdb(unfoldable = true)]` (`@unfoldable`) — or, in raw TerminusDB, when the
//! property carries `@unfold: true` (field-level, which the derive does not yet
//! emit). A link to a class with neither annotation stays a bare id reference even
//! when `unfold=true` is requested. See `docs/terminusdb/document-unfolding-reference.md`.
//!
//! (This file was previously disabled: it used a since-removed `#[tdb(instance)]`
//! field attribute and was walled off behind a never-enabled `#[cfg(feature = ...)]`,
//! so it never compiled. Rewritten to the current `Ref<T>` idiom, corrected to the
//! v12 unfolding semantics above, and run on the embedded `TerminusDBServer`.)

#![recursion_limit = "256"]
#![cfg(not(target_arch = "wasm32"))]

use anyhow::Result;
use terminusdb_bin::TerminusDBServer;
use terminusdb_client::*;
use terminusdb_schema::*;
use terminusdb_schema_derive::{FromTDBInstance, TerminusDBModel};

// `@unfoldable`: references to an Address expand inline on retrieval.
#[derive(Debug, Clone, PartialEq, TerminusDBModel, FromTDBInstance)]
#[tdb(id_field = "id", unfoldable = true)]
struct Address {
    id: EntityIDFor<Self>,
    street: String,
    city: String,
    postal_code: String,
}

// NOT unfoldable: references to a Department stay bare id references (contrast case).
#[derive(Debug, Clone, PartialEq, TerminusDBModel, FromTDBInstance)]
#[tdb(id_field = "id")]
struct Department {
    id: EntityIDFor<Self>,
    name: String,
}

// `@unfoldable`, and itself links to an (unfoldable) Address and a (non-unfoldable)
// Department.
#[derive(Debug, Clone, PartialEq, TerminusDBModel, FromTDBInstance)]
#[tdb(id_field = "id", unfoldable = true)]
struct Person {
    id: EntityIDFor<Self>,
    name: String,
    age: i32,
    address: Option<Ref<Address>>,
    department: Option<Ref<Department>>,
}

// NOT unfoldable; links to (unfoldable) Person documents.
#[derive(Debug, Clone, PartialEq, TerminusDBModel, FromTDBInstance)]
#[tdb(id_field = "id")]
struct Company {
    id: EntityIDFor<Self>,
    name: String,
    ceo: Option<Ref<Person>>,
    employees: Vec<Ref<Person>>,
}

/// Insert the schemas and a small graph: two addresses, one department, two people
/// (each linking an address; alice also linking the department), and a company
/// whose CEO and employees link those people.
async fn setup_test_data(client: &TerminusDBHttpClient, spec: &BranchSpec) -> Result<()> {
    let args = DocumentInsertArgs::from(spec.clone());
    client.insert_entity_schema::<Address>(args.clone()).await?;
    client
        .insert_entity_schema::<Department>(args.clone())
        .await?;
    client.insert_entity_schema::<Person>(args.clone()).await?;
    client.insert_entity_schema::<Company>(args.clone()).await?;

    let home_address = Address {
        id: EntityIDFor::new("home").unwrap(),
        street: "123 Main St".to_string(),
        city: "Springfield".to_string(),
        postal_code: "12345".to_string(),
    };
    let office_address = Address {
        id: EntityIDFor::new("office").unwrap(),
        street: "456 Business Ave".to_string(),
        city: "Corporate City".to_string(),
        postal_code: "67890".to_string(),
    };
    let engineering = Department {
        id: EntityIDFor::new("eng").unwrap(),
        name: "Engineering".to_string(),
    };

    let alice = Person {
        id: EntityIDFor::new("alice").unwrap(),
        name: "Alice Smith".to_string(),
        age: 30,
        address: Some(Ref::from(EntityIDFor::<Address>::new("home").unwrap())),
        department: Some(Ref::from(EntityIDFor::<Department>::new("eng").unwrap())),
    };
    let bob = Person {
        id: EntityIDFor::new("bob").unwrap(),
        name: "Bob Johnson".to_string(),
        age: 35,
        address: Some(Ref::from(EntityIDFor::<Address>::new("office").unwrap())),
        department: None,
    };

    let company = Company {
        id: EntityIDFor::new("acme").unwrap(),
        name: "ACME Corp".to_string(),
        ceo: Some(Ref::from(EntityIDFor::<Person>::new("alice").unwrap())),
        employees: vec![
            Ref::from(EntityIDFor::<Person>::new("alice").unwrap()),
            Ref::from(EntityIDFor::<Person>::new("bob").unwrap()),
        ],
    };

    client.insert_instance(&home_address, args.clone()).await?;
    client
        .insert_instance(&office_address, args.clone())
        .await?;
    client.insert_instance(&engineering, args.clone()).await?;
    client.insert_instance(&alice, args.clone()).await?;
    client.insert_instance(&bob, args.clone()).await?;
    client.insert_instance(&company, args.clone()).await?;

    Ok(())
}

/// A `Ref<T>` to an `@unfoldable` class roundtrips its full nested data.
#[tokio::test]
async fn nested_link_roundtrips_when_target_is_unfoldable() -> Result<()> {
    let server = TerminusDBServer::test_instance().await?;
    server
        .with_tmp_db("test_unfold_link", |client, spec| async move {
            setup_test_data(&client, &spec).await?;
            let mut de = DefaultTDBDeserializer;

            let alice: Person = client.get_instance("alice", &spec, &mut de).await?;

            let address = alice.address.as_ref().expect("alice has an address link");
            assert!(
                address.is_loaded(),
                "link to @unfoldable Address should come back inline"
            );
            let address = address.get_expect();
            assert_eq!(address.id.id(), "home");
            assert_eq!(address.street, "123 Main St");
            assert_eq!(address.city, "Springfield");
            assert_eq!(address.postal_code, "12345");

            Ok(())
        })
        .await
}

/// A `Ref<T>` to a class that is **not** `@unfoldable` stays a bare id reference,
/// even though the read requests `unfold=true`. This is correct TerminusDB v12
/// behavior — and the most likely cause of a "nested roundtrip is broken" report:
/// the nested target class simply isn't marked `@unfoldable`.
#[tokio::test]
async fn link_to_non_unfoldable_class_stays_reference() -> Result<()> {
    let server = TerminusDBServer::test_instance().await?;
    server
        .with_tmp_db("test_unfold_ref_only", |client, spec| async move {
            setup_test_data(&client, &spec).await?;
            let mut de = DefaultTDBDeserializer;

            // Unfolded read, but Department is not @unfoldable.
            let alice: Person = client
                .get_instance_unfolded("alice", &spec, &mut de)
                .await?;

            let dept = alice
                .department
                .as_ref()
                .expect("alice has a department link");
            assert!(
                !dept.is_loaded(),
                "link to non-@unfoldable Department must stay a bare reference"
            );
            assert_eq!(dept.id().id(), "eng");

            // The @unfoldable Address on the same document still unfolds.
            assert!(alice.address.as_ref().unwrap().is_loaded());

            Ok(())
        })
        .await
}

/// Deep nested roundtrip across two link levels: Company -> Person -> Address.
/// Company is not `@unfoldable`, so we unfold explicitly; Person and Address are,
/// so both levels come back inline.
#[tokio::test]
async fn deep_nested_roundtrip_unfolds_all_levels() -> Result<()> {
    let server = TerminusDBServer::test_instance().await?;
    server
        .with_tmp_db("test_unfold_deep", |client, spec| async move {
            setup_test_data(&client, &spec).await?;
            let mut de = DefaultTDBDeserializer;

            let company: Company = client.get_instance_unfolded("acme", &spec, &mut de).await?;
            assert_eq!(company.name, "ACME Corp");

            // Level 1: CEO link -> full Person.
            let ceo = company.ceo.as_ref().expect("company has a CEO");
            assert!(ceo.is_loaded(), "CEO link should be unfolded");
            let ceo = ceo.get_expect();
            assert_eq!(ceo.name, "Alice Smith");
            assert_eq!(ceo.age, 30);

            // Level 2: the CEO's own address link -> full Address.
            let ceo_address = ceo.address.as_ref().expect("CEO has an address link");
            assert!(
                ceo_address.is_loaded(),
                "nested CEO address should be unfolded (deep unfold)"
            );
            assert_eq!(ceo_address.get_expect().street, "123 Main St");

            // Vec of links: employees all unfolded.
            assert_eq!(company.employees.len(), 2);
            assert!(
                company.employees.iter().all(|e| e.is_loaded()),
                "all employee links should be unfolded"
            );
            let mut names: Vec<&str> = company
                .employees
                .iter()
                .map(|e| e.get_expect().name.as_str())
                .collect();
            names.sort();
            assert_eq!(names, vec!["Alice Smith", "Bob Johnson"]);

            Ok(())
        })
        .await
}

/// Bulk retrieval unfolds nested links for every returned document.
#[tokio::test]
async fn bulk_get_unfolds_nested_links() -> Result<()> {
    let server = TerminusDBServer::test_instance().await?;
    server
        .with_tmp_db("test_unfold_bulk", |client, spec| async move {
            setup_test_data(&client, &spec).await?;
            let mut de = DefaultTDBDeserializer;

            let ids = vec!["alice".to_string(), "bob".to_string()];
            let people: Vec<Person> = client.get_instances_unfolded(ids, &spec, &mut de).await?;

            assert_eq!(people.len(), 2);
            for person in &people {
                let address = person.address.as_ref().expect("person has an address link");
                assert!(address.is_loaded(), "each address should be unfolded");
                let address = address.get_expect();
                assert!(!address.street.is_empty());
                assert!(!address.city.is_empty());
            }

            Ok(())
        })
        .await
}

/// Type-filtered, paginated, unfolded bulk retrieval.
#[tokio::test]
async fn pagination_with_unfold_and_type_filter() -> Result<()> {
    let server = TerminusDBServer::test_instance().await?;
    server
        .with_tmp_db("test_unfold_pagination", |client, spec| async move {
            setup_test_data(&client, &spec).await?;
            let mut de = DefaultTDBDeserializer;

            let empty_ids: Vec<String> = vec![];
            let opts = GetOpts::filtered_by_type::<Person>()
                .with_count(1)
                .with_unfold(true);
            let people: Vec<Person> = client
                .get_instances_with_opts(empty_ids, &spec, opts, &mut de)
                .await?;

            assert_eq!(people.len(), 1);
            let address = people[0]
                .address
                .as_ref()
                .expect("person has an address link");
            assert!(address.is_loaded(), "address should be unfolded");

            Ok(())
        })
        .await
}
