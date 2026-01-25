//! Integration tests for the open_database API

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod test_open_database {
    use terminusdb_bin::TerminusDBServer;
    use terminusdb_client::*;
    use terminusdb_schema::{tdbseeder, ToTDBInstance};
    use terminusdb_schema_derive::TerminusDBModel;

    /// A simple test model for seeding
    #[derive(Debug, Clone, PartialEq, TerminusDBModel)]
    struct TestUser {
        name: String,
        email: String,
    }

    /// A second model to test multi-schema scenarios
    #[derive(Debug, Clone, PartialEq, TerminusDBModel)]
    struct TestProject {
        title: String,
        active: bool,
    }

    // ========== Complex Models for Comprehensive Testing ==========

    /// Address as a subdocument (embedded in parent)
    #[derive(Debug, Clone, PartialEq, TerminusDBModel)]
    #[tdb(subdocument)]
    struct Address {
        street: String,
        city: String,
        country: String,
        postal_code: Option<String>,
    }

    /// Contact info with optional fields
    #[derive(Debug, Clone, PartialEq, TerminusDBModel)]
    #[tdb(subdocument)]
    struct ContactInfo {
        phone: Option<String>,
        website: Option<String>,
    }

    /// Company with nested subdocuments
    #[derive(Debug, Clone, PartialEq, TerminusDBModel)]
    struct Company {
        name: String,
        industry: String,
        address: Address,
        contact: Option<ContactInfo>,
        employee_count: i32,
    }

    /// Employee with relation to Company
    #[derive(Debug, Clone, PartialEq, TerminusDBModel)]
    struct Employee {
        full_name: String,
        role: String,
        salary: i32,
        remote: bool,
    }

    /// Department with multiple employees
    #[derive(Debug, Clone, PartialEq, TerminusDBModel)]
    struct Department {
        name: String,
        budget: i32,
    }

    /// Test that open_database creates a new database and seeds it
    #[tokio::test]
    async fn test_open_database_creates_and_seeds() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;
        let client = server.client().await?;

        // Use a unique database name for this test
        let db_name = format!("test_open_db_{}", uuid::Uuid::new_v4());

        // First open: should create and seed
        let result = client
            .open_database::<(TestUser,), _>(
                &db_name,
                tdbseeder![TestUser {
                    name: "Admin".to_string(),
                    email: "admin@test.com".to_string(),
                }],
            )
            .await?;

        assert!(result.was_created, "Database should have been created");
        assert!(result.was_seeded, "Database should have been seeded");

        // Verify the seeded data exists
        let spec = BranchSpec::with_branch(&db_name, "main");
        let count = client.count_instances::<TestUser>(&spec).await?;
        assert_eq!(count, 1, "Should have one seeded user");

        // Cleanup
        client.delete_database(&db_name).await?;

        Ok(())
    }

    /// Test that reopening an existing database is a no-op
    #[tokio::test]
    async fn test_open_database_reopen_is_noop() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;
        let client = server.client().await?;

        let db_name = format!("test_open_reopen_{}", uuid::Uuid::new_v4());

        // First open: create and seed
        let first_result = client
            .open_database::<(TestUser,), _>(
                &db_name,
                tdbseeder![TestUser {
                    name: "First".to_string(),
                    email: "first@test.com".to_string(),
                }],
            )
            .await?;

        assert!(first_result.was_created);
        assert!(first_result.was_seeded);

        // Second open: should be no-op (seeder should NOT run)
        let second_result = client
            .open_database::<(TestUser,), _>(
                &db_name,
                tdbseeder![TestUser {
                    name: "Second".to_string(),
                    email: "second@test.com".to_string(),
                }],
            )
            .await?;

        assert!(
            !second_result.was_created,
            "Database should not be newly created"
        );
        assert!(!second_result.was_seeded, "Seeder should not run on reopen");

        // Verify only the first user exists (seeder didn't run twice)
        let spec = BranchSpec::with_branch(&db_name, "main");
        let count = client.count_instances::<TestUser>(&spec).await?;
        assert_eq!(count, 1, "Should still have only one user");

        // Cleanup
        client.delete_database(&db_name).await?;

        Ok(())
    }

    /// Test that open_database_no_seed works without a seeder
    #[tokio::test]
    async fn test_open_database_no_seed() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;
        let client = server.client().await?;

        let db_name = format!("test_open_no_seed_{}", uuid::Uuid::new_v4());

        // Open without seeder
        let result = client
            .open_database_no_seed::<(TestUser, TestProject)>(&db_name)
            .await?;

        assert!(result.was_created);
        assert!(!result.was_seeded);

        // Verify no data exists but schema is inserted
        let spec = BranchSpec::with_branch(&db_name, "main");
        let user_count = client.count_instances::<TestUser>(&spec).await?;
        let project_count = client.count_instances::<TestProject>(&spec).await?;
        assert_eq!(user_count, 0);
        assert_eq!(project_count, 0);

        // Cleanup
        client.delete_database(&db_name).await?;

        Ok(())
    }

    /// Test that schema hash changes are detected
    #[tokio::test]
    async fn test_open_database_schema_migration_required() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;
        let client = server.client().await?;

        let db_name = format!("test_open_migration_{}", uuid::Uuid::new_v4());

        // First open with TestUser schema
        let _ = client
            .open_database_no_seed::<(TestUser,)>(&db_name)
            .await?;

        // Second open with different schema (TestProject instead of TestUser)
        // This should fail with SchemaMigrationRequired
        let result = client
            .open_database_no_seed::<(TestProject,)>(&db_name)
            .await;

        assert!(result.is_err());
        let err = result.unwrap_err();

        match err {
            OpenDatabaseError::SchemaMigrationRequired { expected, current } => {
                // The hashes should be different
                assert_ne!(expected, current, "Hash values should differ");
            }
            other => {
                panic!("Expected SchemaMigrationRequired, got: {:?}", other);
            }
        }

        // Cleanup
        client.delete_database(&db_name).await?;

        Ok(())
    }

    /// Test that the same schema produces the same hash (idempotent)
    #[tokio::test]
    async fn test_schema_hash_is_deterministic() {
        let hash1 = compute_schema_hash::<(TestUser,)>();
        let hash2 = compute_schema_hash::<(TestUser,)>();
        assert_eq!(hash1, hash2, "Same schema should produce same hash");

        // Different schemas should produce different hashes
        let hash3 = compute_schema_hash::<(TestProject,)>();
        assert_ne!(
            hash1, hash3,
            "Different schemas should produce different hashes"
        );

        // Order shouldn't matter (due to sorting)
        let hash_ab = compute_schema_hash::<(TestUser, TestProject)>();
        let hash_ba = compute_schema_hash::<(TestProject, TestUser)>();
        assert_eq!(hash_ab, hash_ba, "Schema order should not affect hash");
    }

    /// Test seeding with multiple model types
    #[tokio::test]
    async fn test_open_database_multi_model_seed() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;
        let client = server.client().await?;

        let db_name = format!("test_open_multi_{}", uuid::Uuid::new_v4());

        // Open with multiple schemas and seed both
        let result = client
            .open_database::<(TestUser, TestProject), _>(
                &db_name,
                tdbseeder![
                    TestUser {
                        name: "User1".to_string(),
                        email: "user1@test.com".to_string(),
                    },
                    vec![
                        TestProject {
                            title: "Project A".to_string(),
                            active: true,
                        },
                        TestProject {
                            title: "Project B".to_string(),
                            active: false,
                        },
                    ],
                ],
            )
            .await?;

        assert!(result.was_created);
        assert!(result.was_seeded);

        // Verify both types were seeded
        let spec = BranchSpec::with_branch(&db_name, "main");
        let user_count = client.count_instances::<TestUser>(&spec).await?;
        let project_count = client.count_instances::<TestProject>(&spec).await?;
        assert_eq!(user_count, 1);
        assert_eq!(project_count, 2);

        // Cleanup
        client.delete_database(&db_name).await?;

        Ok(())
    }

    /// Test that tdbseeder! works with function calls (lazy evaluation)
    #[tokio::test]
    async fn test_open_database_seeder_with_functions() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;
        let client = server.client().await?;

        let db_name = format!("test_open_fn_seed_{}", uuid::Uuid::new_v4());

        // Helper functions that create seed data
        fn create_admin_user() -> TestUser {
            TestUser {
                name: "Admin".to_string(),
                email: "admin@example.com".to_string(),
            }
        }

        fn create_default_projects() -> Vec<TestProject> {
            vec![
                TestProject {
                    title: "Default Project".to_string(),
                    active: true,
                },
                TestProject {
                    title: "Archive".to_string(),
                    active: false,
                },
            ]
        }

        // Use function calls inside tdbseeder! - they are evaluated lazily
        let result = client
            .open_database::<(TestUser, TestProject), _>(
                &db_name,
                tdbseeder![create_admin_user(), create_default_projects()],
            )
            .await?;

        assert!(result.was_created);
        assert!(result.was_seeded);

        // Verify the function results were seeded
        let spec = BranchSpec::with_branch(&db_name, "main");
        let users: Vec<TestUser> = client.list_instances(&spec, None, None).await?;
        assert_eq!(users.len(), 1);
        assert_eq!(users[0].name, "Admin");

        let projects: Vec<TestProject> = client.list_instances(&spec, None, None).await?;
        assert_eq!(projects.len(), 2);

        // Cleanup
        client.delete_database(&db_name).await?;

        Ok(())
    }

    /// Comprehensive test: complex nested schemas with subdocuments,
    /// optional fields, and multiple instances. Verifies all data is
    /// correctly inserted and retrievable after seeding.
    #[tokio::test]
    async fn test_open_database_complex_schema_and_verify_data() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;
        let client = server.client().await?;

        let db_name = format!("test_open_complex_{}", uuid::Uuid::new_v4());

        // Create complex seed data
        let tech_corp = Company {
            name: "TechCorp".to_string(),
            industry: "Technology".to_string(),
            address: Address {
                street: "123 Innovation Way".to_string(),
                city: "San Francisco".to_string(),
                country: "USA".to_string(),
                postal_code: Some("94105".to_string()),
            },
            contact: Some(ContactInfo {
                phone: Some("+1-555-0100".to_string()),
                website: Some("https://techcorp.example".to_string()),
            }),
            employee_count: 500,
        };

        let startup_inc = Company {
            name: "Startup Inc".to_string(),
            industry: "Fintech".to_string(),
            address: Address {
                street: "456 Venture Blvd".to_string(),
                city: "Austin".to_string(),
                country: "USA".to_string(),
                postal_code: None, // Test optional None
            },
            contact: None, // Test optional None at struct level
            employee_count: 25,
        };

        let employees = vec![
            Employee {
                full_name: "Alice Johnson".to_string(),
                role: "Senior Engineer".to_string(),
                salary: 150000,
                remote: true,
            },
            Employee {
                full_name: "Bob Smith".to_string(),
                role: "Product Manager".to_string(),
                salary: 130000,
                remote: false,
            },
            Employee {
                full_name: "Carol Williams".to_string(),
                role: "Designer".to_string(),
                salary: 110000,
                remote: true,
            },
        ];

        let departments = vec![
            Department {
                name: "Engineering".to_string(),
                budget: 2000000,
            },
            Department {
                name: "Marketing".to_string(),
                budget: 500000,
            },
        ];

        // Open database with complex seeding
        let result = client
            .open_database::<(Company, Employee, Department, Address, ContactInfo), _>(
                &db_name,
                tdbseeder![
                    tech_corp.clone(),
                    startup_inc.clone(),
                    employees.clone(),
                    departments.clone(),
                ],
            )
            .await?;

        assert!(result.was_created, "Database should be newly created");
        assert!(result.was_seeded, "Database should have been seeded");

        // Verify counts
        let spec = BranchSpec::with_branch(&db_name, "main");

        let company_count = client.count_instances::<Company>(&spec).await?;
        assert_eq!(company_count, 2, "Should have 2 companies");

        let employee_count = client.count_instances::<Employee>(&spec).await?;
        assert_eq!(employee_count, 3, "Should have 3 employees");

        let department_count = client.count_instances::<Department>(&spec).await?;
        assert_eq!(department_count, 2, "Should have 2 departments");

        // Retrieve and verify actual data
        // Verify companies with nested subdocuments
        let companies: Vec<Company> = client.list_instances::<Company>(&spec, None, None).await?;
        assert_eq!(companies.len(), 2);

        // Find TechCorp and verify nested data
        let tech = companies
            .iter()
            .find(|c| c.name == "TechCorp")
            .expect("TechCorp should exist");
        assert_eq!(tech.industry, "Technology");
        assert_eq!(tech.address.city, "San Francisco");
        assert_eq!(tech.address.postal_code, Some("94105".to_string()));
        assert!(tech.contact.is_some());
        let contact = tech.contact.as_ref().unwrap();
        assert_eq!(contact.phone, Some("+1-555-0100".to_string()));

        // Find Startup Inc and verify optional None values
        let startup = companies
            .iter()
            .find(|c| c.name == "Startup Inc")
            .expect("Startup Inc should exist");
        assert_eq!(startup.address.postal_code, None);
        assert!(startup.contact.is_none());

        // Verify employees
        let retrieved_employees: Vec<Employee> =
            client.list_instances::<Employee>(&spec, None, None).await?;
        assert_eq!(retrieved_employees.len(), 3);

        let alice = retrieved_employees
            .iter()
            .find(|e| e.full_name == "Alice Johnson")
            .expect("Alice should exist");
        assert_eq!(alice.role, "Senior Engineer");
        assert_eq!(alice.salary, 150000);
        assert!(alice.remote);

        // Verify departments
        let retrieved_departments: Vec<Department> = client
            .list_instances::<Department>(&spec, None, None)
            .await?;
        assert_eq!(retrieved_departments.len(), 2);

        let eng = retrieved_departments
            .iter()
            .find(|d| d.name == "Engineering")
            .expect("Engineering dept should exist");
        assert_eq!(eng.budget, 2000000);

        // Verify reopening doesn't re-seed
        let reopen_result = client
            .open_database::<(Company, Employee, Department, Address, ContactInfo), _>(
                &db_name,
                tdbseeder![Company {
                    name: "NewCompany".to_string(),
                    industry: "Other".to_string(),
                    address: Address {
                        street: "789 New St".to_string(),
                        city: "New York".to_string(),
                        country: "USA".to_string(),
                        postal_code: None,
                    },
                    contact: None,
                    employee_count: 10,
                }],
            )
            .await?;

        assert!(!reopen_result.was_created);
        assert!(!reopen_result.was_seeded);

        // Verify count is still the same (seeder didn't run)
        let final_company_count = client.count_instances::<Company>(&spec).await?;
        assert_eq!(
            final_company_count, 2,
            "Company count should still be 2 after reopen"
        );

        // Cleanup
        client.delete_database(&db_name).await?;

        Ok(())
    }
}
