use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=TERMINUSDB_VERSION");
    println!("cargo:rerun-if-env-changed=TERMINUSDB_FORCE_REBUILD");
    println!("cargo:rerun-if-env-changed=TERMINUSDB_SOURCE");

    let out_dir = env::var("OUT_DIR").unwrap();
    let binary_path = Path::new(&out_dir).join("terminusdb");
    let force_rebuild = env::var("TERMINUSDB_FORCE_REBUILD").unwrap_or_default() == "1";

    // Check if binary already exists and skip build if not forced
    if binary_path.exists() && !force_rebuild {
        println!(
            "cargo:warning=TerminusDB binary already exists at {}",
            binary_path.display()
        );
        println!("cargo:warning=Skipping build. Set TERMINUSDB_FORCE_REBUILD=1 to force rebuild.");
        return;
    }

    // Check for cross-compilation - if target differs from host, try to use pre-built binaries
    let target = env::var("TARGET").unwrap_or_default();
    let host = env::var("HOST").unwrap_or_default();
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();

    if target != host && !target.is_empty() && !host.is_empty() {
        println!("cargo:warning=Cross-compilation detected: {} -> {}", host, target);

        // Try to use pre-built binary
        let prebuilt_dir = PathBuf::from(&manifest_dir).join("prebuilt").join(&target);
        let prebuilt_binary = prebuilt_dir.join("terminusdb");

        if prebuilt_binary.exists() {
            println!(
                "cargo:warning=Using pre-built TerminusDB binary from {}",
                prebuilt_binary.display()
            );

            // Copy the binary to OUT_DIR
            if let Err(e) = fs::copy(&prebuilt_binary, &binary_path) {
                panic!(
                    "Failed to copy pre-built binary from {} to {}: {}",
                    prebuilt_binary.display(),
                    binary_path.display(),
                    e
                );
            }

            // Set executable permissions
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = fs::metadata(&binary_path).unwrap().permissions();
                perms.set_mode(0o755);
                fs::set_permissions(&binary_path, perms).unwrap();
            }

            // Copy librust.dylib for macOS targets
            if target.contains("apple-darwin") {
                let prebuilt_dylib = prebuilt_dir.join("librust.dylib");
                if prebuilt_dylib.exists() {
                    let dylib_dest = Path::new(&out_dir).join("librust.dylib");
                    println!("cargo:warning=Copying pre-built librust.dylib...");
                    if let Err(e) = fs::copy(&prebuilt_dylib, &dylib_dest) {
                        panic!("Failed to copy pre-built dylib: {}", e);
                    }
                } else {
                    println!("cargo:warning=Note: librust.dylib not found in pre-built directory");
                }
            }

            println!(
                "cargo:warning=Successfully installed pre-built TerminusDB binary at {}",
                binary_path.display()
            );
            return;
        } else {
            panic!(
                "Cross-compilation requires pre-built binaries.\n\
                 Expected pre-built binary at: {}\n\
                 \n\
                 To create pre-built binaries:\n\
                 1. Build TerminusDB on the target platform ({})\n\
                 2. Copy the 'terminusdb' binary to {}/\n\
                 3. For macOS, also copy 'src/rust/librust.dylib'\n",
                prebuilt_binary.display(),
                target,
                prebuilt_dir.display()
            );
        }
    }

    println!("cargo:warning=Building TerminusDB from source...");

    // Detect platform
    let platform = detect_platform();
    println!("cargo:warning=Detected platform: {:?}", platform);

    // Get crate directory for local installations
    let deps_dir = PathBuf::from(&manifest_dir).join(".deps");
    fs::create_dir_all(&deps_dir).expect("Failed to create .deps directory");

    // Ensure dependencies are available
    let dep_context = match ensure_dependencies(platform, &deps_dir) {
        Ok(ctx) => ctx,
        Err(e) => panic!("Failed to ensure dependencies: {}", e),
    };

    // Check if TERMINUSDB_SOURCE is set to use a local checkout
    let (build_dir, should_cleanup) = if let Ok(source_path) = env::var("TERMINUSDB_SOURCE") {
        let source_dir = PathBuf::from(&source_path);
        if !source_dir.exists() {
            panic!("TERMINUSDB_SOURCE path does not exist: {}", source_path);
        }
        if !source_dir.join("Makefile").exists() {
            panic!(
                "TERMINUSDB_SOURCE does not appear to be a TerminusDB checkout (no Makefile): {}",
                source_path
            );
        }
        println!("cargo:warning=Building from local source: {}", source_path);
        (source_dir, false)
    } else {
        // Get version to build
        // Default to main branch (the fix is now merged into mainline)
        let version = env::var("TERMINUSDB_VERSION").unwrap_or_else(|_| "main".to_string());
        println!("cargo:warning=Building TerminusDB version: {}", version);

        // Clone TerminusDB to a consistent temporary directory
        // Note: The dev build embeds absolute paths to this directory in the saved state,
        // so we use a fixed path that the runtime can recreate
        let temp_dir = env::temp_dir().join("terminusdb-build");
        println!(
            "cargo:warning=Cloning TerminusDB to: {}",
            temp_dir.display()
        );

        if let Err(e) = clone_terminusdb(&version, &temp_dir) {
            panic!("Failed to clone TerminusDB: {}", e);
        }

        (temp_dir, true)
    };

    // Build TerminusDB with dependency context
    if let Err(e) = build_terminusdb(&build_dir, &dep_context) {
        panic!("Failed to build TerminusDB: {}", e);
    }

    // Copy the binary to OUT_DIR
    let built_binary = build_dir.join("terminusdb");
    if let Err(e) = fs::copy(&built_binary, &binary_path) {
        panic!(
            "Failed to copy binary from {} to {}: {}",
            built_binary.display(),
            binary_path.display(),
            e
        );
    }

    // Set executable permissions on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&binary_path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&binary_path, perms).unwrap();
    }

    // On macOS, the dev build needs librust.dylib at runtime
    #[cfg(target_os = "macos")]
    {
        let dylib_src = build_dir.join("src/rust/librust.dylib");
        if dylib_src.exists() {
            let dylib_dest = Path::new(&out_dir).join("librust.dylib");
            println!("cargo:warning=Copying librust.dylib for macOS dev build...");
            if let Err(e) = fs::copy(&dylib_src, &dylib_dest) {
                panic!("Failed to copy dylib: {}", e);
            }
        } else {
            panic!("librust.dylib not found at {}", dylib_src.display());
        }
    }

    // Clean up temp directory only if we cloned it
    if should_cleanup {
        let _ = fs::remove_dir_all(&build_dir);
    }

    println!(
        "cargo:warning=Successfully built TerminusDB binary at {}",
        binary_path.display()
    );
}

