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
Standard build-host tools for compiling TerminusDB and its Rust/Prolog
extensions from source (on Debian/Ubuntu: `apt install build-essential`, plus
the extras below):
- **git** - For cloning the TerminusDB repository
- **make** + a C/C++ toolchain (`gcc`/`clang`) - TerminusDB build process
- **m4** - Required by `gmp-mpfr-sys` (bignum) when it builds GMP from source
- **clang / libclang** - Required by `bindgen` in the `swipl-fli` binding
- **node** + **npm** and **elm** - For the dashboard build
- **cmake** - Only for the SWI-Prolog source-build fallback (non-x86_64 Linux)

### SWI-Prolog (Linux: fully automatic & self-contained)
On **Linux** you do **not** need to install SWI-Prolog. The build provisions a
relocatable SWI-Prolog 10.x — together with its *full* dependency closure — and
bundles it into the embedded binary, so the resulting server runs on a bare
Linux box with no swipl installed. Acquisition order (each a fallback for the
previous):
1. Cached env under `.deps/swipl-env/<target>/`
2. A `micromamba` conda env (`swi-prolog=10.0.0` + `libxcrypt`), which solves and
   downloads the whole closure into one relocatable prefix. `micromamba` itself
   is a static binary downloaded automatically to `.deps/micromamba/`.
3. Compile from source (needs cmake + C toolchain; covers arches conda-forge has
   no swi-prolog build for, e.g. currently non-x86_64 Linux).

Why the full closure (not just the swi-prolog package): swipl's foreign
extensions (`archive`, `crypt`, `ssl`, …) each pull sibling shared libraries
(`libarchive` → zstd/lz4/xml2/krb5/…, `libxcrypt` → `libcrypt.so.2`). The
`qsave` bootstrap autoloads them, so all must be present or the build fails —
and `libcrypt.so.2` isn't satisfied by Debian's `libcrypt.so.1`. micromamba
resolves this closure correctly; the build then packs the trimmed prefix
(headers/docs/static libs removed) into `swipl-home.tar.gz` for embedding.

On **macOS** SWI-Prolog is still a manual prerequisite (self-contained macOS
bundling is out of scope for now):
- `brew install swi-prolog gmp`

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

On Linux, the build uses `make PROFILE=release` which creates a standalone
release binary. Because that binary is a SWI-Prolog *saved state* that needs its
Prolog home at runtime, the build packs the provisioned, relocatable SWI-Prolog
home into `OUT_DIR/swipl-home.tar.gz` and embeds it. At runtime the library
extracts it to the cache dir and points the binary at it via `SWI_HOME_DIR` +
`LD_LIBRARY_PATH` (see `apply_runtime_env`). The non-glibc shared libraries
swipl needs (GMP, ncurses/tinfo, zlib, ...) are copied into the bundle so it is
self-contained on a bare box.

## Files

- `build.rs` - Build script that compiles TerminusDB
- `src/lib.rs` - Library exports for binary extraction
- `src/main.rs` - CLI wrapper executable
- `.deps/` - Bundled dependencies (gitignored)

## Build Time

- First build: ~1-2 minutes (clones and compiles TerminusDB)
- Subsequent builds: <1 second (uses cached binary)
- Force rebuild: `TERMINUSDB_FORCE_REBUILD=1 cargo build`
