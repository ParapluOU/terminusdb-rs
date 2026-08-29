# terminusdb-bin — AI assistant notes

This crate is a **build-script wrapper**: its `build.rs` compiles TerminusDB (SWI-Prolog +
a Rust FFI dylib via the `swipl-fli`/`bindgen` binding) and embeds the resulting server
binary. Anything that depends on it (e.g. `lib/proof-model-tests`, which spawns an embedded
proof-capable HTTP server) triggers that whole native build.

## The recurring build failure you will hit (and the fix)

**Symptom:** `build.rs` panics with `Failed to build TerminusDB: make rust ... failed`; the
real error above it is from `swipl-fli`'s `bindgen`, one of:
- `error: unknown type name 'intmax_t' / 'uintptr_t'`, or
- `Unable to find libclang: ... libstdc++.so.6: cannot open shared object file`.

**Root cause:** bindgen must `dlopen` a `libclang` whose **version matches** the clang
builtin resource headers (`lib/clang/<v>/include/stdint.h`) it is told to use, and that
libclang's runtime deps must load. The auto-provisioned `.deps/libclang-env` can be
*internally inconsistent* (e.g. `libclang.so → .13` but only `clang/22/include` headers
shipped) → the `intmax_t` error. See the README "Troubleshooting" section for the full
explanation.

**Fix (general):** point at a *coherent* libclang — matching lib version + resource headers,
with loadable deps. The host system LLVM (`/usr/lib/llvm-<N>`) is the reliable source:

```sh
export LIBCLANG_PATH=/usr/lib/llvm-<N>/lib
export BINDGEN_EXTRA_CLANG_ARGS="-I/usr/lib/llvm-<N>/lib/clang/<N>/include"   # MATCHING version
export LD_LIBRARY_PATH="/usr/lib/llvm-<N>/lib:$LD_LIBRARY_PATH"
```

Then build the dylib **directly once** so `build.rs` skips its own (bindgen-running)
`make rust`:

```sh
(cd <terminusdb-repo>/src/rust && cargo build -p terminusdb-dylib --release \
   && cp target/release/libterminusdb_dylib.so librust.so)
```

`build.rs` skips the rebuild when `src/rust/librust.so` is newer than the Rust sources.

## Key facts for navigating build.rs

- `repo_path` (the TerminusDB checkout it builds) is the sibling `modules/terminusdb`; the
  dylib crate is `modules/terminusdb/src/rust/terminusdb-dylib`, and `librust.so` lives at
  `modules/terminusdb/src/rust/librust.so`.
- Provisioned toolchain is cached under `crates/bin/.deps/` (gitignored): `swipl-env`,
  `gmp`, `protoc`, `m4`, `libclang-env`, `micromamba`.
- `build.rs` auto-injects the matching resource `-I` **only when it self-provisions**
  libclang (i.e. `LIBCLANG_PATH` unset). With an external `LIBCLANG_PATH` you must also set
  `BINDGEN_EXTRA_CLANG_ARGS`.
- The canonical full env recipe is `modules/terminusdb/docs/QUERY_PROOF_RELEASE_TESTS.md`;
  cross-repo build source-of-truth is `modules/terminusdb-rs/docs/WOQL_QUERY_PROOF_GPU_HANDOFF.md`.

> The **exact, environment-specific** working values (absolute `.deps` path, the local
> libclang shim dir, the pinned toolchain) are recorded in the assistant's memory under
> `terminusdb-bin-libclang-build-fix` — check there first; the committed docs stay portable.

## `.deps/build-env.sh` (resolved env, auto-emitted)

On a successful build, `build.rs` writes the native build env it resolved to
`.deps/build-env.sh` (PATH, `SWI_HOME_DIR`, `LD_LIBRARY_PATH`, GMP dirs, `CFLAGS`/`CXXFLAGS`,
`LIBCLANG_PATH`/`BINDGEN_EXTRA_CLANG_ARGS`). Prefer `. .deps/build-env.sh` over hand-assembling
the env when you need a manual `cargo build -p terminusdb-dylib`. It captures whichever libclang
was in effect — including a correct system-LLVM one you passed in — so it's the fastest way to
reproduce a known-good env. Regenerated only when build.rs actually runs.