#[derive(Debug, Clone, Copy)]
enum Platform {
    MacOS,
    Linux,
}

fn detect_platform() -> Platform {
    match env::consts::OS {
        "macos" => Platform::MacOS,
        "linux" => Platform::Linux,
        other => panic!(
            "Unsupported platform: {}. Only macOS and Linux are supported.",
            other
        ),
    }
}

fn check_tool(name: &str) -> bool {
    which::which(name).is_ok()
}

struct DependencyContext {
    protoc_path: Option<PathBuf>,
    swipl_path: Option<PathBuf>,
    path_additions: Vec<PathBuf>,
}

fn ensure_dependencies(platform: Platform, deps_dir: &Path) -> Result<DependencyContext, String> {
    println!("cargo:warning=Verifying and bundling dependencies...");

    // Essential tools that cannot be bundled - must be present
    let essential_tools = vec!["git", "make"];
    let mut missing_essential = Vec::new();

    for tool in &essential_tools {
        if !check_tool(tool) {
            missing_essential.push(*tool);
        }
    }

    if !missing_essential.is_empty() {
        return Err(format!(
            "Missing essential system tools: {}. Please install them manually:\n\
             - macOS: brew install git make\n\
             - Linux: sudo apt-get install git make",
            missing_essential.join(", ")
        ));
    }

    let mut ctx = DependencyContext {
        protoc_path: None,
        swipl_path: None,
        path_additions: Vec::new(),
    };

    // Handle protoc - try to bundle it
    if check_tool("protoc") {
        println!("cargo:warning=Found system protoc");
        ctx.protoc_path = which::which("protoc").ok();
    } else {
        println!("cargo:warning=protoc not found, attempting to download and bundle...");
        match download_protoc(platform, deps_dir) {
            Ok(path) => {
                println!(
                    "cargo:warning=Successfully bundled protoc at {}",
                    path.display()
                );
                ctx.protoc_path = Some(path.clone());
                if let Some(parent) = path.parent() {
                    ctx.path_additions.push(parent.to_path_buf());
                }
            }
            Err(e) => {
                return Err(format!(
                    "Failed to bundle protoc: {}.\n\
                     Please install manually:\n\
                     - macOS: brew install protobuf\n\
                     - Linux: sudo apt-get install protobuf-compiler",
                    e
                ));
            }
        }
    }

    // Handle SWI-Prolog - check if present, try to install locally if not
    if check_tool("swipl") {
        println!("cargo:warning=Found system SWI-Prolog");
        ctx.swipl_path = which::which("swipl").ok();
    } else {
        println!("cargo:warning=SWI-Prolog not found, attempting to install locally...");
        match install_swipl_local(platform, deps_dir) {
            Ok(path) => {
                println!(
                    "cargo:warning=Successfully installed SWI-Prolog at {}",
                    path.display()
                );
                ctx.swipl_path = Some(path.clone());
                if let Some(parent) = path.parent() {
                    ctx.path_additions.push(parent.to_path_buf());
                }
            }
            Err(e) => {
                return Err(format!(
                    "Failed to install SWI-Prolog locally: {}.\n\
                     Please install manually:\n\
                     - macOS: brew install swi-prolog\n\
                     - Linux: sudo apt-add-repository ppa:swi-prolog/stable && sudo apt-get update && sudo apt-get install swi-prolog",
                    e
                ));
            }
        }
    }

    // Handle GMP library - verify it exists
    match platform {
        Platform::MacOS => {
            if !check_gmp_macos() {
                return Err("GMP library not found. Please install:\n\
                     - macOS: brew install gmp"
                    .to_string());
            }
            println!("cargo:warning=Found GMP library");
        }
        Platform::Linux => {
            if !check_gmp_linux() {
                return Err("GMP development library not found. Please install:\n\
                     - Linux: sudo apt-get install libgmp-dev"
                    .to_string());
            }
            println!("cargo:warning=Found GMP library");
        }
    }

    println!("cargo:warning=All dependencies verified and ready");
    Ok(ctx)
}

