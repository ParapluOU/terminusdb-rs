//! TerminusDB binary compilation and distribution
//!
//! This crate compiles TerminusDB from source during the build process and embeds
//! the compiled binary. The binary can be accessed through the `TERMINUSDB_BINARY`
//! constant or executed via the wrapper functions.
//!
//! ## Environment Variables
//!
//! - `TERMINUSDB_VERSION`: Git branch or tag to build (default: "v12.1-rc-paraplu.1")
//! - `TERMINUSDB_FORCE_REBUILD`: Set to "1" to force rebuild even if binary exists
//!
//! ## Example (Low-level API)
//!
//! ```no_run
//! use terminusdb_bin::run_terminusdb;
//! use std::process::Command;
//!
//! // Run TerminusDB with arguments
//! let status = run_terminusdb(&["--version"]).expect("Failed to run TerminusDB");
//! ```
//!
//! ## Example (High-level CLI API)
//!
//! ```no_run
//! use terminusdb_bin::api::{TerminusDB, DbSpec};
//!
//! let client = TerminusDB::new();
//!
//! // Create a database
//! let spec = DbSpec::new("admin", "mydb");
//! client.db().create(spec, Default::default())?;
//! # Ok::<(), std::io::Error>(())
//! ```
//!
//! ## Example (Server Management API)
//!
//! ```no_run
//! use terminusdb_bin::TerminusDBServer;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // Quick test server (in-memory, quiet)
//!     let server = TerminusDBServer::test().await?;
//!     let client = server.client().await?;
//!     println!("Connected to TerminusDB");
//!
//!     // Or use a shared instance across tests
//!     let server = TerminusDBServer::test_instance().await?;
//!     let client = server.client().await?;
//!
//!     Ok(())
//! }
//! ```

// Allow dead code since this is a library with CLI builder patterns where not all methods are always used
#![allow(dead_code)]

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

pub mod api;
pub mod server;

// Re-export server API for convenience
pub use server::{start_server, with_server, ServerOptions, TerminusDBServer};

/// The embedded TerminusDB binary.
/// This is compiled during the build process and embedded into this crate.
pub static TERMINUSDB_BINARY: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/terminusdb"));

/// The embedded librust.dylib (macOS only).
/// On macOS, the dev build needs this library at runtime.
#[cfg(target_os = "macos")]
pub static LIBRUST_DYLIB: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/librust.dylib"));

/// The embedded, relocatable SWI-Prolog runtime home (Linux only).
///
/// The terminusdb binary is a SWI-Prolog saved state and needs its Prolog home
/// (boot file, library predicates, foreign `.so`s) at runtime. We ship it as a
/// `.tar.zst` blob and extract it on first use so the server runs on a machine
/// with no swipl installed.
#[cfg(target_os = "linux")]
pub static SWIPL_HOME_BLOB: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/swipl-home.tar.gz"));

/// Extract the embedded SWI-Prolog home (Linux) into the cache dir and return
/// the root of the extracted tree (which contains `bin/` and `lib/swipl/`).
///
/// The tree is extracted once; a stamp file keyed on the blob length avoids
/// re-extracting on every call and refreshes it when the embedded blob changes.
#[cfg(target_os = "linux")]
pub fn extract_swipl_home() -> std::io::Result<PathBuf> {
    let cache_dir = std::env::temp_dir().join("terminusdb-bin-cache");
    let home_root = cache_dir.join("swipl-home");
    let stamp = cache_dir.join("swipl-home.stamp");

    let expected = SWIPL_HOME_BLOB.len().to_string();
    let up_to_date = fs::read_to_string(&stamp)
        .map(|s| s.trim() == expected)
        .unwrap_or(false)
        && home_root.join("lib").join("swipl").exists();

    if !up_to_date {
        // Fresh extraction: clear any stale tree first.
        let _ = fs::remove_dir_all(&home_root);
        fs::create_dir_all(&home_root)?;
        let decoder = flate2::read::GzDecoder::new(SWIPL_HOME_BLOB);
        let mut archive = tar::Archive::new(decoder);
        archive.set_preserve_permissions(true);
        archive.unpack(&home_root)?;
        fs::write(&stamp, &expected)?;
    }

    Ok(home_root)
}

/// SWI-Prolog per-arch library subdir (PLARCH), e.g. `x86_64-linux`.
#[cfg(target_os = "linux")]
fn plarch() -> String {
    format!("{}-linux", std::env::consts::ARCH)
}

