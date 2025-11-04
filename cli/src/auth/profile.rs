//! Profile data structures and operations

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A profile containing connection settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    /// TerminusDB server URL
    pub host: String,
    /// Username for authentication
    pub user: String,
    /// Organization name
    pub org: String,
    /// Optional default database
    #[serde(skip_serializing_if = "Option::is_none")]
    pub database: Option<String>,
    /// Optional default branch
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
}

impl Profile {
    /// Create a new profile
    pub fn new(
        host: String,
        user: String,
        org: String,
        database: Option<String>,
        branch: Option<String>,
    ) -> Self {
        Self {
            host,
            user,
            org,
            database,
            branch,
        }
    }

    /// Get the keyring service name for this profile
    pub fn keyring_service(&self) -> String {
        "terminusdb-cli".to_string()
    }

    /// Get the keyring username (profile-specific identifier)
    pub fn keyring_username(&self, profile_name: &str) -> String {
        format!("{}@{}", profile_name, self.host)
    }
}

/// Configuration settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    /// The currently active profile name
    pub active_profile: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            active_profile: "default".to_string(),
        }
    }
}

/// Complete configuration file structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Global settings
    #[serde(default)]
    pub settings: Settings,
    /// Named profiles
    #[serde(default)]
    pub profiles: HashMap<String, Profile>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            settings: Settings::default(),
            profiles: HashMap::new(),
        }
    }
}

impl Config {
    /// Get the active profile
    pub fn get_active_profile(&self) -> Option<&Profile> {
        self.profiles.get(&self.settings.active_profile)
    }

    /// Get a specific profile by name
    pub fn get_profile(&self, name: &str) -> Option<&Profile> {
        self.profiles.get(name)
    }

    /// Add or update a profile
    pub fn set_profile(&mut self, name: String, profile: Profile) {
        self.profiles.insert(name, profile);
    }

    /// Remove a profile
    pub fn remove_profile(&mut self, name: &str) -> Option<Profile> {
        self.profiles.remove(name)
    }

    /// Set the active profile
    pub fn set_active(&mut self, name: String) {
        self.settings.active_profile = name;
    }

    /// List all profile names
    pub fn profile_names(&self) -> Vec<&String> {
        self.profiles.keys().collect()
    }
}
