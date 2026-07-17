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

    // On Linux the embed also needs the packed swipl home; only treat the cache
    // as complete when both artifacts are present.
    #[cfg(target_os = "linux")]
    let cache_complete = binary_path.exists()
        && Path::new(&out_dir).join("swipl-home.tar.gz").exists();
    #[cfg(not(target_os = "linux"))]
    let cache_complete = binary_path.exists();

    // Check if binary already exists and skip build if not forced
    if cache_complete && !force_rebuild {
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

    // Resolve TerminusDB source directory.
    // Priority: TERMINUSDB_SOURCE env > sibling terminusdb submodule > git clone.
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
        println!("cargo:warning=Building from TERMINUSDB_SOURCE: {}", source_path);
        (source_dir, false)
    } else {
        // Check for sibling terminusdb submodule (e.g., modules/terminusdb alongside modules/terminusdb-rs)
        let workspace_root = PathBuf::from(&manifest_dir).join("../..");
        let submodule_candidates = [
            workspace_root.join("../terminusdb"),          // sibling to terminusdb-rs workspace
        ];
        let submodule = submodule_candidates.iter().find_map(|p| {
            p.canonicalize().ok().filter(|c| c.join("Makefile").exists())
        });

        if let Some(dir) = submodule {
            println!(
                "cargo:warning=Building from local submodule: {}",
                dir.display()
            );
            (dir, false)
        } else {
            // Fall back to cloning from GitHub.
            // Pinned to the ParapluOU/terminusdb fork tag rebased onto upstream
            // TerminusDB 12.1 (upstream branch 12.1-rc; no v12.1 release tag yet).
            // Override with TERMINUSDB_VERSION to track a moving branch or bump.
            let version =
                env::var("TERMINUSDB_VERSION").unwrap_or_else(|_| "v12.1-rc-paraplu.1".to_string());
            println!("cargo:warning=Building TerminusDB version: {}", version);

            let temp_dir = env::temp_dir().join("terminusdb-build");
            println!(
                "cargo:warning=Cloning TerminusDB to: {}",
                temp_dir.display()
            );

            if let Err(e) = clone_terminusdb(&version, &temp_dir) {
                panic!("Failed to clone TerminusDB: {}", e);
            }

            (temp_dir, true)
        }
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

    // On Linux, bundle the relocatable SWI-Prolog home next to the binary so the
    // embedded server runs on a machine with no swipl installed. lib.rs embeds
    // this blob and extracts it at runtime (see extract_swipl_home).
    #[cfg(target_os = "linux")]
    {
        let swipl_root = dep_context
            .swipl_root
            .as_ref()
            .expect("Linux build must provision a relocatable swipl root");
        let home_blob = Path::new(&out_dir).join("swipl-home.tar.gz");
        println!(
            "cargo:warning=Packing SWI-Prolog runtime home from {} ...",
            swipl_root.display()
        );
        if let Err(e) = pack_swipl_home(swipl_root, &home_blob) {
            panic!("Failed to pack SWI-Prolog home: {}", e);
        }
        println!(
            "cargo:warning=Bundled SWI-Prolog home at {}",
            home_blob.display()
        );
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

/// SWI-Prolog's per-architecture library subdir name (PLARCH), e.g.
/// `x86_64-linux`. Used to locate `lib/swipl/lib/<PLARCH>`.
fn plarch() -> String {
    let arch = match env::var("CARGO_CFG_TARGET_ARCH").ok().as_deref() {
        Some(a) => a.to_string(),
        None => env::consts::ARCH.to_string(),
    };
    format!("{}-linux", arch)
}

struct DependencyContext {
    protoc_path: Option<PathBuf>,
    swipl_path: Option<PathBuf>,
    path_additions: Vec<PathBuf>,
    /// Root of a relocatable SWI-Prolog install (contains `bin/`, `lib/swipl/`).
    /// On Linux this is always populated: we provision our own swipl so that
    /// (a) the terminusdb saved-state is built against a known swipl version and
    /// (b) the exact same runtime home can be bundled into the embedded binary.
    swipl_root: Option<PathBuf>,
    /// Root of a provisioned GMP (contains `include/`, `lib/`) used to make the
    /// TerminusDB Rust build resilient on hosts without libgmp-dev.
    gmp_root: Option<PathBuf>,
    /// Root of a provisioned libclang conda env (contains `lib/libclang.so` and a
    /// co-located `libLLVM.so`) used by `bindgen` (swipl-fli) so the build does not
    /// depend on a system libclang. Build-time only — never bundled.
    libclang_root: Option<PathBuf>,
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
        swipl_root: None,
        gmp_root: None,
        libclang_root: None,
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

    // Handle SWI-Prolog.
    //
    // On Linux we ALWAYS provision our own relocatable SWI-Prolog rather than
    // trusting whatever is on PATH. This is deliberate: the terminusdb binary is
    // a SWI-Prolog *saved state* whose format is locked to the exact swipl
    // version that produced it. To ship a self-contained embed we must bundle a
    // Prolog home that matches the build swipl byte-for-byte, so the safest path
    // is to build *and* bundle from the same provisioned install. A system swipl
    // (e.g. distro/nix) is also frequently non-relocatable (RPATHs into
    // /nix/store or /usr), which would break the bundled runtime anyway.
    match platform {
        Platform::Linux => {
            let target = env::var("TARGET").unwrap_or_default();
            match provide_relocatable_swipl(&target, deps_dir) {
                Ok(root) => {
                    let swipl_bin = root.join("bin").join("swipl");
                    println!(
                        "cargo:warning=Using relocatable SWI-Prolog at {}",
                        swipl_bin.display()
                    );
                    ctx.swipl_path = Some(swipl_bin.clone());
                    ctx.swipl_root = Some(root.clone());
                    // Put our swipl first on PATH so `make` uses it, not a
                    // system one that might have a different saved-state version.
                    ctx.path_additions.insert(0, root.join("bin"));
                }
                Err(e) => {
                    return Err(format!(
                        "Failed to provision a relocatable SWI-Prolog: {}",
                        e
                    ));
                }
            }
        }
        Platform::MacOS => {
            // macOS keeps using a system swipl (the dev build + librust.dylib
            // mechanism handles relocation there). Self-contained macOS bundling
            // is tracked separately and out of scope for now.
            if check_tool("swipl") {
                println!("cargo:warning=Found system SWI-Prolog");
                ctx.swipl_path = which::which("swipl").ok();
            } else {
                return Err(
                    "SWI-Prolog not found. Please install: brew install swi-prolog".to_string(),
                );
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
            let target = env::var("TARGET").unwrap_or_default();

            // m4 is required by `gmp-mpfr-sys` (it builds GMP from source during
            // the TerminusDB Rust build). Provision it if the host lacks it.
            if check_tool("m4") {
                println!("cargo:warning=Found system m4");
            } else if let Some(root) = provide_conda_tool(&M4_TOOL, &target, deps_dir) {
                ctx.path_additions.insert(0, root.join("bin"));
            } else {
                println!(
                    "cargo:warning=m4 not found and no prebuilt available; the TerminusDB Rust \
                     build may fail (install m4)."
                );
            }

            // GMP: prefer system dev headers; otherwise provision a prebuilt so
            // C builds that link libgmp can find it. (Our swipl already bundles
            // the GMP *runtime* it needs; this covers build-time headers/libs.)
            if check_gmp_linux() {
                println!("cargo:warning=Found system GMP library");
            } else if let Some(root) = provide_conda_tool(&GMP_TOOL, &target, deps_dir) {
                ctx.gmp_root = Some(root);
            } else {
                println!(
                    "cargo:warning=System GMP not found and no prebuilt available; relying on the \
                     bundled SWI-Prolog runtime (install libgmp-dev if a source build needs it)."
                );
            }

            // libclang: bindgen (swipl-fli, in the TerminusDB Rust build) needs a
            // libclang.so with a co-located libLLVM. Prefer an explicit
            // LIBCLANG_PATH; otherwise provision a conda-forge libclang env so the
            // build is self-contained (many split system installs put libclang and
            // libLLVM in different dirs, which breaks bindgen's dlopen).
            if env::var_os("LIBCLANG_PATH").is_some() {
                println!("cargo:warning=Using libclang from LIBCLANG_PATH");
            } else if let Some(root) = provide_libclang(&target, deps_dir) {
                println!("cargo:warning=Provisioned libclang at {}", root.display());
                ctx.libclang_root = Some(root);
            } else {
                println!(
                    "cargo:warning=Could not provision libclang; bindgen (swipl-fli) may fail \
                     (install libclang / set LIBCLANG_PATH)."
                );
            }
        }
    }

    println!("cargo:warning=All dependencies verified and ready");
    Ok(ctx)
}

/// Provision a build-time-only `libclang` via micromamba/conda-forge (returns the
/// env root whose `lib/` holds `libclang.so` + a co-located `libLLVM.so`). Cached
/// under `.deps/libclang-env/<target>`. Returns `None` (not fatal) on any failure;
/// the caller warns and lets the build try a system libclang.
fn provide_libclang(target: &str, deps_dir: &Path) -> Option<PathBuf> {
    let env_root = deps_dir.join("libclang-env").join(target);
    // A usable env needs BOTH libclang.so AND clang's builtin resource headers
    // (lib/clang/<v>/include/stddef.h) — the bare `libclang` package omits the
    // latter, which bindgen requires (`'stddef.h' file not found`).
    let has_libclang = |root: &Path| -> bool {
        let lib = root.join("lib");
        let so = fs::read_dir(&lib)
            .map(|rd| {
                rd.flatten().any(|e| {
                    e.file_name()
                        .to_str()
                        .map(|n| n.starts_with("libclang.so"))
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false);
        let headers = fs::read_dir(lib.join("clang"))
            .map(|rd| rd.flatten().any(|e| e.path().join("include").join("stddef.h").exists()))
            .unwrap_or(false);
        so && headers
    };
    // Cache hit.
    if has_libclang(&env_root) {
        println!("cargo:warning=Reusing cached libclang env at {}", env_root.display());
        return Some(env_root);
    }
    // Only conda-forge linux-64/aarch64 are wired here (same as swipl).
    if !(target.contains("linux") && (target.starts_with("x86_64") || target.starts_with("aarch64")))
    {
        return None;
    }
    let mm = ensure_micromamba(deps_dir).ok()?;
    let root_prefix = deps_dir.join("mamba-root");
    let _ = fs::create_dir_all(&root_prefix);
    let _ = fs::remove_dir_all(&env_root);
    println!("cargo:warning=Solving libclang environment with micromamba (downloads libclang + libLLVM)...");
    let status = Command::new(&mm)
        .args([
            "create",
            "-y",
            "-p",
            &env_root.display().to_string(),
            "-c",
            "conda-forge",
            // `clang` pulls libclang.so + libLLVM + the clang resource headers
            // (lib/clang/<v>/include/stddef.h) bindgen needs; `libclang` alone
            // ships only the shared libs.
            "clang",
            "libclang",
        ])
        .env("MAMBA_ROOT_PREFIX", &root_prefix)
        .env("CONDA_PKGS_DIRS", root_prefix.join("pkgs"))
        .env_remove("CONDARC")
        .status()
        .ok()?;
    if status.success() && has_libclang(&env_root) {
        Some(env_root)
    } else {
        println!("cargo:warning=micromamba libclang env creation failed (status {status})");
        None
    }
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

    // Extract the zip in-process (no external `unzip` binary required).
    extract_zip(&archive_path, &protoc_dir)?;

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

/// Extract a zip archive into `dest`, preserving unix permissions. Pure-Rust
/// (via the `zip` crate) so no external `unzip` is needed on the build machine.
fn extract_zip(archive: &Path, dest: &Path) -> Result<(), String> {
    let file = fs::File::open(archive).map_err(|e| format!("open {}: {}", archive.display(), e))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| format!("read zip: {}", e))?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
        let out_path = match entry.enclosed_name() {
            Some(p) => dest.join(p),
            None => continue,
        };
        if entry.is_dir() {
            fs::create_dir_all(&out_path).map_err(|e| e.to_string())?;
            continue;
        }
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut out = fs::File::create(&out_path)
            .map_err(|e| format!("create {}: {}", out_path.display(), e))?;
        std::io::copy(&mut entry, &mut out).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Some(mode) = entry.unix_mode() {
                let _ = fs::set_permissions(&out_path, fs::Permissions::from_mode(mode));
            }
        }
    }
    Ok(())
}

/// SWI-Prolog version we standardise on for Linux. The saved-state format is
/// version-locked, so the build swipl and the bundled runtime must match exactly
/// — which is why we build *and* bundle from the same provisioned env.
const SWIPL_VERSION: &str = "10.0.0";

/// Extra conda packages to co-install with swi-prolog so the tree is genuinely
/// self-contained. `libxcrypt` is critical: the conda swi-prolog's `crypt.so`
/// needs `libcrypt.so.2`, but the package omits that dependency (and Debian ships
/// `libcrypt.so.1`, not `.2`), so without this the `qsave` bootstrap — and hence
/// the whole build — fails when it autoloads `library(crypt)`.
const SWIPL_ENV_PACKAGES: &[&str] = &["libxcrypt"];

/// Provide a *relocatable*, self-contained SWI-Prolog install for Linux and
/// return its root directory (a conda-style prefix with `bin/`, `lib/swipl/`,
/// and every shared library its foreign extensions need in `lib/`).
///
/// We use micromamba to solve and materialise the *full* dependency closure into
/// one relocatable prefix. This is the only reliable way to get a swipl whose
/// foreign extensions (archive, crypt, ssl, ...) all load on a bare box — the
/// swi-prolog conda package alone pulls dozens of sibling libraries
/// (libarchive → zstd/lz4/xml2/krb5/..., libxcrypt, ...) that are impractical to
/// hand-resolve. Order:
///   1. Cached prefix under `.deps/swipl-env/<target>` (build cache).
///   2. `micromamba create` the closure (needs network — cargo does too).
///   3. Compile SWI-Prolog from source (arches micromamba/conda-forge lack, e.g.
///      currently non-x86_64 Linux); post-processed by `bundle_aux_libs`.
fn provide_relocatable_swipl(target: &str, deps_dir: &Path) -> Result<PathBuf, String> {
    let env_root = deps_dir.join("swipl-env").join(target);
    let swipl_bin = env_root.join("bin").join("swipl");

    // (1) Cache hit.
    if swipl_bin.exists() {
        println!("cargo:warning=Reusing cached SWI-Prolog env at {}", env_root.display());
        return Ok(env_root);
    }

    // (2) micromamba full-closure env (conda-forge publishes swi-prolog only for
    // linux-64 today; other arches fall through to the source build).
    if target.starts_with("x86_64") && target.contains("linux") {
        match create_swipl_env(target, deps_dir, &env_root) {
            Ok(()) => {
                trim_swipl_env(&env_root);
                finalize_swipl_install(&env_root, false)?;
                return Ok(env_root);
            }
            Err(e) => {
                println!("cargo:warning=micromamba env creation failed ({e}); falling back to source build");
            }
        }
    } else {
        println!("cargo:warning=No conda swi-prolog for target {target}; building from source");
    }

    // (3) Build from source (self-contained via bundle_aux_libs).
    let src_root = deps_dir.join("swi-prolog").join(target);
    build_swipl_from_source(&src_root, deps_dir)?;
    finalize_swipl_install(&src_root, true)?;
    Ok(src_root)
}

/// Ensure a `micromamba` binary exists under `.deps/micromamba` and return it.
/// Downloads the static, dependency-free release binary if missing.
fn ensure_micromamba(deps_dir: &Path) -> Result<PathBuf, String> {
    let mm_dir = deps_dir.join("micromamba");
    let mm = mm_dir.join("micromamba");
    if mm.exists() {
        return Ok(mm);
    }
    fs::create_dir_all(&mm_dir).map_err(|e| e.to_string())?;
    // Raw static binary (no archive) from the official releases.
    let arch = match env::var("CARGO_CFG_TARGET_ARCH").ok().as_deref() {
        Some("aarch64") => "linux-aarch64",
        _ => "linux-64",
    };
    let url = format!(
        "https://github.com/mamba-org/micromamba-releases/releases/latest/download/micromamba-{arch}"
    );
    download_file(&url, &mm)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&mm).map_err(|e| e.to_string())?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&mm, perms).map_err(|e| e.to_string())?;
    }
    Ok(mm)
}

/// Create a swi-prolog conda env (full dependency closure) at `env_root`.
fn create_swipl_env(_target: &str, deps_dir: &Path, env_root: &Path) -> Result<(), String> {
    let mm = ensure_micromamba(deps_dir)?;
    let root_prefix = deps_dir.join("mamba-root");
    let _ = fs::create_dir_all(&root_prefix);
    // A partial prefix from a failed run would look like a cache hit; clear it.
    let _ = fs::remove_dir_all(env_root);

    let mut args: Vec<String> = vec![
        "create".into(),
        "-y".into(),
        "-p".into(),
        env_root.display().to_string(),
        "-c".into(),
        "conda-forge".into(),
        format!("swi-prolog={SWIPL_VERSION}"),
    ];
    args.extend(SWIPL_ENV_PACKAGES.iter().map(|s| s.to_string()));

    println!("cargo:warning=Solving SWI-Prolog environment with micromamba (this downloads its dependency closure)...");
    let status = Command::new(&mm)
        .args(&args)
        .env("MAMBA_ROOT_PREFIX", &root_prefix)
        // Non-interactive / no user config interference.
        .env("CONDA_PKGS_DIRS", root_prefix.join("pkgs"))
        .env_remove("CONDARC")
        .status()
        .map_err(|e| format!("run micromamba: {e}"))?;
    if !status.success() {
        return Err(format!("micromamba create failed with status {status}"));
    }
    if !env_root.join("bin").join("swipl").exists() {
        return Err("micromamba env has no bin/swipl".into());
    }
    Ok(())
}

/// Remove build-time-only bloat from the conda env before we pack it (headers,
/// static libs, docs, man/info pages, pkg-config, conda metadata). Keeps every
/// runtime shared object and the Prolog home intact.
fn trim_swipl_env(env_root: &Path) {
    for rel in [
        "include",
        "share/doc",
        "share/man",
        "share/info",
        "lib/pkgconfig",
        "conda-meta",
    ] {
        let _ = fs::remove_dir_all(env_root.join(rel));
    }
    // Static archives are never needed at runtime.
    if let Ok(entries) = fs::read_dir(env_root.join("lib")) {
        for e in entries.flatten() {
            let p = e.path();
            if p.extension().and_then(|s| s.to_str()) == Some("a") {
                let _ = fs::remove_file(p);
            }
        }
    }
}

/// Post-process a swipl tree: sanity-check + make executables runnable. When
/// `bundle_libs` is set (source builds), also copy in the shared libs it needs.
fn finalize_swipl_install(install_root: &Path, bundle_libs: bool) -> Result<(), String> {
    let swipl_bin = install_root.join("bin").join("swipl");
    if !swipl_bin.exists() {
        return Err(format!(
            "swipl binary missing after install at {}",
            swipl_bin.display()
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for exe in ["bin/swipl", "bin/swipl-ld"] {
            let p = install_root.join(exe);
            if p.exists() {
                if let Ok(md) = fs::metadata(&p) {
                    let mut perms = md.permissions();
                    perms.set_mode(0o755);
                    let _ = fs::set_permissions(&p, perms);
                }
            }
        }
    }
    if bundle_libs {
        bundle_aux_libs(install_root);
    }
    Ok(())
}

/// A conda-forge build tool we can provision on demand for resiliency (so the
/// TerminusDB Rust build works even where the host lacks these). Currently only
/// linux-64 prebuilts are wired up; other arches rely on the host having them.
struct CondaTool {
    /// Package name (also the `.deps/<name>` and `vendor/<name>` subdir).
    name: &'static str,
    /// conda-forge `linux-64/<file>` filename.
    file: &'static str,
    /// md5 of that file (integrity check for downloads).
    md5: &'static str,
    /// A relative path that must exist after extraction (cache/sanity probe).
    probe: &'static str,
}

const M4_TOOL: CondaTool = CondaTool {
    name: "m4",
    file: "m4-1.4.21-hb03c661_0.conda",
    md5: "2c0696bb8251b054469ef84cc8953294",
    probe: "bin/m4",
};

const GMP_TOOL: CondaTool = CondaTool {
    name: "gmp",
    file: "gmp-6.3.0-h59595ed_0.conda",
    md5: "0e33ef437202db431aa5a928248cf2e8",
    probe: "include/gmp.h",
};

/// Provision a conda build tool into `.deps/<name>/<target>` and return its
/// root. Order mirrors swipl: cache → vendored `.conda` → download from
/// conda-forge. Returns `None` (not an error) for arches without a prebuilt so
/// callers can fall back to the host's own copy.
fn provide_conda_tool(tool: &CondaTool, target: &str, deps_dir: &Path) -> Option<PathBuf> {
    let root = deps_dir.join(tool.name).join(target);

    // (1) Cache.
    if root.join(tool.probe).exists() {
        return Some(root);
    }

    // Only linux-64 prebuilts are published/wired up here.
    if !(target.starts_with("x86_64") && target.contains("linux")) {
        return None;
    }

    // (2) Vendored (optional — these are small enough to vendor if desired).
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").ok()?;
    let vendored = PathBuf::from(&manifest_dir)
        .join("vendor")
        .join(tool.name)
        .join(target)
        .join(tool.file);
    let archive = if vendored.exists() {
        vendored
    } else {
        // (3) Download from conda-forge.
        let url = format!(
            "https://conda.anaconda.org/conda-forge/linux-64/{}",
            tool.file
        );
        let dl = deps_dir.join(tool.name).join(tool.file);
        if let Err(e) = download_file(&url, &dl) {
            println!("cargo:warning=Could not download {} ({}); relying on host copy", tool.name, e);
            return None;
        }
        if verify_md5(&dl, tool.md5).is_err() {
            println!("cargo:warning={} checksum mismatch; relying on host copy", tool.name);
            return None;
        }
        dl
    };

    if let Err(e) = extract_conda_package(&archive, &root) {
        println!("cargo:warning=Failed to extract {} ({}); relying on host copy", tool.name, e);
        return None;
    }
    if !root.join(tool.probe).exists() {
        return None;
    }
    #[cfg(unix)]
    if tool.probe.starts_with("bin/") {
        use std::os::unix::fs::PermissionsExt;
        let bin = root.join(tool.probe);
        if let Ok(md) = fs::metadata(&bin) {
            let mut perms = md.permissions();
            perms.set_mode(0o755);
            let _ = fs::set_permissions(&bin, perms);
        }
    }
    println!("cargo:warning=Provisioned {} at {}", tool.name, root.display());
    Some(root)
}

/// Extract a conda `.conda` package (a zip containing `pkg-*.tar.zst`) into
/// `dest`. The package payload has a standard `bin/ lib/ share/` prefix layout.
fn extract_conda_package(conda: &Path, dest: &Path) -> Result<(), String> {
    let file = fs::File::open(conda).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| format!("open zip: {}", e))?;

    // Locate the payload entry `pkg-*.tar.zst`.
    let pkg_name = (0..zip.len())
        .filter_map(|i| zip.by_index(i).ok().map(|f| f.name().to_string()))
        .find(|n| n.starts_with("pkg-") && n.ends_with(".tar.zst"))
        .ok_or_else(|| "no pkg-*.tar.zst entry in .conda package".to_string())?;

    let entry = zip.by_name(&pkg_name).map_err(|e| e.to_string())?;
    // Decode zstd → tar, then unpack (pure-Rust ruzstd decoder).
    let decoder =
        ruzstd::StreamingDecoder::new(entry).map_err(|e| format!("zstd decoder: {}", e))?;
    let mut archive = tar::Archive::new(decoder);
    archive.set_preserve_permissions(true);
    fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    archive
        .unpack(dest)
        .map_err(|e| format!("untar payload: {}", e))?;
    Ok(())
}

/// Copy the non-glibc shared libraries swipl links against into the bundle's
/// `lib/` so the tree runs even where those system libs are absent. glibc itself
/// (libc/libm/libpthread/libdl/ld-linux) is intentionally left to the host.
fn bundle_aux_libs(install_root: &Path) {
    let swipl_bin = install_root.join("bin").join("swipl");
    let dest_lib = install_root.join("lib");
    if fs::create_dir_all(&dest_lib).is_err() {
        return;
    }

    // Libraries we want to travel with the bundle (matched as substrings).
    const WANTED: &[&str] = &[
        "libgmp", "libtinfo", "libncurses", "libncursesw", "libz.", "libzstd",
        "libedit", "libreadline", "libssl", "libcrypto", "libffi", "libpcre",
        "libpcre2", "libgomp", "libunwind", "libtcmalloc",
    ];

    let output = match Command::new("ldd").arg(&swipl_bin).output() {
        Ok(o) if o.status.success() => o,
        _ => return,
    };
    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        // Format: "libgmp.so.10 => /usr/lib/.../libgmp.so.10 (0x...)"
        let path = match line.split("=>").nth(1) {
            Some(rest) => rest.trim().split_whitespace().next().unwrap_or("").to_string(),
            None => continue,
        };
        if path.is_empty() || !Path::new(&path).exists() {
            continue;
        }
        let fname = Path::new(&path)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        if !WANTED.iter().any(|w| fname.starts_with(w)) {
            continue;
        }
        let target = dest_lib.join(fname);
        if target.exists() {
            continue;
        }
        // Resolve symlinks so we copy the real object.
        let real = fs::canonicalize(&path).unwrap_or_else(|_| PathBuf::from(&path));
        if fs::copy(&real, &target).is_ok() {
            println!("cargo:warning=Bundled runtime lib {}", fname);
        }
    }
}

/// Best-effort source build of SWI-Prolog as the final fallback (arches with no
/// prebuilt). Requires a C toolchain + cmake; errors clearly if they are absent.
fn build_swipl_from_source(install_root: &Path, deps_dir: &Path) -> Result<(), String> {
    for tool in ["cmake", "cc"] {
        if !check_tool(tool) && !(tool == "cc" && (check_tool("gcc") || check_tool("clang"))) {
            return Err(format!(
                "Building SWI-Prolog from source needs `{}` but it was not found. \
                 Install a C toolchain + cmake (this path is only used on arches \
                 where conda-forge has no swi-prolog build, e.g. non-x86_64 Linux).",
                tool
            ));
        }
    }

    let src_dir = deps_dir.join("swipl-src");
    if !src_dir.join("CMakeLists.txt").exists() {
        let url = format!(
            "https://www.swi-prolog.org/download/stable/src/swipl-{}.tar.gz",
            SWIPL_VERSION
        );
        let tarball = deps_dir.join(format!("swipl-{}.tar.gz", SWIPL_VERSION));
        download_file(&url, &tarball)
            .map_err(|e| format!("failed to download swipl source: {}", e))?;
        let _ = fs::remove_dir_all(&src_dir);
        fs::create_dir_all(&src_dir).map_err(|e| e.to_string())?;
        let status = Command::new("tar")
            .args(["xzf", tarball.to_str().unwrap(), "--strip-components=1", "-C"])
            .arg(&src_dir)
            .status()
            .map_err(|e| format!("tar: {}", e))?;
        if !status.success() {
            return Err("failed to unpack swipl source tarball".to_string());
        }
    }

    let build_dir = src_dir.join("build");
    fs::create_dir_all(&build_dir).map_err(|e| e.to_string())?;
    let cmake_configure = Command::new("cmake")
        .current_dir(&build_dir)
        .args([
            "-DCMAKE_BUILD_TYPE=Release",
            "-DSWIPL_PACKAGES_JAVA=OFF",
            "-DSWIPL_PACKAGES_X=OFF",
        ])
        .arg(format!("-DCMAKE_INSTALL_PREFIX={}", install_root.display()))
        .arg("..")
        .status()
        .map_err(|e| format!("cmake configure: {}", e))?;
    if !cmake_configure.success() {
        return Err("cmake configure failed".to_string());
    }
    let build = Command::new("cmake")
        .current_dir(&build_dir)
        .args(["--build", ".", "--target", "install", "-j"])
        .status()
        .map_err(|e| format!("cmake build: {}", e))?;
    if !build.success() {
        return Err("cmake build/install failed".to_string());
    }
    Ok(())
}

/// Download `url` to `dest` using curl (already a hard dependency of this build).
fn download_file(url: &str, dest: &Path) -> Result<(), String> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let status = Command::new("curl")
        .args(["-fL", "--retry", "3", "-o", dest.to_str().unwrap(), url])
        .status()
        .map_err(|e| format!("curl: {}", e))?;
    if !status.success() {
        return Err(format!("curl failed for {}", url));
    }
    Ok(())
}

