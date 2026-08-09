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
| `modules/terminusdb-store-paraplu` | `bea5b24` | Recursive exact unordered distinct-class partitions for one or two nonnullable Store-ID columns, with v9 wire coverage. |
| `modules/terminusdb-rs` | `3535898` plus this documentation commit | Canonical `Count(Distinct(...))` planning authenticates projected one-/two-witness class counts over recursive relations while keeping first-occurrence order unobservable. |
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

Before the broad Mac suites, the exact new semantic gates can be run with:

```sh
cargo test -p terminusdb-woql2 --features proof --test proof_plan \
  exact_complete_row_multikey_order_compiles_and_rejects_key_mappings -- --exact
cargo test -p terminusdb-woql2 --features proof --test proof_prove \
  multikey_order_nested_limit_e2e -- --exact
cargo test -p terminusdb-schema --lib \
  xsdtype::tests::canonical_fixed_width_integer_json_roundtrips_edges -- --exact
cargo test -p terminusdb-woql2 --features proof --test proof_plan \
  accepts_every_exact_fixed_width_integer_datatype_at_edges_and_zero -- --exact
cargo test -p terminusdb-woql2 --features proof --test proof_prove \
  object_range_int8_ge_zero_authenticates_signed_width_and_dictionary_tag -- --exact
cargo test -p terminusdb-woql2 --features proof --test proof_plan \
  count_accepts_exact_projected_distinct_over_one_or_two_known_domains -- --exact
cargo test -p terminusdb-woql2 --features proof --test proof_prove \
  count_projected_distinct_proves_duplicate_classes_and_rejects_tamper -- --exact
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

The current semantic checkpoint permits equal adjacent keys without weakening strict
callers that depend on distinctness. Store's positional shift proof now has explicit
strict and nondecreasing modes bound before Fiat-Shamir challenges. Strict join/index
callers retain the strict wrappers. Exact scalar and lexicographic ordering use only
nondecreasing mode, while the existing exact multiset permutation preserves duplicate
multiplicity and tuple correlation. Since the accepted ordering keys are the complete
public schema, equal keys imply identical complete rows and SWI's stable tie order is
observationally irrelevant. `woql2` therefore accepts recursively proved bag-producing
children only under the existing complete-key, nonnullable, exact fixed-width datatype
guards.

Focused Linux Store gates passed for duplicate/all-equal/u64-boundary/padding/decrease
(42.24s), strict/nondecreasing mode substitution (10.31s), lexicographic ties and
direction tamper (33.74s), complete-row duplicate multiplicity and changed-content
tamper (45.54s), and scalar duplicate preservation plus mixed-type rejection (46.92s).
The duplicate `Or` -> `OrderBy(y DESC, x ASC)` -> `Limit(2)` WOQL run took 633.16s:
proof generation, duplicate `(1,4),(1,4)` result resolution, envelope round-trip, and
direction/key-priority substitution rejection all passed. Its final legacy row-swap
assertion failed because it swapped identical rows and correctly remained valid. That
assertion is now a real ID-content mutation and compiles (`--no-run` passed), but its
full rerun was deliberately stopped after a bounded two-minute observation. The green
Store changed-content tamper gate covers the underlying proof property; the corrected
full WOQL E2E remains pending, preferably on the Mac or accelerated hardware.

The next checkpoint implements the largest sound `When` subset that does not require
inventing a Prolog-unbound output representation. Leading Prolog is authoritative:
`compile_wf(when(A,B), ((ProgA,ProgB);true))`. Store checkpoint `7005dd1` adds a
recursive exact inner join by exposing the complete matched projection already proven
inside the anti-join argument; the verifier still checks the hidden unmatched-left
compaction before accepting it. Direct multiplicity/empty/key/length tamper passed in
67.50s, and recursive two-key wire/key/content tamper passed in 96.35s.

`woql2` checkpoint `1e9a90f` lowers a bounded `When` after an authenticated outer
relation to `bag_union(inner_join(inner_join(outer,A),B), outer)`, wrapped in a distinct
`/when` query digest. `A` and `B` may be recursive reads but must each expose only one
or two nonnullable variables already in the outer schema. Planner tests cover duplicate
`Or` conditions, nesting in `And`/`Or`/`Optional`/`Not`, new-variable rejection, missing
outer rejection, and nullable-correlation rejection. The focused Linux E2E passed in
163.31s: one successful outer row appeared twice (success plus `true`), one failing row
appeared once (`true` only), envelope round-trip passed, and consequent substitution
rejected.

Store checkpoint `3b86b80` adds an explicit public `ObjectKindFilter` to every leaf
atom and bumps every plan-carrying envelope version. `Node` authenticates the reserved
node datatype selector; `Literal` authenticates its exact `chi_real - node_selector`
complement. Both preserve the original Store object ID. The selection-only composition
also now proves `sel = chi_real` for an otherwise unconstrained `?s ?p ?o`, enabling
predicate-variable kind filters. Focused Linux gates passed for mixed UInt32/Boolean/node
selection plus kind tamper (78.02s), empty literal results plus predicate-variable and
join composition (120.99s), and envelope round-trip/version rejection (34.70s).

woql2 checkpoint `fb251ca` lowers its canonical `Link` and `Data` AST variants to those
explicit filters. Constants remain authenticated directly. A numeric `Data` comparison
uses the exact datatype range proof, which subsumes the broad literal constraint; a
numeric comparison on `Link` rejects closed. All 86 planner tests passed, and the mixed
literal `Data` joined to a node-label triple passed prove/resolve/verify plus wrong-kind
substitution rejection in 80.67s. The leading TerminusDB JSON decoder currently exposes
neither `Link` nor `Data`, so running-node coverage remains blocked on an authoritative
Prolog/JSON shape rather than silently inventing one in the client AST.

Store checkpoint `2bea830` adds recursive exact bag projection. It emits no redundant
polynomial argument: output commitments and columns must be a unique subset/reordering
of the already-verified child, with identical row count and commitment domain. Recursive
envelope v8 uses tag 10. Duplicate-row preservation, wire round-trip, output-content and
index substitution, and duplicate-index rejection passed in 37.56s.

woql2 checkpoint `5550226` extends the bounded leading-Prolog `When` lowering to
`bag_union(project_outer(inner_join(inner_join(outer,A),B)), outer)`. Condition and
consequent may introduce existential locals and the consequent may correlate through a
condition local. Locals are accepted only when enclosing `Select` chains omit them or
`Count` hides the underlying bindings; visible escape and later `Equals`/`Not`/`Optional`/
`When` reuse reject closed. All 86 planner tests and full test compilation passed. The
focused E2E passed in 196.24s: two local successful assignments projected to two equal
outer rows, the true branch added that outer row once more, a failing outer row appeared
once, envelope round-trip passed, and consequent substitution rejected.

Store checkpoint `96546e9` verifies the exact monotone field encoding at minimum, zero,
and maximum for every fixed-width signed and unsigned integer datatype. woql2 checkpoint
`6287001` exposes the corresponding distinct XSD variants through the canonical explicit
decoder and maps each to its identical Store tag. Typed JSON bounds checks, all 89 planner
tests, Asc/Desc `Int8` ordering, and mixed-width bound rejection passed. A real `Int8`
dictionary/range/join prove-and-verify gate passed in 55.53s. `xsd:integer` remains an
explicit bounded-i64 subset; arbitrary precision and Prolog cross-type numeric coercion
are not claimed. Decimal, temporal, string collation, and the AST's conflated float/double
representation remain documented fail-closed boundaries in the coverage matrix.

Leading Prolog compiles canonical `distinct(Vars, Query)` directly to SWI
`solution_sequences:distinct/2`. SWI 10.0.2 uses term-variance equality and streams the
first generated solution for every new witness tuple. Store checkpoint `bea5b24` therefore
adds an exact unordered class-partition statement rather than claiming a sorted or arbitrary
visible result: chained equality compactions prove each one-/two-column tuple's nonempty
class, public tuples are unique, and class cardinalities exactly partition the authenticated
child bag. Recursive wire v9/tag 11, duplicate-heavy one-/two-key inputs, empty input,
omission/invention/duplicate rejection, key substitution, proof tamper, and wire round-trip
passed focused gates.

woql2 checkpoint `3535898` exposes this statement only as
`Count(Distinct([Witness...], Child))`, where output order and the first row's discarded
non-witness bindings are unobservable. Witnesses require exact known nonnullable Store-ID
domains; recursive bag unions and authenticated ordered children compose. All 93 planner
tests and full proof-feature test compilation passed. The duplicate-heavy E2E proved count
2, envelope round-trip, output tamper rejection, and query-substitution rejection in 41.90s;
the empty result proved count 0 in 9.19s. The canonical explicit decoder already matched
the leading Prolog `variables`/`query` Distinct shape and required no serde path or shape
change.

- [x] Exact non-ground dictionary absence, recursively composable.
- [x] Full-row authenticated permutation, preserving multiplicity and tuple correlation.
- [x] Bounded exact scalar ascending/descending `OrderBy` over a complete one-column recursively proved result, preserving duplicate multiplicity.
- [x] Exact `Start`/`Limit` contiguous slices over that authenticated ordered result.
- [x] `Once` over an authenticated ordered child, digest-distinct from `Limit(1)`.
- [x] Two-/three-key complete-row lexicographic order with full-row permutation and root-catalog type/value authentication.
- [x] Arbitrary complete-key priority permutations, with canonical output schema and Store/WOQL digest binding.
- [x] Transcript-bound nondecreasing order and recursive bag-child composition where ties are identical complete rows.
- [ ] Complete the corrected duplicate bag-union WOQL E2E content-tamper rerun; its
  positive path and query-substitution checks passed in 633.16s, while the corrected
  final mutation currently has compile-only coverage.
- [ ] Run the full Store proof suite and woql2 `proof_plan`/`proof_prove` suites when
  time permits; focused semantic tests are the commit gate because full Arkworks runs
  take minutes.
- [ ] Extend general lexicographic/stable-tie ordering and triple navigation.
  `Once` can now be implemented exactly as `Limit(1)` only when its child already has an
  authenticated order; unordered first-solution semantics remain unsupported.
- [x] Implement bounded leading-Prolog `When` exactly when `A` and `B` introduce no
  variables beyond one or two nonnullable outer keys; preserve success multiplicity
  plus one unchanged `true` row without using left outer join.
- [x] Authenticate variable-object `Link` and unconstrained mixed-datatype `Data` as
  exact node/literal dictionary partitions, including predicate variables and joins.
- [x] Extend bounded `When` with existential condition/consequent locals when enclosing
  `Select`/`Count` makes them unobservable; preserve local-match bag multiplicity through
  exact projection back to the incoming outer schema.
- [x] Cover distinct fixed-width XSD integer datatypes end to end without cross-datatype
  coercion; retain `xsd:integer` as a documented bounded-i64 subset.
- [x] Authenticate one-/two-witness projected `Distinct` class counts over recursive
  relations without claiming unauthenticated first-occurrence output order.
- [ ] Expose visible projected `Distinct` only after authenticating SWI first-occurrence
  order, first-row non-witness bindings, and Prolog-unbound variant equality.
- [ ] Extend `When` to observable branch-local variables only after defining an
  authenticated Prolog-unbound output representation and its later-correlation semantics.
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
