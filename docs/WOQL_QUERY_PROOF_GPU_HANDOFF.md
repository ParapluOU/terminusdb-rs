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
| `modules/terminusdb-store-paraplu` | `9f3901e` | Store wire v17 authenticates immediate zero-/one-/two-node-key grouped-list cardinality and shares canonical tuple grouping with grouped sums. |
| `modules/terminusdb-rs` | `9b242c9` | WOQL planning and verification cover immediate `GroupBy`→`Length`, generated-U64 comparisons, and outer result wrappers. |
| `modules/terminusdb` | `98ec75e2` | Prolog emits `generated_decimal(CanonicalString)` for `TripleCount` and `Sum`; the Rust bridge recompiles descriptors before choosing U64/I64, while owned CPU/Blitzar prover selection remains application-controlled. |

The canonical proof input is `woql2::Query`, not JSON-LD bytes. At the server
boundary parsed JSON is decoded with the schema crate's explicit
`Query::from_json` decoder (not serde-derived AST decoding). Prolog is the
leading definition of WOQL shapes; update that decoder and `woql2` when the
server's accepted structure differs.

The running-node result boundary is likewise descriptor-driven. Plain positive
integers are Store dictionary IDs. Generated integral aggregate outputs use the
explicit lexical `generated_decimal/1` Prolog term, and `null` remains the only
nullable marker. Rust never infers signedness from the lexical sign: the
independently compiled result descriptor selects generated U64 or I64 and then
enforces canonical lexical form and bounds. Count keeps its existing scalar
verification internally, but raw Count input must also carry the explicit
generated marker, including zero.

The fresh debug `terminusdb-dylib` built and loaded on Linux without replacing
the checked-in `src/rust/librust.so`. The focused structural suite passed, and
the live release fixture reached `count_native_verify_complete` after Prolog
execution, foreign parsing, CPU proof generation, and native verification. That
run also established that authoritative Prolog JSON may omit the default
instance graph; the server trust boundary now accepts exactly that equivalence
while retaining unknown-field rejection. Archive/tamper continuation beyond
the decisive Count stage was stopped to keep this checkpoint bounded; the
previous full CPU release-suite result remains the broad regression baseline.

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
cargo test -p terminusdb-woql2 --features proof --test proof_plan \
  generated_decimal_foreign_inputs_are_descriptor_driven_and_canonical -- --exact

cd ../terminusdb/src/rust
cargo test -p terminusdb-query-proof --test explicit_control \
  prolog_omitted_default_graph_is_canonical_but_unknown_fields_still_fail_closed -- --exact
cargo test -p terminusdb-query-proof --lib \
  count_scalar_boundary_is_exhaustive_and_never_reinterprets_generated_domains -- --exact
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

Leading Prolog and its explicit JSON decoder expose all four ordered triple-value
constructs. `TripleSlice`/`TripleSliceRev` enumerate the half-open typed-object range
`[low, high)` in ascending/descending value order. `TripleNext`/`TriplePrevious` require
ground subject and predicate and choose the nearest same-datatype object strictly above/
below an arbitrary typed reference; when the cursor rather than object is ground, Prolog
uses the inverse relation. Store checkpoint `d923e69` fixes child snapshots to merge their
per-layer typed dictionaries into one global value order (and deduplicate reused outer
IDs) instead of concatenating individually sorted layer runs.

The bounded proof lowering accepts default/instance graph, constant subject/predicate,
one fixed-width numeric object variable, and constant same-datatype bounds/reference.
Slices compose exact BGP range selection with exact scalar ascending/descending order;
next/previous additionally apply an exact `Limit(1)`. This proves membership,
completeness, strict cursor boundaries, result order, and the empty endpoint without a
new proof gadget or physical-ID-order assumption. All 96 planner tests pass. Forward and
reverse prove/resolve/verify plus direction-substitution rejection passed in 184.22s;
next, previous, and empty end-cursor proving passed in 162.06s. These nested tests require
`RUST_MIN_STACK=16777216` on this Linux host; the default Rust test-thread stack overflow
is a recorded CPU/stack-pressure optimization target, not a verifier relaxation.

