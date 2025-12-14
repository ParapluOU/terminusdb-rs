use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let scripts_dir = PathBuf::from(&manifest_dir).join("scripts");

    println!("cargo:rerun-if-changed=scripts/parse-woql.js");
    println!("cargo:rerun-if-changed=scripts/package.json");

    // Check if Node.js is available
    let node_check = Command::new("node")
        .arg("--version")
        .output();

    if node_check.is_err() {
        eprintln!("WARNING: Node.js not found. The woql-js crate will not work at runtime.");
        eprintln!("         Skipping build script. Install Node.js to enable WOQL JS parsing.");
        return;
    }

    // Run npm install
    println!("cargo:warning=Running npm install in scripts directory...");
    let npm_install = Command::new("npm")
        .arg("install")
        .current_dir(&scripts_dir)
        .status()
        .expect("Failed to run npm install. Is npm installed?");

    if !npm_install.success() {
        panic!("npm install failed");
    }

    // Run npm build to bundle the script
    println!("cargo:warning=Bundling parse-woql.js with esbuild...");
    let npm_build = Command::new("npm")
        .arg("run")
        .arg("build")
        .current_dir(&scripts_dir)
        .status()
        .expect("Failed to run npm build");

    if !npm_build.success() {
        panic!("npm build failed");
    }

    println!("cargo:warning=Successfully bundled parse-woql.js");
}
