//! Profile and authentication management commands (login/logout/profile).

use anyhow::{Context, Result};

// Profile and authentication management functions

pub(crate) async fn run_login(profile_name: &str) -> Result<()> {
    use std::io::{self, Write};

    println!("Logging in to profile: {}", profile_name);
    println!();

    // Prompt for connection details
    print!("TerminusDB Host (default: http://localhost:6363): ");
    io::stdout().flush()?;
    let mut host = String::new();
    io::stdin().read_line(&mut host)?;
    let host = host.trim();
    let host = if host.is_empty() {
        "http://localhost:6363".to_string()
    } else {
        host.to_string()
    };

    print!("Username (default: admin): ");
    io::stdout().flush()?;
    let mut user = String::new();
    io::stdin().read_line(&mut user)?;
    let user = user.trim();
    let user = if user.is_empty() {
        "admin".to_string()
    } else {
        user.to_string()
    };

    let password = rpassword::prompt_password("Password: ")?;

    print!("Organization (default: admin): ");
    io::stdout().flush()?;
    let mut org = String::new();
    io::stdin().read_line(&mut org)?;
    let org = org.trim();
    let org = if org.is_empty() {
        "admin".to_string()
    } else {
        org.to_string()
    };

    print!("Default Database (optional, press Enter to skip): ");
    io::stdout().flush()?;
    let mut database = String::new();
    io::stdin().read_line(&mut database)?;
    let database = database.trim();
    let database = if database.is_empty() {
        None
    } else {
        Some(database.to_string())
    };

    print!("Default Branch (optional, press Enter to skip): ");
    io::stdout().flush()?;
    let mut branch = String::new();
    io::stdin().read_line(&mut branch)?;
    let branch = branch.trim();
    let branch = if branch.is_empty() {
        None
    } else {
        Some(branch.to_string())
    };

    // Create profile
    let profile = crate::auth::Profile::new(host, user, org, database, branch);

    // Save profile with password
    crate::auth::save_profile_with_password(profile_name, &profile, &password)?;

    // Set as active profile
    let mut config = crate::auth::load_config()?;
    config.set_active(profile_name.to_string());
    crate::auth::save_config(&config)?;

    println!();
    println!(
        "Successfully logged in and set '{}' as active profile",
        profile_name
    );
    println!("Credentials saved to system keyring");

    Ok(())
}

pub(crate) async fn run_logout(profile_name: Option<&str>) -> Result<()> {
    let config = crate::auth::load_config()?;

    let profile_to_logout = match profile_name {
        Some(name) => name.to_string(),
        None => config.settings.active_profile.clone(),
    };

    // Delete the profile
    crate::auth::delete_profile(&profile_to_logout)?;

    println!(
        "Successfully logged out from profile '{}'",
        profile_to_logout
    );
    println!("Credentials removed from system keyring");

    Ok(())
}

pub(crate) async fn run_profile_list() -> Result<()> {
    let config = crate::auth::load_config()?;

    if config.profiles.is_empty() {
        println!("No profiles configured. Use 'tdb login' to create one.");
        return Ok(());
    }

    println!("Available profiles:");
    println!();

    let mut profile_names: Vec<_> = config.profiles.keys().collect();
    profile_names.sort();

    for name in profile_names {
        let profile = &config.profiles[name];
        let is_active = name == &config.settings.active_profile;
        let marker = if is_active { "*" } else { " " };

        println!(
            "{} {} ({}@{} / org: {})",
            marker, name, profile.user, profile.host, profile.org
        );

        if let Some(ref db) = profile.database {
            println!("    default database: {}", db);
        }
        if let Some(ref branch) = profile.branch {
            println!("    default branch: {}", branch);
        }
    }

    println!();
    println!("* = active profile");

    Ok(())
}

pub(crate) async fn run_profile_set(name: &str) -> Result<()> {
    let mut config = crate::auth::load_config()?;

    // Verify profile exists
    if !config.profiles.contains_key(name) {
        anyhow::bail!(
            "Profile '{}' not found. Use 'tdb profile list' to see available profiles.",
            name
        );
    }

    config.set_active(name.to_string());
    crate::auth::save_config(&config)?;

    println!("Set '{}' as active profile", name);

    Ok(())
}

