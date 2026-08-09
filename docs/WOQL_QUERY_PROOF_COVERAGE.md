# WOQL query-proof coverage and soundness boundary

This matrix classifies the complete `woql2::Query` enum against the intrinsic
PF4 proof stack. “Transparent” never means “discard the syntax”: public query
identity, projection, routing, and result metadata remain bound into the WOQL
envelope digest. An operator is accepted only when its complete result can be
authenticated.

## A. Semantics-preserving compiler wrappers

| Construct | Status | Soundness condition |
|---|---|---|
| `Select` | Implemented | The proof authenticates the complete BGP; delivered rows must exactly match the selected columns and declared order. |
| `Using` | Implemented; running-node integration covered | Collection identity is in the query digest, and the running node proves/verifies against the exact transaction layer selected by the executed `Using` context. Envelope reuse under another collection string rejects. App/domain callers remain responsible for resolving their declared collection to the supplied immutable layer. |
| `From(instance)` | Implemented | Schema and wildcard graph reads reject. |
| `Pin`, `Immediately` | Implemented | They affect execution/optimization timing, not the declarative BGP result. Their query identity remains bound. |
| `Distinct` | Implemented only for the complete BGP schema | Full natural-join assignments over set-valued triples are already unique. Projection/subset distinct needs authenticated deduplication. |
| `Comment`, `True` inside `And` | Implemented in woql2 | They add no bindings and do not change conjunction solutions. This TerminusDB fork's JSON parser does not expose query-level `True`; running-node coverage therefore cannot claim it. |
| `Count` | Implemented | The authenticated complete result length is the count. The ordinary executor must deliver exactly the declared count variable and one nonnegative `u64` value. |
| Fully-ground `Triple`/`Link`/`Data` and all-ground `And` | Implemented in woql2; this TerminusDB fork's JSON parser exposes only `Triple` | Each Store UNIT filter authenticates a zero-arity existence bit; an all-ground conjunction returns their verified boolean AND, preserving duplicate-atom idempotence. If a standalone term is dictionary-absent, the verifier recommits the root-bound clear catalog and authenticates the empty relation. |
| Ground atoms mixed with variable-bearing atoms | Implemented | Variable-bearing atoms form the authenticated relational prefix; verified ground UNIT bits gate its complete result. A false gate forces the same output arity to canonical zero commitments at the verifier-derived result domain and forces result length zero. |
| Dictionary absence in any constant-bearing atom | Implemented recursively | The verifier recommits the root-bound clear predicate and node/value catalogs, authenticates the first absent `(atom index, position)` claim, and requires the complete relation to have zero length with canonical zero commitments at its exact public arity. Empty leaves compose through joins, `Or`, `Not`, and `Optional`; node IRIs and aligned typed literals are covered. |
| Disconnected BGP/cartesian product | Implemented | Each disconnected step joins verifier-derived UNIT columns through the complete binary-join argument, proving every source-index pair exactly once. UNIT relocations bind both sides, the synthetic output key is checked and removed, and schema order remains accumulator variables followed by the new component. |
| Compatible-schema `Or` | Implemented recursively | Every branch is an independently proven read relation against the same trusted root. Branches may themselves be BGP, bag union, `Not`, or `Optional`; nested unions flatten in source order and preserve exact bag multiplicity. Schema reorder mappings, branch order, descriptors, routing, and outer projection/count metadata are digest-bound. Branch-local `Select`/`Count`/`Distinct`, conflicting routing, and incompatible schemas reject. |
| `Not` | Implemented recursively | Global `Not` is authenticated by right-side nonemptiness. Correlated `Not` is an exact one- or two-key left anti-join over any already-proved read relation, including `Or`; top-level `Not` and `Optional` operators fold in source order after the positive relation. Correlation through a nullable outer column rejects because an unbound Prolog variable is bindable and cannot soundly be modeled as SQL null equality between independent children. |
| `Optional` | Implemented recursively | Global and one-/two-key correlated left outer joins accept any already-proved non-ground read relation, including `Or`, and authenticate exactly one null-extended row per unmatched left row. Top-level optionals may mix sequentially with `Not` when every later correlation key is nonnullable. Nullable-key correlation rejects for the same Prolog-unbound reason as `Not`. |
| Predicate variables | Implemented | Predicate IDs are authenticated in their independent dictionary namespace, remain public result columns, compose through joins/operators, and resolve only through the predicate catalog. Cross-namespace variable reuse rejects. |
| `Link` and `Data` | Implemented for constants and variable `Link`; bounded variable `Data` | Constants intrinsically authenticate their kind. A variable `Link` uses the root-bound node/value datatype column to filter exactly the reserved node tag. A variable `Data` is accepted only when a supported numeric comparison pins one exact literal datatype; unconstrained all-literal `Data` still rejects. The current TerminusDB JSON parser does not expose either variant. |
| Exact relational `Equals` | Implemented recursively for already-bound operands | An IsZero selection plus exact LogUp compaction filters a complete authenticated input relation. Node-only, predicate-only, or identical exact-datatype nonnullable columns are accepted; operand order, column indices, exact domains, and canonical constants are digest-bound. Free-variable assignment, nullable operands, unknown mixed node/value columns, predicate↔node/value comparisons, lists/dictionaries, and cross-datatype numeric coercion reject. Constant/constant equality is an authenticated true/false relation gate under the same exact-type rule. |
| Scalar `GroupBy` immediately consumed by `RangeMin`/`RangeMax` | Implemented recursively for the exact bounded form | Accepts exactly one nonnullable Store-ID group key, a variable template pinned to one supported fixed-width order-preserving numeric datatype, a private grouped-list variable, and an adjacent extrema operator consuming that same private variable. Exact equality compactions prove distinct nonempty groups whose cardinalities partition the child bag including duplicates; membership retains an original typed Store value ID; authenticated dictionary range proofs establish domination. Output is only key plus extremum. Composite keys, visible/unconsumed lists, coercive/variable-width datatypes, and arbitrary list extrema reject. Outer `Select`/`Count` and recursive children remain supported. |
| Exact scalar `OrderBy` | Implemented recursively for the bounded ascending form | The canonical proof input is the domain-decoded `Query::OrderBy`, matching Prolog's `OrderBy(ordering, query)` and `OrderTemplate(variable, asc)` structure rather than JSON-LD bytes. Accepts one ascending template naming the complete one-column, nonnullable node/value result of a leaf set-valued BGP, with an exact Store-supported fixed-width order-preserving datatype. A full-row permutation argument preserves multiplicity and tuple identity; authenticated dictionary lookup and datatype completeness bind the scalar ordering values; strict ascending order proves the output. Because the complete leaf row is the key, equal-key ties cannot occur. Outer routing/timing wrappers remain supported. Descending, multiple/partial keys, nullable results, unknown or mixed domains, and recursive/bag-producing children reject closed. |

