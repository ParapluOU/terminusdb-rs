//! Build script for terminusdb-xsd
//!
//! Ensures Python xmlschema module is installed for PyO3 integration.

use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    // First check if xmlschema is already installed
    let check = Command::new("python3")
        .args(["-c", "import xmlschema"])
        .status();

    if let Ok(status) = check {
        if status.success() {
            // Already installed, nothing to do
            return;
        }
    }

    // Try to install xmlschema using pip3
    // First try normal install (works in venvs)
    let result = Command::new("pip3")
        .args(["install", "--quiet", "xmlschema"])
        .status();

    if let Ok(status) = result {
        if status.success() {
            println!("cargo:warning=xmlschema installed successfully");
            return;
        }
    }

    // Try with --user flag
    let result = Command::new("pip3")
        .args(["install", "--user", "--quiet", "xmlschema"])
        .status();

    if let Ok(status) = result {
        if status.success() {
            println!("cargo:warning=xmlschema installed successfully");
            return;
        }
    }

    // For externally-managed environments (Homebrew Python 3.12+), use --break-system-packages
    // This is safe for pure Python packages like xmlschema
    let result = Command::new("pip3")
        .args([
            "install",
            "--quiet",
            "--break-system-packages",
            "xmlschema",
        ])
        .status();

    match result {
        Ok(status) if status.success() => {
            println!("cargo:warning=xmlschema installed successfully");
        }
        _ => {
            println!("cargo:warning=Could not install xmlschema automatically.");
            println!("cargo:warning=Please run: pip3 install xmlschema");
        }
    }
}
