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
| Standalone fully-ground `Triple`/`Link`/`Data` | Implemented in woql2; this TerminusDB fork's JSON parser exposes only `Triple` | The Store's internal UNIT column authenticates the zero-arity relation: one empty binding when the triple exists, none when it does not. If a term is dictionary-absent, the verifier recommits the root-bound clear catalog and authenticates the resulting empty relation. |
| Constant-object `Link` and `Data` | Implemented in woql2 | The constant's canonical node/literal representation intrinsically authenticates the declared object kind. Variable-object forms reject until object-kind selection is proven; the current TerminusDB JSON parser does not expose either variant. |

Write/routing constructs (`Into`, `Add*`, `Delete*`, document mutations) are not
read-query wrappers and remain rejected by query-proof generation.

## B. Operators composable from existing proof gadgets

These are tractable without inventing a new polynomial commitment protocol, but
still require authenticated composition and must remain rejected until that
composition is implemented.

| Construct | Existing foundation | Missing composition |
|---|---|---|
| `Start`, `Limit` | result-column commitments, compaction, relocation, count identities | Authenticate the exact selected slice. Without an authenticated order, arbitrary BGP pagination is not a declarative set result. |
| `OrderBy`, `TripleSlice`, `TripleSliceRev`, `TripleNext`, `TriplePrevious` | strict-sort, range, membership, relocation, multiset/LogUp gadgets | Prove a permutation of the complete input, lexicographic typed ordering, and exact prefix/range boundaries. Sortedness alone is insufficient. |
| Ground atoms inside a conjunction | complete constant filter, authenticated UNIT relation/result length | Compose the zero-arity existence bit with the other relation without an unauthenticated post-filter. |
| Dictionary non-membership in relational/non-ground atoms | dictionary commitments and standalone-ground catalog openings | Compose authenticated absence with a relation that still exposes variables. Only the zero-arity ground case can currently short-circuit to empty. |
| `Once` | complete BGP plus count/existence and authenticated selection | Define and authenticate TerminusDB's first-solution order; without it, stripping `Once` is unsound. |
| Full-schema `Equals` and variable equality filters | atom equality filter and sumcheck | General cross-atom equality routing beyond natural join and constant unification. |
| `Or` | complete branch proofs, multiset/compaction gadgets | Authenticated bag union with correct overlap multiplicity and compatible schemas. |
| `Not` | selection/boolean-product/compaction and complete branch proofs | Correlated anti-join/non-membership over outer bindings. |
| `Optional` | join, anti-filter, compaction | Left outer join including exactly one null-extended row when the right side is absent. |
| `If` | existence plus union/anti-join | Correlated branch selection with authenticated test success/failure. |
| `GroupBy`, `Collect`, `Sum` | sumcheck, count, sort, membership and commitments | Authenticate grouping partition/permutation, per-group aggregation, and output dictionary/list values. |
| `RangeMin`, `RangeMax` | range selection and strict sort | Authenticate extrema and the empty-input case. |
| Disconnected BGP/cartesian product | binary join and count identities | A complete cartesian-product gadget with multiplicity and output-column linkage. |
| Additional scalar ranges | dictionary value lookup and range bit proofs | A store-aligned order-preserving encoding and comparison circuit for each datatype. |
| Variable-object `Link`/`Data` | canonical dictionary values and result commitments | Authenticate the node-vs-literal object-kind filter. Normalizing either to an unconstrained `Triple` would prove the wrong complete relation. |

## C. Operators needing genuinely new crypto or a new authenticated data model

| Construct family | Why existing gadgets are insufficient |
|---|---|
| Predicate variables | A constant predicate currently selects one authenticated PF4 table. Variable predicates require an authenticated union across all five triple tables plus predicate-ID output columns and dictionary resolution. |
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