The fully-ground continuation uses exact zero-column projection rather than exposing a
synthetic value. Store checkpoint `a9fddb5` permits an empty exact `BagProjection` from a
non-ground child and derives the canonical hidden UNIT column (one on each real row,
zero on padding) from the authenticated child length/domain. The verifier independently
recomputes its content and commitment; rows, order, domain, and multiplicity remain
unchanged. Direct positive, empty dictionary-absence, wire, and UNIT-content tamper
coverage passed in 35.43s.

Ground slices prove the ranged ordered S/P scan, exact equality to the supplied object,
then project the hidden match to UNIT. Ground next/previous prove the strict ordered scan
and `Limit(1)` before equality, so a greater-but-non-nearest candidate cannot verify.
Fixed-width same-datatype guards remain unchanged. The focused ground slice run proved
presence, exclusive-high rejection, a dictionary-present value missing the chosen S/P
triple, dictionary absence, and forward/reverse query substitution in 322.94s with
`RUST_MIN_STACK=33554432`. Ground immediate-next success, non-nearest rejection, and
next/previous query substitution passed in 150.28s.

Leading Prolog's explicit-graph `TripleCount(Resource, Count)` reads the selected
graph object's logical visible triple count and returns a nonnegative `xsd:decimal`.
Store checkpoint `43e0757` adds recursive wire v10 `LayerTripleCount`: its sole field
is verifier-derived from the trusted root's committed `materialized_count`, including
child additions/removals. A distinct `GeneratedDecimalU64` descriptor prevents that
field—including zero—from being resolved or accepted as a Store dictionary ID.

The woql2 checkpoint accepts only a resource ending in `/instance` and a variable
output, preserving the branch collection in query identity. Graphless database counts,
schema graphs, other resources, and constant-output unification reject closed. `Size`
remains unsupported because leading Prolog measures physical layer-file bytes outside
the logical root commitment. Focused gates passed: three planner tests, the canonical
non-serde domain-decoder test, and two E2E tests covering empty and child-layer counts,
generated executor values, portable envelope round-trip, wrong resource/root, and a
forged/recommitted result (10.90s for the E2E pair).

Store checkpoint `c5389a6` adds recursive wire v11/tag 13
`GeneratedU64Filter`. It accepts Eq/Lt/Le/Gt/Ge, a `u64` constant, operand order,
and one generated-decimal child column. Since the child values are already public and
authenticated, proving and verification deterministically select the complete matching
rows and recommit them; no Store dictionary membership or additional polynomial proof is
used. The focused Store gate covered every operator, zero and `u64::MAX` constants,
operand order, wire round-trip, operator substitution, and forged/recommitted output in
17.69s.

The woql2 lowering recognizes a sequential explicit-instance `TripleCount` followed by
`Equals`, `Greater`, `Gte`, `Less`, or `Lte` on that bound count variable. Constants must
be `xsd:decimal`, nonnegative, integral, non-scientific in the retained Decimal form, and
within `u64`; the planner does not coerce Store integer tags, fractions, negatives, or
other datatypes. Six focused planner tests passed. Two E2E tests covered true/false
relations for all comparison families, operand reversal, `u64::MAX`, outer `Count`,
envelopes, and operator/order/constant substitution in 14.89s.

Leading Prolog lowers `GroupBy` to SWI `bagof`: an empty child yields no group, duplicate
template solutions remain duplicated, each bag retains generator order, and groups follow
standard term order of their free keys. Immediate `Sum` makes the private bag order
unobservable; its marshalling strips typed literals before `sum_list` and wraps the result
as `xsd:decimal`. The implemented subset requires one expanded node-IRI key and an exact
UInt8/UInt16/UInt32 template. Store checkpoint `758d94f` adds recursive v12/tag 14
`GroupSum`; fix `69f28cd` sorts groups by authenticated canonical node bytes rather than
Store ID, since child-layer IDs append and need not be lexical. A base-`b`/child-`a`
fixture proves visible `a,b` order despite the later `a` ID.