/// Apply the environment the embedded terminusdb binary needs to locate its
/// SWI-Prolog runtime. On Linux this points `SWI_HOME_DIR` at the bundled home
/// and prepends its shared-library dirs to `LD_LIBRARY_PATH`. No-op elsewhere
/// (macOS handles relocation via the extracted librust.dylib).
pub fn apply_runtime_env(cmd: &mut Command) -> std::io::Result<()> {
    #[cfg(target_os = "linux")]
    {
        let home_root = extract_swipl_home()?;
        let swipl_home = home_root.join("lib").join("swipl");
        cmd.env("SWI_HOME_DIR", &swipl_home);

        let mut ld_dirs = vec![home_root.join("lib")];
        let arch_lib = swipl_home.join("lib").join(plarch());
        if arch_lib.exists() {
            ld_dirs.push(arch_lib);
        }
        if let Some(existing) = std::env::var_os("LD_LIBRARY_PATH") {
            ld_dirs.extend(std::env::split_paths(&existing));
        }
        if let Ok(joined) = std::env::join_paths(&ld_dirs) {
            cmd.env("LD_LIBRARY_PATH", joined);
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = cmd;
    }
    Ok(())
}

/// Extracts the embedded TerminusDB binary to a temporary location and returns the path.
///
/// The binary is cached in a temporary directory to avoid repeated extractions.
/// On Unix systems, the extracted binary is made executable.
///
/// ## Returns
///
/// Returns a `PathBuf` pointing to the extracted executable.
///
/// ## Errors
///
/// Returns an error if:
/// - The temporary directory cannot be created
/// - The binary cannot be written to disk
/// - (Unix only) Permissions cannot be set
pub fn extract_binary() -> std::io::Result<PathBuf> {
    let cache_dir = std::env::temp_dir().join("terminusdb-bin-cache");
    fs::create_dir_all(&cache_dir)?;

    let binary_path = cache_dir.join("terminusdb");

    // Only write if it doesn't exist or is outdated
    if !binary_path.exists() || is_outdated(&binary_path)? {
        let mut file = fs::File::create(&binary_path)?;
        file.write_all(TERMINUSDB_BINARY)?;
        file.sync_all()?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&binary_path)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&binary_path, perms)?;
        }
    }

    // On macOS, also extract the dylib needed by the dev build
    // The dev build embeds absolute paths from the build directory in its saved state,
    // so we must extract librust.dylib to the same directory structure used during build
    #[cfg(target_os = "macos")]
    {
        let build_dir = std::env::temp_dir().join("terminusdb-build");
        let src_rust_dir = build_dir.join("src").join("rust");
        fs::create_dir_all(&src_rust_dir)?;

        let dylib_path = src_rust_dir.join("librust.dylib");
        if !dylib_path.exists() {
            let mut file = fs::File::create(&dylib_path)?;
            file.write_all(LIBRUST_DYLIB)?;
            file.sync_all()?;
        }
    }

    Ok(binary_path)
}

/// Checks if the extracted binary is outdated compared to the embedded one.
fn is_outdated(path: &Path) -> std::io::Result<bool> {
    let metadata = fs::metadata(path)?;
    let file_size = metadata.len() as usize;

    // Simple check: if sizes differ, consider it outdated
    Ok(file_size != TERMINUSDB_BINARY.len())
}

/// Runs TerminusDB with the given arguments.
///
/// This function extracts the embedded binary (if needed), then executes it
/// with the provided arguments. Standard input, output, and error are inherited
/// from the parent process.
///
/// ## Arguments
///
/// * `args` - Command-line arguments to pass to TerminusDB
///
/// ## Returns
///
/// Returns the exit status of the TerminusDB process.
///
/// ## Errors
///
/// Returns an error if:
/// - The binary cannot be extracted
/// - The process cannot be spawned
///
/// ## Example
///
/// ```no_run
/// use terminusdb_bin::run_terminusdb;
///
/// let status = run_terminusdb(&["serve"]).expect("Failed to start TerminusDB");
/// println!("TerminusDB exited with: {}", status);
/// ```
pub fn run_terminusdb(args: &[&str]) -> std::io::Result<ExitStatus> {
    let binary_path = extract_binary()?;

    let mut cmd = Command::new(binary_path);
    cmd.args(args);
    apply_runtime_env(&mut cmd)?;
    cmd.status()
}

/// Runs TerminusDB with the given arguments and inherits all I/O streams.
///
/// This is similar to `run_terminusdb` but provides more explicit control.
pub fn exec_terminusdb(args: &[&str]) -> std::io::Result<ExitStatus> {
    run_terminusdb(args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_binary_embedded() {
        assert!(!TERMINUSDB_BINARY.is_empty(), "Binary should be embedded");
        assert!(
            TERMINUSDB_BINARY.len() > 1000,
            "Binary should be reasonably sized"
        );
    }

    #[test]
    fn test_extract_binary() {
        let path = extract_binary().expect("Should extract binary successfully");
        assert!(path.exists(), "Extracted binary should exist");
        assert!(path.is_file(), "Extracted binary should be a file");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let metadata = fs::metadata(&path).unwrap();
            let mode = metadata.permissions().mode();
            assert_eq!(mode & 0o111, 0o111, "Binary should be executable");
        }
    }
}