pub(crate) async fn run_profile_show(name: Option<&str>) -> Result<()> {
    let config = crate::auth::load_config()?;

    let profile_name = match name {
        Some(n) => n,
        None => &config.settings.active_profile,
    };

    let profile = config
        .get_profile(profile_name)
        .with_context(|| format!("Profile '{}' not found", profile_name))?;

    println!("Profile: {}", profile_name);
    println!("  Host: {}", profile.host);
    println!("  User: {}", profile.user);
    println!("  Organization: {}", profile.org);

    if let Some(ref db) = profile.database {
        println!("  Default Database: {}", db);
    }
    if let Some(ref branch) = profile.branch {
        println!("  Default Branch: {}", branch);
    }

    println!();
    println!("Password is stored securely in system keyring");

    Ok(())
}

pub(crate) async fn run_profile_delete(name: &str, force: bool) -> Result<()> {
    use std::io::{self, Write};

    let config = crate::auth::load_config()?;

    // Check if profile exists
    if !config.profiles.contains_key(name) {
        anyhow::bail!("Profile '{}' not found", name);
    }

    // Check if it's the active profile
    if name == config.settings.active_profile {
        println!("Warning: '{}' is currently the active profile", name);
    }

    // Confirm deletion unless --force
    if !force {
        print!(
            "Are you sure you want to delete profile '{}'? (y/N): ",
            name
        );
        io::stdout().flush()?;
        let mut response = String::new();
        io::stdin().read_line(&mut response)?;

        if !response.trim().eq_ignore_ascii_case("y") {
            println!("Deletion cancelled");
            return Ok(());
        }
    }

    crate::auth::delete_profile(name)?;

    println!("Profile '{}' deleted", name);

    Ok(())
}

/// Helper function to resolve credentials from multiple sources
/// Priority: CLI args > Environment variables > Active profile > Error
fn resolve_credentials(
    cli_host: Option<String>,
    cli_user: Option<String>,
    cli_password: Option<String>,
    cli_org: Option<String>,
    cli_database: Option<String>,
    cli_branch: Option<String>,
    profile_name: Option<String>,
) -> Result<crate::auth::ResolvedCredentials> {
    // If profile is specified, load from that profile
    if let Some(profile) = profile_name {
        let mut creds = crate::auth::get_profile_credentials(&profile)?;

        // CLI args override profile values
        if let Some(h) = cli_host {
            creds.host = h;
        }
        if let Some(u) = cli_user {
            creds.user = u;
        }
        if let Some(p) = cli_password {
            creds.password = p;
        }
        if let Some(o) = cli_org {
            creds.org = o;
        }
        if let Some(db) = cli_database {
            creds.database = Some(db);
        }
        if let Some(br) = cli_branch {
            creds.branch = Some(br);
        }

        return Ok(creds);
    }

    // Try to load from active profile if no explicit credentials provided
    if cli_host.is_none() || cli_user.is_none() || cli_password.is_none() || cli_org.is_none() {
        if let Ok(mut creds) = crate::auth::get_active_credentials() {
            // CLI args override profile values
            if let Some(h) = cli_host {
                creds.host = h;
            }
            if let Some(u) = cli_user {
                creds.user = u;
            }
            if let Some(p) = cli_password {
                creds.password = p;
            }
            if let Some(o) = cli_org {
                creds.org = o;
            }
            if let Some(db) = cli_database {
                creds.database = Some(db);
            }
            if let Some(br) = cli_branch {
                creds.branch = Some(br);
            }

            return Ok(creds);
        }
    }

    // Fall back to CLI args only (must have all required fields)
    match (cli_host, cli_user, cli_password, cli_org) {
        (Some(host), Some(user), Some(password), Some(org)) => Ok(crate::auth::ResolvedCredentials {
            host,
            user,
            password,
            org,
            database: cli_database,
            branch: cli_branch,
        }),
        _ => {
            anyhow::bail!(
                "Missing required credentials. Please provide --host, --user, --password, and --org, \
                or use 'tdb login' to save credentials in a profile."
            )
        }
    }
}