/// Verify a file's md5 matches `expected` (best-effort: skips if no md5 tool).
fn verify_md5(path: &Path, expected: &str) -> Result<(), String> {
    let out = Command::new("md5sum").arg(path).output();
    let digest = match out {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_string(),
        _ => {
            // Fall back to `md5` (BSD/macOS) or skip verification.
            match Command::new("md5").args(["-q"]).arg(path).output() {
                Ok(o) if o.status.success() => {
                    String::from_utf8_lossy(&o.stdout).trim().to_string()
                }
                _ => {
                    println!("cargo:warning=No md5 tool available; skipping checksum verification");
                    return Ok(());
                }
            }
        }
    };
    if digest != expected {
        return Err(format!(
            "checksum mismatch for {}: expected {}, got {}",
            path.display(),
            expected,
            digest
        ));
    }
    Ok(())
}

/// Pack a relocatable swipl root into `out_file` as a `.tar.gz` blob that
/// `lib.rs` embeds and extracts at runtime (pure-Rust gzip via flate2).
fn pack_swipl_home(swipl_root: &Path, out_file: &Path) -> Result<(), String> {
    let file =
        fs::File::create(out_file).map_err(|e| format!("create {}: {}", out_file.display(), e))?;
    let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::new(6));
    let mut builder = tar::Builder::new(encoder);
    builder.follow_symlinks(false);
    builder
        .append_dir_all(".", swipl_root)
        .map_err(|e| format!("tar append {}: {}", swipl_root.display(), e))?;
    let encoder = builder
        .into_inner()
        .map_err(|e| format!("tar finish: {}", e))?;
    encoder.finish().map_err(|e| format!("gzip finish: {}", e))?;
    Ok(())
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

