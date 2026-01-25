//! Integration tests for TerminusDB organization API endpoints
//!
//! Tests organization CRUD operations, user management, and capabilities
//! using the in-memory TerminusDBServer.

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod tests {
    use terminusdb_bin::TerminusDBServer;
    use uuid::Uuid;

    // ============================================================
    // Core Organization CRUD Tests
    // ============================================================

    /// Test listing all organizations.
    /// Default server always has "admin" organization.
    #[tokio::test]
    async fn test_list_organizations() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;
        let client = server.client().await?;

        let orgs = client.list_organizations().await?;

        // Default server has "admin" organization
        assert!(!orgs.is_empty(), "Should have at least one organization");

        let admin_org = orgs.iter().find(|o| o.name == "admin");
        assert!(admin_org.is_some(), "Should have 'admin' organization");

        // Verify structure
        let admin = admin_org.unwrap();
        assert!(
            admin.id.contains("Organization"),
            "ID should contain 'Organization'"
        );
        assert_eq!(
            admin.org_type, "Organization",
            "@type should be 'Organization'"
        );

        println!("Found {} organizations", orgs.len());
        for org in &orgs {
            println!("  - {} ({})", org.name, org.id);
        }

        Ok(())
    }

    /// Test getting a specific organization by name.
    #[tokio::test]
    async fn test_get_organization() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;
        let client = server.client().await?;

        let org = client.get_organization("admin").await?;

        assert_eq!(org.name, "admin");
        assert!(org.id.contains("Organization"));
        assert_eq!(org.org_type, "Organization");

        // Admin org should have at least the system database
        println!("Admin org databases: {:?}", org.database);

        Ok(())
    }

    /// Test creating and deleting an organization.
    #[tokio::test]
    async fn test_create_and_delete_organization() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;
        let client = server.client().await?;

        // Use UUID to avoid conflicts with parallel tests
        let org_name = format!(
            "test_org_{}",
            Uuid::new_v4().to_string().split('-').next().unwrap()
        );

        // Create organization
        let create_result = client.create_organization(&org_name).await?;
        println!("Create result: {:?}", create_result);

        // Verify it exists
        let org = client.get_organization(&org_name).await?;
        assert_eq!(org.name, org_name);

        // Delete organization
        let delete_result = client.delete_organization(&org_name).await?;
        println!("Delete result: {:?}", delete_result);

        // Verify it's gone
        let get_result = client.get_organization(&org_name).await;
        assert!(
            get_result.is_err(),
            "Organization should not exist after deletion"
        );

        Ok(())
    }

    /// Test that creating a duplicate organization fails.
    #[tokio::test]
    async fn test_create_duplicate_organization_fails() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;
        let client = server.client().await?;

        // Try to create "admin" which already exists
        let result = client.create_organization("admin").await;

        assert!(
            result.is_err(),
            "Creating duplicate organization should fail"
        );
        let err = result.unwrap_err();
        println!("Expected error: {}", err);

        Ok(())
    }

    /// Test that getting a non-existent organization returns 404.
    #[tokio::test]
    async fn test_get_nonexistent_organization_fails() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;
        let client = server.client().await?;

        let result = client.get_organization("nonexistent_org_12345").await;

        assert!(
            result.is_err(),
            "Getting non-existent organization should fail"
        );
        let err = result.unwrap_err();
        println!("Expected error: {}", err);

        Ok(())
    }

    // ============================================================
    // Organization Users Tests
    // ============================================================

    /// Test getting all users in an organization.
    #[tokio::test]
    async fn test_get_organization_users() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;
        let client = server.client().await?;

        let users = client.get_organization_users("admin").await?;

        // Admin org should have at least the admin user
        assert!(!users.is_empty(), "Should have at least one user");

        let admin_user = users.iter().find(|u| u.name == "admin");
        assert!(admin_user.is_some(), "Should have 'admin' user");

        println!("Found {} users in admin org", users.len());
        for user in &users {
            println!("  - {} ({})", user.name, user.id);
            for cap in &user.capability {
                println!("    Capability: {} scope={}", cap.id, cap.scope);
                for role in &cap.role {
                    println!("      Role: {} actions={:?}", role.name, role.action);
                }
            }
        }

        Ok(())
    }

    /// Test getting a specific user in an organization.
    #[tokio::test]
    async fn test_get_organization_user() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;
        let client = server.client().await?;

        let user = client.get_organization_user("admin", "admin").await?;

        assert_eq!(user.name, "admin");
        assert!(user.id.contains("User"));

        // Admin user should have capabilities
        assert!(
            !user.capability.is_empty(),
            "Admin user should have capabilities"
        );

        println!("Admin user: {:?}", user);

        Ok(())
    }

    /// Test getting databases available to a user in an organization.
    #[tokio::test]
    async fn test_get_organization_user_databases() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;
        let client = server.client().await?;

        let databases = client
            .get_organization_user_databases("admin", "admin")
            .await?;

        // Admin user should have access to at least the system database
        println!("Found {} databases for admin user", databases.len());
        for db in &databases {
            println!("  - {} ({}) type={}", db.name, db.id, db.db_type);
        }

        Ok(())
    }

    // ============================================================
    // User Role Management Tests
    // ============================================================

    /// Test creating a user role (capability) in an organization.
    /// This test creates a temporary organization, adds a role, then cleans up.
    #[tokio::test]
    async fn test_create_user_role() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;
        let client = server.client().await?;

        // Create a test organization
        let org_name = format!(
            "test_role_org_{}",
            Uuid::new_v4().to_string().split('-').next().unwrap()
        );
        client.create_organization(&org_name).await?;

        // Create a role for admin user in the new org
        // Scope is the resource, role is the permission level
        let result = client
            .create_user_role(
                &org_name,
                "admin",
                &format!("Organization/{}", org_name),
                "admin",
            )
            .await;

        println!("Create user role result: {:?}", result);

        // Clean up
        client.delete_organization(&org_name).await?;

        Ok(())
    }

    /// Test removing a user from an organization.
    /// Note: This is a destructive operation - be careful in tests.
    #[tokio::test]
    async fn test_remove_user_from_org() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;
        let client = server.client().await?;

        // Create a test organization
        let org_name = format!(
            "test_remove_org_{}",
            Uuid::new_v4().to_string().split('-').next().unwrap()
        );
        client.create_organization(&org_name).await?;

        // First add admin to the org with a role, then remove
        let _ = client
            .create_user_role(
                &org_name,
                "admin",
                &format!("Organization/{}", org_name),
                "admin",
            )
            .await;

        // Now remove the user from the org
        let result = client.remove_user_from_org(&org_name, "admin").await;
        println!("Remove user result: {:?}", result);

        // Clean up
        client.delete_organization(&org_name).await?;

        Ok(())
    }
}
