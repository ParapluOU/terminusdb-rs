# WOQL query-proof GPU handoff

This is the continuation plan for query-proof coverage and accelerated proving.
Keep the semantic coverage matrix in
[`WOQL_QUERY_PROOF_COVERAGE.md`](WOQL_QUERY_PROOF_COVERAGE.md) authoritative;
this file records cross-repository state, performance work, and validation.

## Current checkpoint (2026-08-09)

The proof wire format is independent of the prover backend. The caller owns one
`StoreProverContext` and passes it through `CompiledBgp::prove_with_context`.
TerminusDB exposes the same model as an owned `QueryProver` through the Prolog
blob bridge. Backend selection and proof-generation lifecycle remain application
policy, not Store policy.

| Repository | Checkpoint | State |
| --- | --- | --- |
| `modules/dory` | `2c571d4` | Caller-owned Arkworks/Blitzar G1 backend plus a profiled, canonical provider-neutral BN254 G2 boundary; proof bytes remain unchanged. |
| `modules/terminusdb-store-paraplu` | `a93ab32` | Ascending and descending exact scalar order; descending derives `MAX*chi-value` from authenticated commitments and reuses strict sort. |
| `modules/terminusdb-rs` | `f65e97e` plus this documentation commit | WOQL `OrderBy` direction is Store-plan, verifier, recursive-envelope, and digest bound under the same bounded shape. |
| `modules/terminusdb` | `de8ecd7c` | Owned CPU/Blitzar query provers and Prolog bridge. |

The canonical proof input is `woql2::Query`, not JSON-LD bytes. At the server
boundary parsed JSON is decoded with the schema crate's explicit
`Query::from_json` decoder (not serde-derived AST decoding). Prolog is the
leading definition of WOQL shapes; update that decoder and `woql2` when the
server's accepted structure differs.

## Benchmark commands

Run from `modules/terminusdb-rs`. Record the commit, OS/architecture, CPU/GPU,
driver/CUDA versions, `RAYON_NUM_THREADS`, tier, row counts, and precomputation
count with every result. Compare identical arguments and a warm filesystem.

Stock Arkworks CPU (portable, including macOS):

```sh
RAYON_NUM_THREADS=4 cargo run --release -p terminusdb-woql2 \
  --features proof --example proof_performance -- \
  --backend cpu --tier log20 --rows 64 --group-rows 128 --precompute 0
```

Blitzar native CPU (Linux x86-64 only):

```sh
RAYON_NUM_THREADS=4 cargo run --release -p terminusdb-woql2 \
  --features blitzar-cpu --example proof_performance -- \
  --backend blitzar-cpu --tier log20 --rows 64 --group-rows 128 --precompute 0
```

Blitzar CUDA (Linux x86-64, NVIDIA GPU and CUDA only):

```sh
RAYON_NUM_THREADS=4 cargo run --release -p terminusdb-woql2 \
  --features blitzar-gpu --example proof_performance -- \
  --backend blitzar-gpu --tier log20 --rows 64 --group-rows 128 --precompute 0
```

The runner measures native runtime and setup initialization separately, warms
the GPU once, and then exercises a small BGP, recursive
`Or`/`Optional`/`Not`, and grouped minimum. CPU and GPU Blitzar features are
mutually exclusive. Test `log20` first; only move to `log22` or `log24` when the
query domain requires it. Sweep `--precompute` only after obtaining the zero
precomputation baseline.

## Mac validation path

The current `blitzar-sys` 1.81.0 build script rejects every non-Linux OS and
non-x86-64 architecture. Therefore neither `blitzar-cpu` nor `blitzar-gpu` is a
valid native Mac test today, including on an Intel Mac. Apple Silicon and Apple
GPU acceleration would require a different backend (for example Metal) or a
maintained portable fork; CUDA/Blitzar cannot provide it.

The Mac is still valuable for the portable CPU baseline and proof portability:

```sh
cd modules/terminusdb-rs
rustc -Vv
uname -a
RAYON_NUM_THREADS=4 cargo run --release -p terminusdb-woql2 \
  --features proof --example proof_performance -- \
  --backend cpu --tier log20 --rows 64 --group-rows 128 --precompute 0
cargo test -p terminusdb-woql2 --features proof --test proof_plan
cargo test -p terminusdb-woql2 --features proof --test proof_prove
```

For a meaningful portability check, retain a Linux-generated envelope and
compact verifier artifact as fixtures in a future checkpoint, then verify them
on macOS and compare their bytes with a Mac CPU-generated proof for the same
fixed layer/query. Do not claim GPU validation from the Mac CPU run.

