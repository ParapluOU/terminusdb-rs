# terminusdb-bin

Build script crate that compiles TerminusDB from source and embeds it as a Rust executable.

## Features

- ✅ Verifies required dependencies (git, make, SWI-Prolog, GMP, protoc)
- ✅ Auto-downloads and bundles protoc if missing
- ✅ Clones TerminusDB from GitHub
- ✅ Builds with `make PROFILE=release`
- ✅ Embeds compiled binary with `include_bytes!`
- ✅ Provides CLI wrapper that forwards all arguments
- ✅ Caches builds (skip rebuild unless forced)

## Environment Variables

- `TERMINUSDB_VERSION`: Git branch/tag to build (default: "main")
- `TERMINUSDB_FORCE_REBUILD`: Set to "1" to force rebuild even if cached

## Dependencies

### Required (must be installed manually)
- **git** - For cloning TerminusDB repository
- **make** - For running TerminusDB build process
- **SWI-Prolog** - Prolog runtime required by TerminusDB
  - macOS: `brew install swi-prolog`
  - Linux: `sudo apt-add-repository ppa:swi-prolog/stable && sudo apt-get update && sudo apt-get install swi-prolog`
- **GMP library** - Math library required by SWI-Prolog
  - macOS: `brew install gmp`
  - Linux: `sudo apt-get install libgmp-dev`

### Optional (auto-bundled if missing)
- **protoc** - Protocol Buffers compiler
  - Downloaded to `.deps/protoc/` if not found in system

## Usage

```rust
use terminusdb_bin::run_terminusdb;

// Run TerminusDB with arguments
let status = run_terminusdb(&["--version"])?;
```

Or use the CLI wrapper:

```bash
cargo build --release -p terminusdb-bin
./target/release/terminusdb serve
```

## Build Process

1. Checks for required dependencies
2. Downloads protoc if needed (to `.deps/protoc/`)
3. Clones TerminusDB to temp directory
4. Runs `make install-deps` (installs SWI-Prolog packs)
5. Runs `make dev` on macOS (preserves library signatures) or `make PROFILE=release` on Linux
6. Copies binary and (on macOS) librust.dylib to `OUT_DIR`
7. Embeds binary into Rust executable

## Platform Differences

### macOS

On macOS, the build uses `make dev` which creates a development build with `foreign(no_save)` in SWI-Prolog's `qsave_program`. This preserves the code signatures of dynamically loaded libraries, allowing the binary to run without requiring code signing or quarantine workarounds.

The dev build requires `librust.dylib` at runtime, which is automatically bundled and extracted alongside the binary.

### Linux

On Linux, the build uses `make PROFILE=release` which creates a standalone release binary

## Files

- `build.rs` - Build script that compiles TerminusDB
- `src/lib.rs` - Library exports for binary extraction
- `src/main.rs` - CLI wrapper executable
- `.deps/` - Bundled dependencies (gitignored)

## Build Time

- First build: ~1-2 minutes (clones and compiles TerminusDB)
- Subsequent builds: <1 second (uses cached binary)
- Force rebuild: `TERMINUSDB_FORCE_REBUILD=1 cargo build`