The verifier resolves every child value ID through the root-recommitted catalog, checks
the exact unsigned datatype and lexical width, preserves duplicate multiplicity, and
derives each checked `u64` total. The authenticated row count multiplied by the datatype
maximum must fit `u64`, which also establishes no BN254 field wrap. Empty input emits no
groups. The Store focused gate passed in 14.52s. Two planner tests cover all three accepted
datatypes plus list/order/key/type rejection; the canonical non-serde decoder test passed.
Two WOQL E2E tests passed in 138.52s, covering duplicate 2+2, zero, `u32::MAX`, child-stack
key order, generated-decimal resolution, mixed executor values, outer Count, empty input,
envelope round-trip, forged/recommitted sum, and `Sum`/`RangeMax` substitution.

Store checkpoint `0f7f52d` extends the same statement and recursive wire to v13 with an
optional key. Empty `group_by` follows zero-free-variable `bagof`: all duplicate template
solutions form one private bag when the child is nonempty, while an empty child fails and
therefore produces no aggregate row. The result is one `GeneratedDecimalU64` column with
no synthetic UNIT/key. The same row-count × UInt8/16/32 maximum bound proves the total
fits `u64` and the field. The combined keyed/global Store gate passed in 18.60s, including
`u32::MAX + 4`, zero, empty input, and a forged/recommitted global total.

woql2 binds keyed/global arity into its relation and query digest. The three planner gates
passed, including visible global output and outer Count. The canonical non-serde decoder
round-trips an empty `group_by` array. The nonempty global E2E proved generated decimal 5,
executor matching, envelope round-trip, keyed/global substitution rejection, and tamper in
89.07s; the empty global outer-Count gate proved zero rows/groups in 35.45s.

Store checkpoint `7428496` extends recursive `GroupSum` to v14 and exact UInt64
templates. This is sound without the earlier pessimistic row-count × datatype-maximum
bound: every public child value ID is resolved against the validated root-bound catalog,
the verifier requires the exact datatype and canonical eight-byte lexical value, and both
keyed and global derivation use checked addition over those actual authenticated values.
Totals through `u64::MAX` are below the BN254 scalar modulus and therefore have no field
wrap ambiguity; only actual overflow rejects. The 14.84s focused Store gate covered a
partitioned exact maximum, near-maximum, generated-result tamper, datatype substitution,
and overflow rejection in both recursive proving and the verifier derivation path.

woql2 now maps the canonical decoder's existing `xsd:unsignedLong` form to UInt64 for
immediate grouped sums. The proof verifier independently resolves the same root-bound
public values and checked-adds them. Planner/check gates passed, and the 83.90s E2E gate
proved a partitioned `u64::MAX`, matched the generated executor value, round-tripped the
portable envelope, and rejected authenticated `u64::MAX + 1` input during proving.

Store checkpoint `7b5e889` adds exact Int8/Int16/Int32/Int64 grouped sums and recursive
wire v15. Signed results use a separate `GeneratedDecimalI64` descriptor/value/wire tag;
their canonical field is `Fr(v)` for nonnegative values and `-Fr(abs(v))` for negatives.
Resolution checks that exact canonical image before returning a signed decimal, and the
descriptor—not field zero—controls nullability, so generated signed zero is nonnull.
Store signed lexical bytes are order-preserving sign-bit-flipped encodings and are decoded
accordingly only after exact root-catalog datatype/width authentication. Accumulation uses
i128 so Prolog-valid cancellation is not rejected by an intermediate i64 overflow; the
four-million-item envelope cap gives magnitude below 2^85, and final conversion requires
the result to fit i64. The 18.20s Store gate covers duplicates, zero, negative/positive
cancellation, i64 minimum/maximum inputs, child-stack key order, wire round-trip, sign and
domain tamper, exact datatype, and positive/negative actual overflow. The unsigned
regression gate remained green in 14.84s.