## Validation matrix

| Target | Required result | Status |
| --- | --- | --- |
| Arkworks CPU, default proof features | Full proof tests and benchmark scenarios verify. | Passed on Linux. |
| Blitzar CPU, Dory unit level | Edge scalar lengths/values, fallback paths, commitments, and full proof bytes equal Arkworks. | Passed on Linux x86-64. |
| Blitzar CPU, recursive WOQL envelope | Envelope bytes equal CPU and verify against the same root/query. | Passed on Linux x86-64. |
| Blitzar GPU feature, all Rust layers | Dory, Store, woql2, QueryProver, and Prolog bridge compile. | Passed on Linux x86-64; execution not tested. |
| Caller-owned G2 provider seam | Edge MSMs and a complete opening equal Arkworks, serialized bytes match, and Store reports G2 work after type erasure. | Passed on Linux; hardware provider pending. |
| Canonical ICICLE interchange boundary | Scalar and affine G2 identity/edge/random round trips; noncanonical fields, malformed infinity, and invalid points reject. | Passed without native ICICLE; adapter compile/link pending cloud. |
| Blitzar GPU hardware | Benchmark all three scenarios, exact proof/envelope equality, verification, repeated-run stability, and peak memory. | Pending NVIDIA host. |
| macOS portable CPU | Benchmark plus `proof_plan` and `proof_prove`; verify portable fixtures when added. | Pending Mac. |
| TerminusDB running node | Opt-in release suite from `modules/terminusdb/docs/QUERY_PROOF_RELEASE_TESTS.md`. | Passed for CPU; accelerated prover integration needs a hardware run. |

Hardware results are acceptable only if verification succeeds and proof and
envelope bytes remain identical to the CPU run. Performance alone is not a pass.

## BN254 G2 assessment

Blitzar cannot safely accelerate Dory's G2 work. In Blitzar 4.3.0,
`ElementP2<C>` denotes short-Weierstrass projective coordinates, not a pairing
curve's second source group. Its `SXT_CURVE_BN_254` ABI accepts
`sxt_bn254_g1_p2`; neither the Rust curve map nor `blitzar-sys` 1.81.0 exposes a
BN254 G2/Fq2 curve identifier or point ABI. Never reinterpret `ArkG2` as that
type.

Dory checkpoints `82e9611` and `8fdf2c5` instead expose a generic exact G2
provider plus `ProfiledBackend` and a retainable observation handle. Store
checkpoint `46607d4` threads it through `StoreProverContext` using
`cpu_with_g2_backend` and `blitzar_with_g2_backend`. Edge-size/scalar tests and
a complete Dory opening establish exact Arkworks and serialized proof-byte
equivalence. A real accelerator must implement `ProverBackend<ArkG2>` and pass
the same acceptance tests; no proof or lifecycle changes are required.

Official ICICLE v4.0.0, pinned to
`6c77775961b0e4dc2be359feec1ac11536c18043`, does expose BN254 G2 MSM in its
Rust API and is the current provider candidate. Dory checkpoint `2c571d4` adds
tested canonical scalar and G2 affine interchange values and records the exact
adapter/runtime/fallback design in `modules/dory/ICICLE_G2_DESIGN.md`. It does
not add ICICLE dependencies or an uncompiled feature: this host lacks CMake,
Clang/C++, CUDA, an NVIDIA device, and installed ICICLE frontend/backend
libraries, while ICICLE v4's Rust build invokes CMake. The optional
`icicle-gpu` provider must be compiled, linked, and equivalence-tested first on
the prepared cloud server.

## Active WOQL coverage checklist

GPU implementation is parked until WOQL completeness is substantially higher. The
current semantic checkpoint adds an exact scalar ascending `OrderBy` slice on top of the
existing recursively composable read relation. The Store proof covers full-row
permutation (including duplicate preservation), root-authenticated dictionary lookup,
datatype completeness, strict ordering, recursive wire round-trip, and plan tamper
rejection. The woql2 planner accepts only the complete one-column, nonnullable,
exact-datatype leaf-BGP form. Its tests cover canonical domain decoding, outer routing,
resolved ordered results, envelope round-trip, datatype/query/row-order substitution,
and fail-closed descending, multikey, partial-key, nullable, unknown-domain, and
duplicate-generating recursive children.