/// Locate a system CA certificate bundle for OpenSSL/swipl HTTPS. Checks the
/// usual env vars first, then well-known distro paths. Returns `None` if none
/// found (the build then relies on whatever swipl defaults to).
fn find_ca_bundle() -> Option<PathBuf> {
    for var in ["SSL_CERT_FILE", "NIX_SSL_CERT_FILE", "CURL_CA_BUNDLE"] {
        if let Ok(p) = env::var(var) {
            let path = PathBuf::from(p);
            if path.exists() {
                return Some(path);
            }
        }
    }
    const CANDIDATES: &[&str] = &[
        "/etc/ssl/certs/ca-certificates.crt", // Debian/Ubuntu, Alpine
        "/etc/pki/tls/certs/ca-bundle.crt",   // Fedora/RHEL
        "/etc/ssl/cert.pem",                  // macOS/BSD
        "/etc/ssl/ca-bundle.pem",             // openSUSE
    ];
    CANDIDATES
        .iter()
        .map(PathBuf::from)
        .find(|p| p.exists())
}

/// Write a small SWI-Prolog module that forces every `http_open/3` to trust the
/// given CA bundle, working around broken `system(root_certificates)` in
/// relocatable/conda swipl builds. Returns the path to preload with `swipl -l`.
fn write_swipl_certfix(swipl_root: &Path, ca_bundle: &Path) -> Result<PathBuf, String> {
    let path = swipl_root.join("certfix.pl");
    // Prolog atoms use single quotes; CA paths are plain filesystem paths.
    let contents = format!(
        ":- module(certfix, []).\n\
         :- multifile http:open_options/2.\n\
         http:open_options(_, [cacerts([file('{}')])]).\n",
        ca_bundle.display()
    );
    fs::write(&path, contents).map_err(|e| format!("write {}: {}", path.display(), e))?;
    Ok(path)
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

    // When we provisioned our own relocatable swipl (Linux), point the build at
    // its home + bundled shared libs so `make` uses exactly the swipl we will
    // also bundle into the embed.
    // Assemble shared-library search dirs from the provisioned swipl and gmp.
    let mut ld_dirs: Vec<PathBuf> = Vec::new();
    let swipl_home = ctx.swipl_root.as_ref().map(|root| {
        let home = root.join("lib").join("swipl");
        ld_dirs.push(root.join("lib"));
        let arch_lib = home.join("lib").join(plarch());
        if arch_lib.exists() {
            ld_dirs.push(arch_lib);
        }
        home
    });
    if let Some(ref gmp) = ctx.gmp_root {
        ld_dirs.push(gmp.join("lib"));
    }
    // libclang's lib dir also holds its co-located libLLVM; bindgen dlopens it.
    let libclang_lib = ctx.libclang_root.as_ref().map(|r| r.join("lib"));
    if let Some(ref lc) = libclang_lib {
        ld_dirs.push(lc.clone());
    }
    // clang's builtin resource headers (stddef.h, ...) — pass to bindgen explicitly
    // so it finds them regardless of how it derives the resource dir from libclang.
    let clang_resource_inc = ctx.libclang_root.as_ref().and_then(|r| {
        fs::read_dir(r.join("lib").join("clang")).ok().and_then(|rd| {
            rd.flatten()
                .map(|e| e.path().join("include"))
                .find(|p| p.join("stddef.h").exists())
        })
    });
    if let Ok(existing) = env::var("LD_LIBRARY_PATH") {
        ld_dirs.extend(env::split_paths(&existing));
    }
    let swipl_ld_path = if ld_dirs.is_empty() {
        None
    } else {
        Some(
            env::join_paths(&ld_dirs)
                .map_err(|e| format!("Failed to build LD_LIBRARY_PATH: {}", e))?,
        )
    };

    // Build-time header/lib/pkg-config paths for a provisioned GMP so any C
    // build (e.g. gmp-mpfr-sys' fallbacks, or extensions linking libgmp) finds it.
    let gmp_paths = ctx.gmp_root.as_ref().map(|gmp| {
        let prepend = |var: &str, dir: PathBuf| -> Result<std::ffi::OsString, String> {
            let mut dirs = vec![dir];
            if let Ok(existing) = env::var(var) {
                dirs.extend(env::split_paths(&existing));
            }
            env::join_paths(&dirs).map_err(|e| format!("build {}: {}", var, e))
        };
        (
            prepend("C_INCLUDE_PATH", gmp.join("include")),
            prepend("CPLUS_INCLUDE_PATH", gmp.join("include")),
            prepend("LIBRARY_PATH", gmp.join("lib")),
            prepend("PKG_CONFIG_PATH", gmp.join("lib").join("pkgconfig")),
        )
    });

    // `make install-deps` runs swipl's `pack_install`, which fetches over HTTPS.
    // A relocatable/conda swipl frequently ships a broken `system(root_certificates)`
    // on Debian-style hosts (it reads a BSD-style `/etc/ssl/cert.pem` that doesn't
    // exist and ignores SSL_CERT_FILE / the `system_cacert_filename` flag). To make
    // HTTPS verify reliably we preload a tiny module that injects an explicit CA
    // bundle into every `http_open/3` call via the `http:open_options/2` hook, and
    // point swipl at it by overriding the `SWIPL` make variable for that target.
    let ca_bundle = find_ca_bundle();
    let swipl_override = match (&ctx.swipl_root, &ca_bundle) {
        (Some(root), Some(ca)) => match write_swipl_certfix(root, ca) {
            Ok(certfix) => Some(format!(
                "SWIPL={} -l {}",
                root.join("bin").join("swipl").display(),
                certfix.display()
            )),
            Err(e) => {
                println!("cargo:warning=Could not write swipl certfix ({}); HTTPS pack install may fail", e);
                None
            }
        },
        _ => None,
    };

    // Helper to apply the shared build environment to a `make` command.
    let apply_env = |cmd: &mut Command| {
        cmd.env("PATH", &new_path)
            .env_remove("RUSTFLAGS")
            .env_remove("CARGO_ENCODED_RUSTFLAGS")
            .env_remove("RUSTC_WORKSPACE_WRAPPER")
            .env_remove("RUSTC_WRAPPER");
        if let Some(ref home) = swipl_home {
            cmd.env("SWI_HOME_DIR", home);
        }
        if let Some(ref ld) = swipl_ld_path {
            cmd.env("LD_LIBRARY_PATH", ld);
        }
        if let Some(ref ca) = ca_bundle {
            cmd.env("SSL_CERT_FILE", ca);
            if let Some(dir) = ca.parent() {
                cmd.env("SSL_CERT_DIR", dir);
            }
        }
        if let Some((ref c_inc, ref cpp_inc, ref lib, ref pc)) = gmp_paths {
            if let Ok(v) = c_inc {
                cmd.env("C_INCLUDE_PATH", v);
            }
            if let Ok(v) = cpp_inc {
                cmd.env("CPLUS_INCLUDE_PATH", v);
            }
            if let Ok(v) = lib {
                cmd.env("LIBRARY_PATH", v);
            }
            if let Ok(v) = pc {
                cmd.env("PKG_CONFIG_PATH", v);
            }
        }
        // Force C/C++17: GMP 6.x's bundled configure (built from source by
        // gmp-mpfr-sys/rug in the TerminusDB Rust build) fails under GCC 15's C23
        // default ("too many arguments to function 'g'"). Append so any host flags
        // are preserved.
        let append_std = |var: &str, std: &str| -> std::ffi::OsString {
            match env::var(var) {
                Ok(v) if !v.trim().is_empty() => format!("{v} {std}").into(),
                _ => std.into(),
            }
        };
        cmd.env("CFLAGS", append_std("CFLAGS", "-std=gnu17"));
        cmd.env("CXXFLAGS", append_std("CXXFLAGS", "-std=gnu++17"));
        if let Some(ref lc) = libclang_lib {
            cmd.env("LIBCLANG_PATH", lc);
        }
        if let Some(ref inc) = clang_resource_inc {
            let mut val = std::ffi::OsString::from("-isystem ");
            val.push(inc);
            if let Ok(existing) = env::var("BINDGEN_EXTRA_CLANG_ARGS") {
                if !existing.trim().is_empty() {
                    val.push(" ");
                    val.push(existing);
                }
            }
            cmd.env("BINDGEN_EXTRA_CLANG_ARGS", val);
        }
    };

    // Install tus pack
    // Clear cargo/clippy flags to prevent clippy from propagating to external TerminusDB build
    // RUSTC_WORKSPACE_WRAPPER is how cargo clippy injects clippy-driver
    let mut install_deps_cmd = Command::new("make");
    install_deps_cmd.arg("install-deps").current_dir(repo_path);
    if let Some(ref o) = swipl_override {
        install_deps_cmd.arg(o);
    }
    apply_env(&mut install_deps_cmd);
    let status = install_deps_cmd
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
    let mut build_cmd = Command::new("make");
    build_cmd.args(make_args).current_dir(repo_path);
    apply_env(&mut build_cmd);
    let status = build_cmd
        .status()
        .map_err(|e| format!("Failed to run make: {}", e))?;

    if !status.success() {
        return Err(format!("make failed with status: {}", status));
    }

    println!("cargo:warning=TerminusDB build completed successfully");
    Ok(())
}