woql2 accepts canonical `byte`, `short`, `int`, `long`, and its bounded-i64 `integer`
forms without cross-datatype coercion, and returns `GeneratedDecimalI64` through result
resolution and `ExecutedResultValue`. Planner gates passed. The 136.91s E2E gate proves
visible signed keyed sums over a child stack, including duplicates, zero, i64 boundaries,
negative executor output, envelope round-trip, sign/recommit tamper, descriptor-domain
substitution, and actual global overflow rejection.

Store checkpoint `4326f27` adds the recursive `GeneratedI64Filter` proof kind. Shape
validation requires a nonnullable `GeneratedDecimalI64` child descriptor, so neither an
unsigned generated value nor a Store ID can be reinterpreted as signed. The prover and
verifier decode only the canonical signed field image, derive the complete Eq/Lt/Le/Gt/Ge
row subset, and recompute every output commitment. The separate proof tag plus the woql2
relation/query digest bind domain, operator, operand order, constant, and child plan. The
focused Store gate passed in 57.49s, covering -1/0 rows, both operand orders, every
operator, i64 minimum/maximum constants, wire round-trip, unsigned-domain substitution,
wrong operator/order, and sign/content/recommit tamper.

woql2 now recognizes comparisons following immediate keyed or global `GroupBy`→`Sum`.
The aggregate datatype descriptor chooses U64 or I64; constants must be exact integral
`xsd:decimal` values in that domain. Fractional, retained scientific-exponent, overflow,
wrong-variable, cross-domain, and Store integer-tag constants reject closed. Outer
`Select`/`Count` remain supported, and the existing sequential planner composes bounded
`When` over the aggregate prefix. The focused planner test passed in 0.00s. The reduced
signed WOQL E2E passed in 156.46s, proving `< 0`, resolving the negative executor value,
round-tripping the envelope, and rejecting operand/query substitution, descriptor-domain
substitution, and signed-result tamper.

Store wire version 16 and woql2 now extend the same immediate aggregate to exactly two
distinct nonnullable node keys. The relation plan binds the ordered child-column mapping,
and both prover and verifier resolve each key through the root-authenticated node catalog
before sorting canonical expanded-IRI byte tuples. This matches SWI `group_by/4`: the
declared key list determines lexicographic priority, independently of child schema order or
layer-local numeric IDs. Output retains the declared keys in that order followed by the
generated U64/I64 sum; duplicate value rows retain bag multiplicity. Empty input produces
zero rows with the exact three-column schema. Duplicate, out-of-range, value-aliasing,
non-node, and three-key plans reject closed, while swapping the declared key order rejects
an existing envelope. The focused Store positive gate passed in 140.11s; the signed WOQL
E2E passed in 309.72s and covered lexical order, negative filtering, envelope round-trip,
key-order substitution, and signed result/recommit tamper. A direct SWI 10.0.2 probe also
confirmed that `[K1,K2]` and `[K2,K1]` produce the corresponding distinct tuple-priority
orders.