- [x] Exact non-ground dictionary absence, recursively composable.
- [x] Full-row authenticated permutation, preserving multiplicity and tuple correlation.
- [x] Bounded exact scalar ascending/descending `OrderBy` over a complete one-column leaf result.
- [x] Exact `Start`/`Limit` contiguous slices over that authenticated ordered result.
- [x] `Once` over an authenticated ordered child, digest-distinct from `Limit(1)`.
- [ ] Run the full Store proof suite and woql2 `proof_plan`/`proof_prove` suites when
  time permits; focused semantic tests are the commit gate because full Arkworks runs
  take minutes.
- [ ] Extend general lexicographic/stable-tie ordering and triple navigation.
  `Once` can now be implemented exactly as `Limit(1)` only when its child already has an
  authenticated order; unordered first-solution semantics remain unsupported.
- [ ] Implement Prolog `When` exactly. `compile_wf(when(A,B), ((ProgA,ProgB);true))`
  proves it is not conventional if/else: matched `A,B` rows are bag-unioned with the
  unchanged outer solution. Do not normalize it to the existing left-outer gadget.
- [ ] Define a versioned authenticated Prolog-list/generated-scalar contract, then
  generalize `GroupBy`/`Collect`/`Sum` and list-consuming extrema.
- [ ] Add operators requiring new value circuits/data models in the order recorded in
  the coverage matrix; keep every unsupported case fail-closed.
- [ ] Resume ICICLE/GPU work only after the semantic items above or when proving
  performance becomes the explicit active milestone. Mac remains a portable CPU and
  proof-compatibility target, not a CUDA target.

## Ordered acceleration continuation plan

1. Run the Mac CPU validation above and record the exact environment and output.
2. On Linux/NVIDIA, run CPU, Blitzar CPU, and Blitzar GPU with identical inputs.
   Repeat enough times to separate initialization from steady-state proving.
3. On the prepared cloud server, implement the exact-revision ICICLE adapter
   from `modules/dory/ICICLE_G2_DESIGN.md`, validate generator conversion first,
   then run edge MSM, full proof-byte, and recursive Store envelope equivalence.
4. Profile the remaining wall time and retain separate G1/G2 backend snapshots.
   Today only BN254 G1 MSM is accelerated; G2 MSM, pairings, sumcheck, relation
   construction, and vector work remain on CPU. Use measurements before choosing
   a G2 CUDA provider or the next kernel/backend.
5. Tune a minimum offload size and precomputation count. Preserve explicit
   application-owned backend creation; do not add automatic lifecycle policy to
   Store.
6. Vendor or independently checksum the `blitzar-sys` native library before
   production use. Its build script downloads a release `.so` without enforcing
   a checksum. See `modules/dory/PROVENANCE.md` for pinned versions and the
   observed Linux artifact digest.
7. Resume this acceleration plan only after the active WOQL checklist above.
8. Publish or register the Dory fork and Store fork so a fresh checkout resolves
   the exact committed sources rather than relying on sibling directories.

## Commit and handoff discipline

- Before editing, record `git status --short` and the current commit in all four
  repositories. Never absorb unrelated work.
- Make one bounded change at a time and commit after each green checkpoint:
  semantic coverage, proof gadget, context/backend integration, server bridge,
  then benchmark/docs. Do not leave a cross-repository change represented only
  by an outer pointer commit.
- Every semantic commit must add positive, negative/tamper, and fail-closed tests.
  Every acceleration commit must additionally establish CPU byte equivalence.
- Update this handoff and `WOQL_QUERY_PROOF_COVERAGE.md` in the same checkpoint
  when capability or constraints change. Record commands and measured output,
  not only “tests pass”.
- Use scoped `cargo clean -p ...` or repository-local `cargo clean` when disk
  pressure requires it; inspect targets first and do not remove unrelated data.
- Before handing off, verify each repository is clean, list its HEAD, and call
  out intentionally untracked sibling repositories or unrelated root changes.

## Known constraints

- Blitzar 4.3.0 / `blitzar-sys` 1.81.0 supports Linux x86-64 only; its GPU mode
  requires NVIDIA CUDA.
- Blitzar runtime initialization is explicit in our API, but upstream native
  initialization is process-global and the first configuration fixes it for the
  process. There is no shutdown path.
- The adapter accelerates matching setup-prefix G1 bases. Other bases and G2
  work fall back to Arkworks CPU. A future G2 provider plugs into Dory/Store's
  exact `ArkG2` boundary; Blitzar's BN254 G1 ABI is not usable for it.
- Storage proof support is unconditional. Acceleration is an optional prover
  backend and must not create alternate layer or proof formats.
- Proof generation lifecycle, scheduling, caching, and admission control belong
  to the application/`terminusdb-rs` domain and are deliberately not selected
  here.
