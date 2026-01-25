#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};
    use terminusdb_bin::TerminusDBServer;
    use terminusdb_client::{BranchSpec, DocumentInsertArgs, TerminusDBHttpClient};
    use terminusdb_schema::ToTDBInstance;
    use terminusdb_schema_derive::TerminusDBModel;

    #[derive(TerminusDBModel, Clone, Debug, PartialEq)]
    struct Person {
        name: String,
        age: i32,
    }

    // Test that demonstrates integer filtering works correctly after the fix
    #[tokio::test]
    async fn test_filter_by_integer() {
        let server = TerminusDBServer::test_instance().await.unwrap();

        server
            .with_tmp_db("test_int_filter", |client, spec| async move {
                // Insert schema
                client
                    .insert_schemas::<(Person,)>(DocumentInsertArgs::from(spec.clone()).as_schema())
                    .await
                    .unwrap();

                // Insert test data
                let people = vec![
                    Person {
                        name: "Alice".to_string(),
                        age: 25,
                    },
                    Person {
                        name: "Bob".to_string(),
                        age: 30,
                    },
                    Person {
                        name: "Charlie".to_string(),
                        age: 25,
                    },
                    Person {
                        name: "David".to_string(),
                        age: 35,
                    },
                ];

                for person in &people {
                    client
                        .insert_instance(person, DocumentInsertArgs::from(spec.clone()))
                        .await
                        .unwrap();
                }

                // Test: Filter by age (integer)
                let age_25: Vec<Person> = client
                    .list_instances_where(
                        &spec,
                        None,              // offset
                        None,              // limit
                        vec![("age", 25)], // filters
                    )
                    .await
                    .unwrap();

                assert_eq!(age_25.len(), 2, "Should find 2 people aged 25");
                assert!(age_25.iter().all(|p| p.age == 25));
                assert!(age_25.iter().any(|p| p.name == "Alice"));
                assert!(age_25.iter().any(|p| p.name == "Charlie"));

                // Test: Filter by different age
                let age_30: Vec<Person> = client
                    .list_instances_where(
                        &spec,
                        None, // offset
                        None, // limit
                        vec![("age", 30)],
                    )
                    .await
                    .unwrap();

                assert_eq!(age_30.len(), 1, "Should find 1 person aged 30");
                assert_eq!(age_30[0].name, "Bob");

                // Test: Filter by name alone for the multiple filter test
                // (mixed type filters need separate calls due to Rust's type system)
                let alice: Vec<Person> = client
                    .list_instances_where(
                        &spec,
                        None, // offset
                        None, // limit
                        vec![("name", "Alice")],
                    )
                    .await
                    .unwrap();

                // Filter the result by age locally
                let alice_25: Vec<&Person> = alice.iter().filter(|p| p.age == 25).collect();

                assert_eq!(alice_25.len(), 1, "Should find exactly Alice");
                assert_eq!(alice_25[0].name, "Alice");
                assert_eq!(alice_25[0].age, 25);
                // Also verify the initial filter worked
                assert!(alice.iter().all(|p| p.name == "Alice"));

                Ok(())
            })
            .await
            .unwrap();
    }
}