The canonical list-consumer audit selected immediate `GroupBy`→`Length` as the next exact
form. `Length` is present in the canonical woql2 AST and leading Prolog decoder with
`list`/`length` fields; Prolog calls marshalled `length/2`, whose unbound numeric output is
reported as `xsd:decimal`. Because only bag cardinality survives, neither list order nor
template contents are observable. Store wire version 17 reuses the canonical node-tuple
grouping shared with grouped sum, counts authenticated child row indices including
duplicates, and emits `GeneratedDecimalU64`. Global empty `bagof` still yields no row;
`Collect`→`Length` is deliberately deferred because `findall` instead yields one zero row
on empty input. `Member`, slices, list-to-set, and set operators expose elements, ordering,
or equality semantics and remain behind the authenticated-list/value-contract boundary.
A direct SWI 10.0.2 probe produced `[[a,2],[z,1]]` for duplicate keyed solutions and `[]`
for a global empty child. The post-refactor Store proof gate passed in 130.88s. The focused
planner and decoder gates passed in 0.00s, and the filtered two-key WOQL E2E passed in
311.09s. Generalizing woql2's generated-U64 verifier during that E2E also removed its stale
one-row TripleCount assumption; the original TripleCount comparison regression remained
green in 12.38s.

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
- [ ] Extend general lexicographic/stable-tie ordering and triple navigation beyond the
  exact constant-S/P, one fixed-width numeric object-column subset.
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
- [x] Authenticate bounded `TripleSlice`/`TripleSliceRev` and both binding modes of
  `TripleNext`/`TriplePrevious` for constant S/P and one fixed-width numeric object column,
  including global child-layer order, half-open/strict boundaries, reverse order, limit
  one, empty endpoints, and direction substitution.
- [x] Authenticate fully-ground slices and next/previous adjacency as exact zero-column
  UNIT/empty relations, including equality after nearest-value `Limit(1)`, boundary,
  missing-triple, dictionary-absence, datatype, and query-direction guards.
- [x] Authenticate explicit instance-graph `TripleCount` directly from the trusted
  root's logical visible count, with a generated-decimal result domain, child stacks,
  envelope/executor integration, and resource/root/tamper guards.
- [x] Authenticate bounded Eq/Gt/Gte/Lt/Lte filters over the generated TripleCount
  decimal against exact integral decimal constants, including operand order and
  zero/maximum boundaries, without conflating generated scalars with Store IDs.
- [x] Authenticate immediate one-node-key `GroupBy`→`Sum` for UInt8/UInt16/UInt32/UInt64,
  preserving duplicate multiplicity, canonical SWI-compatible node group order, checked
  no-wrap arithmetic, and generated-decimal result typing without exposing the list.
- [x] Extend immediate `GroupBy`→`Sum` to the zero-key/global form: one sum row iff the
  authenticated child is nonempty, no row when empty, and no synthetic key/list leakage.
- [x] Extend immediate signed/unsigned `GroupBy`→`Sum` to two node keys, binding declared
  key priority independently of child schema order, sorting canonical expanded-IRI tuples,
  preserving duplicates, and rejecting key-order, mapping, arity, and result tamper.
- [x] Authenticate immediate zero-/one-/two-node-key `GroupBy`→`Length` as exact bag
  cardinality with generated nonnegative decimal output and reusable comparisons/Count,
  without exposing private list order or elements.
- [x] Replace the pessimistic grouped-sum datatype-maximum bound with exact authenticated
  value accumulation, accepting UInt64 totals through `u64::MAX` and rejecting actual
  overflow without field-modulus ambiguity.
- [x] Add exact signed Int8/16/32/64 grouped sums with a distinct generated-I64 domain,
  canonical signed field encoding, cancellation-safe accumulation, and signed executor
  resolution.
- [x] Add bounded Eq/Lt/Le/Gt/Ge comparisons over signed and unsigned generated grouped
  sums, with exact decimal constants, operand order, domain-separated plans, outer Count,
  bounded When composition, envelope/executor integration, and tamper rejection.
- [ ] Extend database-level or schema `TripleCount` only after the proof input binds all
  selected graph roots and exactly matches leading Prolog's multi-graph aggregation.
- [ ] Keep physical `Size` rejected unless a separately authenticated filesystem/storage
  accounting commitment is deliberately introduced.
- [ ] Extend ordered triple values to variable S/P or visible full triples only after
  authenticating complete tie order; add variable-width string collation, schema routing,
  and broader datatype families separately.
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