fn check_gmp_macos() -> bool {
    // Check if gmp is installed via homebrew
    Command::new("brew")
        .args(["list", "gmp"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
        || Path::new("/opt/homebrew/opt/gmp").exists()
        || Path::new("/usr/local/opt/gmp").exists()
}

fn check_gmp_linux() -> bool {
    // Check if libgmp-dev is installed
    Path::new("/usr/include/gmp.h").exists()
        || Path::new("/usr/local/include/gmp.h").exists()
        || Path::new("/usr/include/x86_64-linux-gnu/gmp.h").exists()
}

fn download_protoc(platform: Platform, deps_dir: &Path) -> Result<PathBuf, String> {
    let protoc_dir = deps_dir.join("protoc");
    let bin_dir = protoc_dir.join("bin");
    let protoc_path = bin_dir.join("protoc");

    // Check if already downloaded
    if protoc_path.exists() {
        println!("cargo:warning=protoc already bundled");
        return Ok(protoc_path);
    }

    fs::create_dir_all(&protoc_dir).map_err(|e| format!("Failed to create protoc dir: {}", e))?;

    let (url, archive_name) = match platform {
        Platform::MacOS => {
            let arch = env::consts::ARCH;
            let arch_suffix = match arch {
                "aarch64" => "aarch_64",
                "x86_64" => "x86_64",
                _ => return Err(format!("Unsupported macOS architecture: {}", arch)),
            };
            (
                format!("https://github.com/protocolbuffers/protobuf/releases/download/v25.1/protoc-25.1-osx-{}.zip", arch_suffix),
                "protoc.zip"
            )
        }
        Platform::Linux => {
            let arch = env::consts::ARCH;
            let arch_suffix = match arch {
                "aarch64" => "aarch_64",
                "x86_64" => "x86_64",
                _ => return Err(format!("Unsupported Linux architecture: {}", arch)),
            };
            (
                format!("https://github.com/protocolbuffers/protobuf/releases/download/v25.1/protoc-25.1-linux-{}.zip", arch_suffix),
                "protoc.zip"
            )
        }
    };

    let archive_path = protoc_dir.join(archive_name);

    println!("cargo:warning=Downloading protoc from {}", url);

    // Download using curl
    let download_status = Command::new("curl")
        .args(["-L", "-o", archive_path.to_str().unwrap(), &url])
        .status()
        .map_err(|e| format!("Failed to run curl: {}", e))?;

    if !download_status.success() {
        return Err("Failed to download protoc".to_string());
    }

    // Extract using unzip
    let extract_status = Command::new("unzip")
        .args([
            "-q",
            "-o",
            archive_path.to_str().unwrap(),
            "-d",
            protoc_dir.to_str().unwrap(),
        ])
        .status()
        .map_err(|e| format!("Failed to run unzip: {}", e))?;

    if !extract_status.success() {
        return Err("Failed to extract protoc".to_string());
    }

    // Set executable permission
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&protoc_path)
            .map_err(|e| format!("Failed to get protoc permissions: {}", e))?
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&protoc_path, perms)
            .map_err(|e| format!("Failed to set protoc permissions: {}", e))?;
    }

    // Clean up archive
    let _ = fs::remove_file(&archive_path);

    Ok(protoc_path)
}