Write/routing constructs (`Into`, `Add*`, `Delete*`, document mutations) are not
read-query wrappers and remain rejected by query-proof generation.
Recursive plans and envelopes fail closed above depth 64, 4096 relation nodes, or
256 MiB of encoded proof data.

## B. Operators composable from existing proof gadgets

These are tractable without inventing a new polynomial commitment protocol, but
still require authenticated composition and must remain rejected until that
composition is implemented.

| Construct | Existing foundation | Missing composition |
|---|---|---|
| `Start`, `Limit` | authenticated exact scalar ordering, full-row permutation, result-column commitments, compaction, relocation, count identities | First implement the exact prefix/suffix slice over the bounded ordered relation above, binding requested offset/limit, source length, and every selected row. Unordered pagination still has no declarative order to authenticate. |
| General `OrderBy`, `TripleSlice`, `TripleSliceRev`, `TripleNext`, `TriplePrevious` | exact scalar ascending order, strict-sort, range, membership, relocation, multiset/LogUp gadgets | Extend to typed lexicographic key packing, descending direction transforms, and exact prefix/range boundaries. Prolog/SWI ordering is stable for equal keys; partial-key ordering therefore needs an authenticated tie-order contract, not merely permutation plus sortedness. |
| `Once` | complete BGP plus count/existence and authenticated selection | Define and authenticate TerminusDB's first-solution order; without it, stripping `Once` is unsound. |
| `If` (`When` in canonical Prolog/JSON) | existence, union, joins and null-extension | Leading Prolog compiles `when(A,B)` as `((A,B);true)`: successful matches are bag-unioned with an always-present unchanged solution. This is not conventional if/else and not SQL left-outer semantics. The proof plan needs exact outer-binding propagation plus schema-compatible null-extension for the `true` branch. |
| General `GroupBy`, `Collect`, `Sum` | sumcheck, count, sort, membership and commitments | The bounded scalar grouped-extrema form above is implemented without exposing its private list. General live `GroupBy` is SWI `bagof` and `Collect` is `findall`, so both expose ordered Prolog lists. `Sum` consumes a ground list separately and generates an xsd:decimal after datatype-stripping numeric coercion. A versioned authenticated list/generated-scalar result contract is required for those broader forms. |
| General `RangeMin`, `RangeMax` | range selection and strict sort | The immediately consumed exact-datatype grouped form above is implemented. Arbitrary instances consume already-bound lists, fail on empty input, and use coercive `woql_less`; those still require an authenticated list contract and coercion semantics. |
| Additional scalar ranges | dictionary value lookup and range bit proofs | A store-aligned order-preserving encoding and comparison circuit for each datatype. |
| Unconstrained variable-object `Data` | root-bound datatype dictionary column | Authenticate the union of every literal datatype without admitting the reserved node tag. Normalizing it to an unconstrained `Triple` would prove the wrong complete relation. |

## C. Operators needing genuinely new crypto or a new authenticated data model

| Construct family | Why existing gadgets are insufficient |
|---|---|
| Arbitrary `Path` | Requires authenticated transitive closure/regular-path evaluation, not a fixed left-deep relational join. |
| `Eval`, generative/coercive `Equals`, `Typecast`, string/regex/`Like`, collection/set/list operators, `Sequence`, interval/date operators | Need circuits or lookup arguments for their full value semantics and authenticated linkage to output values. |
| `Get` and external resources | Requires an authenticated external-resource commitment and routing model. |
| `ReadDocument` | Requires proof semantics for document reconstruction, field multiplicity, subdocuments and JSON shape over triples. |
| `IsA`, `TypeOf`, `Subsumption` | Require authenticated schema graph selection and transitive schema reasoning. |
| `Size`, `TripleCount` across routed resources | Require authenticated resource routing and the precise database-level statistic definition. |
| `LexicalKey`, `HashKey`, `RandomKey` | Deterministic key functions need value circuits; `RandomKey` is intentionally nondeterministic and cannot be a reproducible proof statement without an explicit randomness transcript. |
| Named-query `Call` | Requires authenticated query-definition lookup, parameter substitution, and recursion termination semantics before compiling the expanded statement. |

All unlisted `Query` variants fall into their corresponding family above and
fail closed today. No query path schedules proof generation, and this matrix
makes no GPU or proof-lifecycle policy choice.

The cross-repository implementation checkpoint, hardware commands, macOS path,
and ordered continuation plan are recorded in
[`WOQL_QUERY_PROOF_GPU_HANDOFF.md`](WOQL_QUERY_PROOF_GPU_HANDOFF.md).
