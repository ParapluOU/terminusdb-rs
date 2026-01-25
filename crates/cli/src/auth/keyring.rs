//! Keyring integration for secure password storage

use anyhow::{Context, Result};
use keyring::Entry;

/// Store a password in the system keyring
pub fn store_password(service: &str, username: &str, password: &str) -> Result<()> {
    let entry = Entry::new(service, username).context("Failed to create keyring entry")?;

    entry
        .set_password(password)
        .context("Failed to store password in keyring")?;

    Ok(())
}

/// Retrieve a password from the system keyring
pub fn get_password(service: &str, username: &str) -> Result<String> {
    let entry = Entry::new(service, username).context("Failed to create keyring entry")?;

    let password = entry
        .get_password()
        .context("Failed to retrieve password from keyring")?;

    Ok(password)
}

/// Delete a password from the system keyring
pub fn delete_password(service: &str, username: &str) -> Result<()> {
    let entry = Entry::new(service, username).context("Failed to create keyring entry")?;

    entry
        .delete_credential()
        .context("Failed to delete password from keyring")?;

    Ok(())
}

/// Check if keyring is available and working
pub fn is_keyring_available(service: &str, username: &str) -> bool {
    // Try to create an entry and set/get a test value
    if let Ok(entry) = Entry::new(service, username) {
        let test_password = "test";
        if entry.set_password(test_password).is_ok() {
            if let Ok(retrieved) = entry.get_password() {
                let _ = entry.delete_credential();
                return retrieved == test_password;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keyring_operations() {
        let service = "tdb-test";
        let username = "test-user";
        let password = "test-password";

        // Skip test if keyring is not available
        if !is_keyring_available(service, username) {
            eprintln!("Keyring not available, skipping test");
            return;
        }

        // Store password
        store_password(service, username, password).expect("Failed to store password");

        // Retrieve password
        let retrieved = get_password(service, username).expect("Failed to retrieve password");
        assert_eq!(retrieved, password);

        // Delete password
        delete_password(service, username).expect("Failed to delete password");

        // Verify deletion
        assert!(get_password(service, username).is_err());
    }
}
