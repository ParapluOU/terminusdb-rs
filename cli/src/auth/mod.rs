//! Authentication and credential management module

pub mod config;
pub mod keyring;
pub mod profile;

pub use self::config::{config_dir, config_file_path, delete_config, load_config, save_config};
pub use self::keyring::{delete_password, get_password, is_keyring_available, store_password};
pub use self::profile::{Config, Profile, Settings};

use anyhow::{Context, Result};

/// Credentials resolved from various sources
#[derive(Debug, Clone)]
pub struct ResolvedCredentials {
    pub host: String,
    pub user: String,
    pub password: String,
    pub org: String,
    pub database: Option<String>,
    pub branch: Option<String>,
}

/// Get credentials for a profile
pub fn get_profile_credentials(profile_name: &str) -> Result<ResolvedCredentials> {
    let config = load_config()?;

    let profile = config
        .get_profile(profile_name)
        .with_context(|| format!("Profile '{}' not found", profile_name))?;

    let service = profile.keyring_service();
    let username = profile.keyring_username(profile_name);

    let password = get_password(&service, &username)
        .context("Failed to retrieve password from keyring. Try logging in again with 'tdb login'")?;

    Ok(ResolvedCredentials {
        host: profile.host.clone(),
        user: profile.user.clone(),
        password,
        org: profile.org.clone(),
        database: profile.database.clone(),
        branch: profile.branch.clone(),
    })
}

/// Get credentials from the active profile
pub fn get_active_credentials() -> Result<ResolvedCredentials> {
    let config = load_config()?;
    let profile_name = config.settings.active_profile.clone();
    get_profile_credentials(&profile_name)
}

/// Save a profile with password in keyring
pub fn save_profile_with_password(
    profile_name: &str,
    profile: &Profile,
    password: &str,
) -> Result<()> {
    // Save password to keyring
    let service = profile.keyring_service();
    let username = profile.keyring_username(profile_name);

    store_password(&service, &username, password)
        .context("Failed to store password in keyring")?;

    // Save profile to config
    let mut config = load_config()?;
    config.set_profile(profile_name.to_string(), profile.clone());
    save_config(&config)?;

    Ok(())
}

/// Delete a profile and its password
pub fn delete_profile(profile_name: &str) -> Result<()> {
    let mut config = load_config()?;

    let profile = config
        .get_profile(profile_name)
        .with_context(|| format!("Profile '{}' not found", profile_name))?
        .clone();

    // Delete password from keyring
    let service = profile.keyring_service();
    let username = profile.keyring_username(profile_name);

    // Ignore errors when deleting password (it might not exist)
    let _ = delete_password(&service, &username);

    // Remove profile from config
    config.remove_profile(profile_name);
    save_config(&config)?;

    Ok(())
}