fn install_swipl_local(platform: Platform, deps_dir: &Path) -> Result<PathBuf, String> {
    let swipl_dir = deps_dir.join("swi-prolog");
    let bin_dir = swipl_dir.join("bin");
    let swipl_path = bin_dir.join("swipl");

    // Check if already installed
    if swipl_path.exists() {
        println!("cargo:warning=SWI-Prolog already installed locally");
        return Ok(swipl_path);
    }

    fs::create_dir_all(&swipl_dir).map_err(|e| format!("Failed to create swipl dir: {}", e))?;

    match platform {
        Platform::MacOS => {
            // For macOS, we can try to download a pre-built version or use homebrew
            // This is complex, so for now just return an error with instructions
            Err(
                "SWI-Prolog automatic installation not yet supported on macOS.\n\
                 Please install manually: brew install swi-prolog"
                    .to_string(),
            )
        }
        Platform::Linux => {
            // For Linux, we could download and install from PPA or compile from source
            // For now, return an error with instructions
            Err(
                "SWI-Prolog automatic installation not yet supported on Linux.\n\
                 Please install manually: sudo apt-add-repository ppa:swi-prolog/stable && \
                 sudo apt-get update && sudo apt-get install swi-prolog"
                    .to_string(),
            )
        }
    }
}

