# Pre-built TerminusDB Binaries

This directory contains pre-built TerminusDB binaries for cross-compilation.

> **⚠️ STALE — needs a v12.1 rebuild.** The macOS binaries here
> (`aarch64-apple-darwin/`) were built from the pre-v12 (TerminusDB 11) fork and
> do NOT match the current pinned server (`v12.1-rc-paraplu.1`, see
> `crates/bin/build.rs`). Rebuild them on a macOS host from the pinned fork tag
> before relying on the cross-compilation path; `x86_64-apple-darwin/` has never
> been populated. (Linux builds from source and does not use these.)

## Structure

```
prebuilt/
├── aarch64-apple-darwin/
│   ├── terminusdb
│   └── librust.dylib
└── x86_64-apple-darwin/
    ├── terminusdb
    └── librust.dylib
```

## Building Pre-built Binaries

To create pre-built binaries for a target platform:

1. On the target platform (e.g., macOS), ensure dependencies are installed:
   ```bash
   brew install swi-prolog gmp
   ```

2. Clone and build TerminusDB:
   ```bash
   git clone --depth=1 https://github.com/ParapluOU/terminusdb.git
   cd terminusdb
   make install-deps
   make PROFILE=release dev
   ```

3. Copy the binaries to the appropriate prebuilt directory:
   ```bash
   cp terminusdb prebuilt/<target-triple>/
   cp src/rust/librust.dylib prebuilt/<target-triple>/
   ```

## Target Triples

- `aarch64-apple-darwin` - macOS Apple Silicon (M1/M2/M3)
- `x86_64-apple-darwin` - macOS Intel

## Notes

- For macOS, both `terminusdb` and `librust.dylib` are required
- The build.rs automatically detects cross-compilation and uses these binaries
- Native builds (same host/target) will build from source as usual
