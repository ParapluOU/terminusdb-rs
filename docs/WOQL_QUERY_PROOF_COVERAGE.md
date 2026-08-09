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
| Ground atoms mixed with variable-bearing atoms | Implemented | Variable-bearing atoms form the authenticated relational prefix; verified ground UNIT bits gate its complete result. A false gate forces the same output arity to canonical zero commitments at the verifier-derived result domain and forces result length zero. Dictionary-absent mixed atoms still fail closed. |
| Disconnected BGP/cartesian product | Implemented | Each disconnected step joins verifier-derived UNIT columns through the complete binary-join argument, proving every source-index pair exactly once. UNIT relocations bind both sides, the synthetic output key is checked and removed, and schema order remains accumulator variables followed by the new component. |
| Compatible-schema `Or` | Implemented recursively | Every branch is an independently proven read relation against the same trusted root. Branches may themselves be BGP, bag union, `Not`, or `Optional`; nested unions flatten in source order and preserve exact bag multiplicity. Schema reorder mappings, branch order, descriptors, routing, and outer projection/count metadata are digest-bound. Branch-local `Select`/`Count`/`Distinct`, conflicting routing, and incompatible schemas reject. |
| `Not` | Implemented recursively | Global `Not` is authenticated by right-side nonemptiness. Correlated `Not` is an exact one- or two-key left anti-join over any already-proved read relation, including `Or`; top-level `Not` and `Optional` operators fold in source order after the positive relation. Correlation through a nullable outer column rejects because an unbound Prolog variable is bindable and cannot soundly be modeled as SQL null equality between independent children. |
| `Optional` | Implemented recursively | Global and one-/two-key correlated left outer joins accept any already-proved non-ground read relation, including `Or`, and authenticate exactly one null-extended row per unmatched left row. Top-level optionals may mix sequentially with `Not` when every later correlation key is nonnullable. Nullable-key correlation rejects for the same Prolog-unbound reason as `Not`. |
| Predicate variables | Implemented | Predicate IDs are authenticated in their independent dictionary namespace, remain public result columns, compose through joins/operators, and resolve only through the predicate catalog. Cross-namespace variable reuse rejects. |
| Constant-object `Link` and `Data` | Implemented in woql2 | The constant's canonical node/literal representation intrinsically authenticates the declared object kind. Variable-object forms reject until object-kind selection is proven; the current TerminusDB JSON parser does not expose either variant. |

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
| `Start`, `Limit` | result-column commitments, compaction, relocation, count identities | Authenticate the exact selected slice. Without an authenticated order, arbitrary BGP pagination is not a declarative set result. |
| `OrderBy`, `TripleSlice`, `TripleSliceRev`, `TripleNext`, `TriplePrevious` | strict-sort, range, membership, relocation, multiset/LogUp gadgets | Prove a permutation of the complete input, lexicographic typed ordering, and exact prefix/range boundaries. Sortedness alone is insufficient. |
| Dictionary non-membership in relational/non-ground atoms | dictionary commitments and standalone-ground catalog openings | Compose authenticated absence with a relation that still exposes variables. Only the zero-arity ground case can currently short-circuit to empty. |
| `Once` | complete BGP plus count/existence and authenticated selection | Define and authenticate TerminusDB's first-solution order; without it, stripping `Once` is unsound. |
| Full-schema `Equals` and variable equality filters | atom equality filter and sumcheck | General cross-atom equality routing beyond natural join and constant unification. |
| `If` | existence plus union/anti-join | Correlated branch selection with authenticated test success/failure. |
| `GroupBy`, `Collect`, `Sum` | sumcheck, count, sort, membership and commitments | Authenticate grouping partition/permutation, per-group aggregation, and output dictionary/list values. |
| `RangeMin`, `RangeMax` | range selection and strict sort | Authenticate extrema and the empty-input case. |
| Additional scalar ranges | dictionary value lookup and range bit proofs | A store-aligned order-preserving encoding and comparison circuit for each datatype. |
| Variable-object `Link`/`Data` | canonical dictionary values and result commitments | Authenticate the node-vs-literal object-kind filter. Normalizing either to an unconstrained `Triple` would prove the wrong complete relation. |

## C. Operators needing genuinely new crypto or a new authenticated data model

| Construct family | Why existing gadgets are insufficient |
|---|---|
| Arbitrary `Path` | Requires authenticated transitive closure/regular-path evaluation, not a fixed left-deep relational join. |
| `Eval`, general `Equals`, `Typecast`, string/regex/`Like`, collection/set/list operators, `Sequence`, interval/date operators | Need circuits or lookup arguments for their full value semantics and authenticated linkage to output values. |
| `Get` and external resources | Requires an authenticated external-resource commitment and routing model. |
| `ReadDocument` | Requires proof semantics for document reconstruction, field multiplicity, subdocuments and JSON shape over triples. |
| `IsA`, `TypeOf`, `Subsumption` | Require authenticated schema graph selection and transitive schema reasoning. |
| `Size`, `TripleCount` across routed resources | Require authenticated resource routing and the precise database-level statistic definition. |
| `LexicalKey`, `HashKey`, `RandomKey` | Deterministic key functions need value circuits; `RandomKey` is intentionally nondeterministic and cannot be a reproducible proof statement without an explicit randomness transcript. |
| Named-query `Call` | Requires authenticated query-definition lookup, parameter substitution, and recursion termination semantics before compiling the expanded statement. |

All unlisted `Query` variants fall into their corresponding family above and
fail closed today. No query path schedules proof generation, and this matrix
makes no GPU or proof-lifecycle policy choice.
