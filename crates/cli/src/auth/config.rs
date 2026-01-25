//! Configuration file management

use super::profile::Config;
use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

/// Get the config directory path
pub fn config_dir() -> Result<PathBuf> {
    let config_dir = directories::ProjectDirs::from("com", "terminusdb", "tdb")
        .context("Could not determine config directory")?
        .config_dir()
        .to_path_buf();

    Ok(config_dir)
}

/// Get the config file path
pub fn config_file_path() -> Result<PathBuf> {
    let mut path = config_dir()?;
    path.push("config.toml");
    Ok(path)
}

/// Ensure the config directory exists
fn ensure_config_dir() -> Result<PathBuf> {
    let dir = config_dir()?;
    if !dir.exists() {
        fs::create_dir_all(&dir)
            .with_context(|| format!("Failed to create config directory: {:?}", dir))?;
    }

    // Set permissions to 0700 (user read/write/execute only) on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&dir)?.permissions();
        perms.set_mode(0o700);
        fs::set_permissions(&dir, perms)?;
    }

    Ok(dir)
}

/// Load the configuration from file
pub fn load_config() -> Result<Config> {
    let config_path = config_file_path()?;

    if !config_path.exists() {
        // Return default config if file doesn't exist
        return Ok(Config::default());
    }

    let contents = fs::read_to_string(&config_path)
        .with_context(|| format!("Failed to read config file: {:?}", config_path))?;

    let config: Config = toml::from_str(&contents)
        .with_context(|| format!("Failed to parse config file: {:?}", config_path))?;

    Ok(config)
}

/// Save the configuration to file
pub fn save_config(config: &Config) -> Result<()> {
    ensure_config_dir()?;
    let config_path = config_file_path()?;

    let contents = toml::to_string_pretty(config).context("Failed to serialize config")?;

    fs::write(&config_path, contents)
        .with_context(|| format!("Failed to write config file: {:?}", config_path))?;

    // Set permissions to 0600 (user read/write only) on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&config_path)?.permissions();
        perms.set_mode(0o600);
        fs::set_permissions(&config_path, perms)?;
    }

    Ok(())
}

/// Delete the configuration file
pub fn delete_config() -> Result<()> {
    let config_path = config_file_path()?;

    if config_path.exists() {
        fs::remove_file(&config_path)
            .with_context(|| format!("Failed to delete config file: {:?}", config_path))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_paths() {
        // Should not panic
        let _ = config_dir();
        let _ = config_file_path();
    }
}