fn clone_terminusdb(version: &str, dest: &Path) -> Result<(), String> {
    // If destination already exists, try to update it instead of cloning fresh
    if dest.exists() {
        let git_dir = dest.join(".git");
        if git_dir.exists() {
            println!("cargo:warning=TerminusDB directory already exists, updating...");

            // Fetch the latest
            let fetch_status = Command::new("git")
                .args(["fetch", "--depth=1", "origin", version])
                .current_dir(dest)
                .status()
                .map_err(|e| format!("Failed to run git fetch: {}", e))?;

            if fetch_status.success() {
                // Checkout the version
                let checkout_status = Command::new("git")
                    .args(["checkout", "FETCH_HEAD"])
                    .current_dir(dest)
                    .status()
                    .map_err(|e| format!("Failed to run git checkout: {}", e))?;

                if checkout_status.success() {
                    println!("cargo:warning=Successfully updated TerminusDB");
                    return Ok(());
                }
            }

            // If fetch/checkout failed, try to remove and re-clone
            println!("cargo:warning=Git update failed, attempting fresh clone...");
        }

        // Remove existing directory
        if let Err(e) = fs::remove_dir_all(dest) {
            return Err(format!(
                "Failed to remove existing directory at {}: {}. \
                 Please manually delete it and retry.",
                dest.display(),
                e
            ));
        }
    }

    println!("cargo:warning=Cloning TerminusDB repository...");

    let status = Command::new("git")
        .args([
            "clone",
            "--depth=1",
            "--branch",
            version,
            "https://github.com/ParapluOU/terminusdb.git",
            dest.to_str().unwrap(),
        ])
        .status()
        .map_err(|e| format!("Failed to run git: {}", e))?;

    if !status.success() {
        return Err(format!("git clone failed with status: {}", status));
    }

    println!("cargo:warning=Successfully cloned TerminusDB");
    Ok(())
}

fn build_terminusdb(repo_path: &Path, ctx: &DependencyContext) -> Result<(), String> {
    println!("cargo:warning=Installing SWI-Prolog pack dependencies...");

    // Build PATH with bundled dependencies
    let mut path_dirs: Vec<PathBuf> = ctx.path_additions.clone();

    // Add existing PATH
    if let Ok(existing_path) = env::var("PATH") {
        for dir in env::split_paths(&existing_path) {
            path_dirs.push(dir);
        }
    }

    let new_path =
        env::join_paths(&path_dirs).map_err(|e| format!("Failed to build PATH: {}", e))?;

    // Install tus pack
    // Clear cargo/clippy flags to prevent clippy from propagating to external TerminusDB build
    // RUSTC_WORKSPACE_WRAPPER is how cargo clippy injects clippy-driver
    let status = Command::new("make")
        .args(["install-deps"])
        .current_dir(repo_path)
        .env("PATH", &new_path)
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env_remove("RUSTC_WORKSPACE_WRAPPER")
        .env_remove("RUSTC_WRAPPER")
        .status()
        .map_err(|e| format!("Failed to run make install-deps: {}", e))?;

    if !status.success() {
        return Err(format!("make install-deps failed with status: {}", status));
    }

    println!("cargo:warning=Building TerminusDB binary...");

    // On macOS, use 'make PROFILE=release dev' target to create a development binary
    // that doesn't strip libraries (preserves code signatures, requires librust.dylib at runtime)
    // On Linux, use default target with 'make PROFILE=release' for standalone binary
    // Both explicitly set PROFILE=release to avoid inheriting PROFILE=debug from cargo check/build
    #[cfg(target_os = "macos")]
    let make_args = &["PROFILE=release", "dev"];
    #[cfg(not(target_os = "macos"))]
    let make_args = &["PROFILE=release"];

    // Clear cargo/clippy flags to prevent clippy from propagating to external TerminusDB build
    // RUSTC_WORKSPACE_WRAPPER is how cargo clippy injects clippy-driver
    let status = Command::new("make")
        .args(make_args)
        .current_dir(repo_path)
        .env("PATH", &new_path)
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env_remove("RUSTC_WORKSPACE_WRAPPER")
        .env_remove("RUSTC_WRAPPER")
        .status()
        .map_err(|e| format!("Failed to run make: {}", e))?;

    if !status.success() {
        return Err(format!("make failed with status: {}", status));
    }

    println!("cargo:warning=TerminusDB build completed successfully");
    Ok(())
}
