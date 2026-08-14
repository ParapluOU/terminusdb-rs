//! Core proof data types — the compiler's output vocabulary.
//!
//! WOQL constructs are lowered (in the sibling submodules) into the types defined here.
//! [`CompiledBgp`] is a compiled query: the one authenticated relation plus read-side
//! routing metadata (projection, collection, count binding, full-row distinctness).
//! [`CompiledRelation`] / [`CompiledRelationStatement`] are the recursive plan tree — a
//! base BGP scan or one of the relational operators layered over it (filters, joins,
//! aggregates, ordering, distinct, unions, the value/eval/string layers, paths).
//! [`EvalExpr`] / [`DecimalExpr`] / [`ConcatElement`] / [`StringNumericTarget`] /
//! [`CalendarIntFn`] / [`CalendarDateFn`] are the small expression trees the Eval, string
//! and calendar forms compile to. The result-readout types ([`ProvedBgp`],
//! [`ExecutedResultValue`], [`ExecutedResultInput`], [`ProvedQueryStatement`],
//! [`VerifiedBgpEnvelope`]) are what a caller uses to read a proven query's answer columns.
//! These are plain data; the proving / verifying / lowering logic lives in the siblings.

use super::*;

/// A compiled, variable-blind BGP plan plus the metadata a caller needs to read the
/// proof's result columns.
#[derive(Clone, Debug)]
pub struct CompiledBgp {
    /// The one authoritative recursive authenticated relation.
    pub relation: CompiledRelation,
    /// If a `Select` wrapped the BGP, the selected variables' column indices into
    /// `relation.schema`. The proof still proves the full relation; projection is which output
    /// columns the caller reads (sound — a projection of the true answer).
    pub projection: Option<Vec<usize>>,
    /// Query-routing metadata retained from outer WOQL wrappers. The cryptographic
    /// statement is anchored to the layer root supplied by the caller; integrations
    /// must use this collection when resolving that layer.
    pub collection: Option<String>,
    /// An outer WOQL `Count` binding. The proven BGP result length is the count, so no
    /// additional cryptographic argument is required.
    pub count_variable: Option<String>,
    /// True when a WOQL `Distinct` was proven redundant because it names the complete
    /// BGP schema. Full natural-join assignments over set-valued triple relations are
    /// already unique; subset/projection distinctness still requires a dedup argument.
    pub full_row_distinct: bool,
}

#[derive(Clone, Debug)]
pub struct CompiledRelation {
    pub schema: Vec<String>,
    /// Exact executor value domains known for each schema column. `None` means the
    /// node/value dictionary column may contain both nodes and differently typed values.
    pub domains: Vec<Option<EqualityValueDomain>>,
    pub statement: CompiledRelationStatement,
}

/// SCHEMA-GRAPH FOUNDATION — the per-BGP-leaf graph selector.
///
/// A query-proof database commits TWO separate triple graphs (see
/// [`terminus_store::proof::commitment::database_root`]): the INSTANCE graph and the
/// SCHEMA graph, each as its own `LayerCommitment` with its own `commitment_root`. A
/// triple-reading plan node (today: a transitive-closure edge) carries this selector to
/// say WHICH committed graph its columns and dictionaries belong to.
///
/// `Instance` is the default and matches every pre-existing plan byte-for-byte: it is
/// folded into no digest, engages no schema commitment, and anchors to the instance root
/// exactly as before. `Schema` is engaged ONLY by a query that reads the schema graph
/// (Subsumption); it routes the node to the schema commitment and folds a domain tag into
/// the query digest so the choice is authenticated and cannot be substituted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ProofGraph {
    /// The instance triple graph (default; byte-identical to all prior behaviour).
    #[default]
    Instance,
    /// The schema triple graph (`sys:inherits` etc.), a separate committed layer.
    Schema,
}

#[derive(Clone, Debug)]
pub enum CompiledRelationStatement {
    Bgp(LayerBgpPlan),
    LayerTripleCount,
    GeneratedU64Filter {
        input: Box<CompiledRelation>,
        column: usize,
        op: GeneratedU64ComparisonOp,
        constant: u64,
        constant_on_left: bool,
    },
    GeneratedI64Filter {
        input: Box<CompiledRelation>,
        column: usize,
        op: GeneratedU64ComparisonOp,
        constant: i64,
        constant_on_left: bool,
    },
    BagUnion {
        branches: Vec<CompiledRelation>,
        /// Public output column -> child relation column, per ordered branch.
        mappings: Vec<Vec<usize>>,
    },
    BagProjection {
        input: Box<CompiledRelation>,
        indices: Vec<usize>,
    },
    /// Exact unordered distinct projection. The initial bounded WOQL exposure is only
    /// beneath `Count`, because SWI's first-occurrence output order is not authenticated.
    Distinct {
        input: Box<CompiledRelation>,
        key_columns: Vec<usize>,
    },
    /// Leading-Prolog `when(A,B)` lowered to authenticated joins plus bag union.
    /// This wrapper keeps canonical query identity distinct from an explicit `Or`.
    When {
        expanded: Box<CompiledRelation>,
    },
    LeftAnti {
        left: Box<CompiledRelation>,
        right: Box<CompiledRelation>,
        key_pairs: Vec<(usize, usize)>,
    },
    InnerJoin {
        left: Box<CompiledRelation>,
        right: Box<CompiledRelation>,
        key_pairs: Vec<(usize, usize)>,
    },
    /// CROSS-DICTIONARY two-root hash join (woql2 mirror of the store
    /// [`terminus_store::proof::envelope::RelationProofPlanKind::CrossRootHashJoin`]). The ONE node
    /// that reads BOTH committed graphs: `left` reads the SCHEMA graph (a class hash SET — the downward
    /// `sys:inherits` subsumption closure of `C`), `right` reads the INSTANCE graph
    /// (`Triple(?x, rdf:type, ?class)`, `[?x, class]`). Each side's class-id column is authenticated to
    /// its `hash_object(IRI)` against its OWN root; a full-field LogUp SEMI-JOIN keeps instance rows
    /// whose class-hash is a member of the closure hash set. Emitted ONLY by [`plan_isa`]; routed via
    /// the two-layer store `prove_cross_root`/`verify_cross_root`. `left` is a single class column
    /// (`left_key_col == 0`); output = `right`'s `?x` column (instance namespace).
    CrossRootHashJoin {
        left: Box<CompiledRelation>,
        right: Box<CompiledRelation>,
        left_key_col: usize,
        right_key_col: usize,
    },
    LeftOuter {
        left: Box<CompiledRelation>,
        right: Box<CompiledRelation>,
        key_pairs: Vec<(usize, usize)>,
    },
    Equality {
        input: Box<CompiledRelation>,
        constraint: RelationEqualityPlan,
    },
    /// EDGE-SIMPLE filter for a finite single-predicate repeated-predicate path: drops the
    /// rows in which some hop-pair reuses a stored edge (the ONLY over-count of a
    /// conjunctive bag-join versus Prolog's edge-simple `Open_Set` semantics), keeping
    /// every node-repeating solution (`a->b->a`). `pairs` enumerates the `C(n,2)` hop-pairs
    /// of an `n`-hop chain, referencing the join's node columns / bound endpoint constants;
    /// the schema passes through unchanged (endpoint projection is a separate
    /// `BagProjection`). See the store [`terminus_store::proof::disequality`] gadget.
    TupleDisequality {
        input: Box<CompiledRelation>,
        pairs: Vec<TupleDisequalityPairPlan>,
    },
    ExactScalarOrder {
        input: Box<CompiledRelation>,
        datatype: CanonicalDatatype,
        direction: OrderDirection,
    },
    ExactMultiOrder {
        input: Box<CompiledRelation>,
        /// Priority-ordered mapping into the input's canonical schema columns.
        key_indices: Vec<usize>,
        /// Datatypes remain in canonical schema order.
        datatypes: Vec<CanonicalDatatype>,
        /// Directions are parallel to `key_indices`.
        directions: Vec<OrderDirection>,
    },
    ExactOrderedSlice {
        input: Box<CompiledRelation>,
        offset: u64,
        limit: Option<u64>,
        /// Distinguishes canonical Prolog `once/1` from explicit `limit(1, ...)` in
        /// the WOQL query digest while reusing the identical Store slice statement.
        once: bool,
    },
    GroupExtrema {
        input: Box<CompiledRelation>,
        key_columns: Vec<usize>,
        value_column: usize,
        datatype: CanonicalDatatype,
        kind: GroupExtremaKind,
    },
    /// Bounded transitive closure (`star`/`plus`) of a SINGLE predicate from a BOUND endpoint,
    /// delivering the reachable node SET. This is the woql2 mirror of the store
    /// [`terminus_store::proof::envelope::RelationProofPlanKind::TransitiveClosure`] composite node;
    /// it is emitted ONLY under an endpoint `Distinct` (set semantics), because the leading-Prolog
    /// path evaluator returns an edge-simple BAG of endpoints that equals the reachable SET only once a
    /// surrounding `Distinct`/`Count` collapses the bag. See [`plan_transitive_closure_path`].
    TransitiveClosure {
        /// The edge relation `E = Triple(?s, p, ?o)` (a single-atom 2-column node/value BGP).
        edge: LayerBgpPlan,
        /// The bound endpoint node the traversal starts from (`a` for a bound SOURCE, `b` for a bound
        /// TARGET reached by traversing the inverse edge).
        seed: CanonicalObject,
        /// `true` = `star` (reflexive: the seed itself is in the set); `false` = `plus`.
        reflexive: bool,
        /// `false` walks `subject -> object` from a bound source; `true` walks the inverse edge
        /// `object -> subject` (a bound target, or an inverse-predicate closure).
        inverted: bool,
        /// SCHEMA-GRAPH FOUNDATION — which committed triple graph this closure's edge `E`, seed
        /// dictionary lookup, and delivered-node dictionary rendering read against. `Instance` (the
        /// universal default for every pre-existing instance-graph path closure) selects the instance
        /// `LayerCommitment`; `Schema` selects the schema `LayerCommitment` (its own dictionaries /
        /// columns / `commitment_root`), enabling the `sys:inherits` subsumption closure. The verifier
        /// RECONSTRUCTS this selector from the trusted compiled query and it is bound into
        /// `relation_query_digest` (additively — `Schema` folds a domain tag, `Instance` folds NOTHING,
        /// keeping every instance-only digest byte-identical), so a prover cannot silently read the
        /// wrong graph. See [`ProofGraph`] and [`relation_graph`].
        graph: ProofGraph,
    },
    /// Authenticated ORDERED walk of one stored `rdf:List`: given a BOUND subject node `S` and a
    /// bound list-valued predicate `P`, deliver the list's elements IN ORDER as one nonnullable
    /// node/value column (ReadDocument increment 2a). This is the woql2 mirror of the store
    /// [`terminus_store::proof::envelope::RelationProofPlanKind::OrderedListWalk`] composite
    /// leaf; ALL of the proving (the cons-chain topology walk, the positional successor, the
    /// `rdf:first`/`rdf:rest`/`rdf:nil` handling) lives in the store — the compiler only routes to
    /// it, folds the digest, and passes the proof through, exactly as it does for
    /// [`Self::TransitiveClosure`].
    ///
    /// Reached ONLY by direct construction in this increment: no WOQL construct lowers to it yet
    /// (`Query::ReadDocument` wiring is increment 2b). It carries no `CompiledRelation` child — it
    /// is a leaf, like `TransitiveClosure`.
    OrderedListWalk {
        /// The bound list-owning subject; must be a node IRI (a literal can never own a list edge).
        /// Matches the store plan's `subject` field ([`CanonicalObject::Node`]).
        subject: CanonicalObject,
        /// The bound list-valued predicate IRI bytes (the `P` of `t(S,P,head)`). Matches the store
        /// plan's `predicate` field.
        predicate: Vec<u8>,
        /// SCHEMA-GRAPH FOUNDATION — which committed graph this walk reads. Always `Instance` in
        /// this increment (a stored `rdf:List` lives in the instance graph); carried so the
        /// exhaustive graph/digest machinery treats it uniformly with `TransitiveClosure`. The
        /// selector is bound ADDITIVELY into `relation_query_digest` (`Instance` folds nothing) and
        /// is NOT forwarded into the store plan kind (the store recursion is graph-blind — the
        /// prove/verify ENTRY picks the matching `LayerCommitment`). See [`relation_graph`].
        graph: ProofGraph,
    },
    /// Authenticated ORDERED walk of one stored 1-D DENSE `sys:Array`: given a BOUND subject node `S`
    /// and a bound array-valued predicate `P`, deliver the array's elements IN ASCENDING `sys:index`
    /// ORDER as one nonnullable node/value column (ReadDocument increment 4). This is the woql2 mirror
    /// of the store
    /// [`terminus_store::proof::envelope::RelationProofPlanKind::OrderedArrayWalk`] composite
    /// leaf; ALL of the proving (the complete cell-set BGP, the `sys:index`/`sys:value`/`rdf:type`
    /// functional binds, the `sys:Array` type filter, the authenticated dictionary index lookup, and
    /// the density+order permutation) lives in the store — the compiler only routes to it, folds the
    /// digest, and passes the proof through, EXACTLY as it does for [`Self::OrderedListWalk`]. (An array
    /// is a FLAT bag of numbered cells rather than a cons chain, but from the compiler's vantage the two
    /// leaves are indistinguishable: a bound `(subject, predicate)` in, one ordered element column out.)
    ///
    /// Reached ONLY by direct construction (from the `ReadDocument` bundle's `ordered_array_walk_bgp`);
    /// no WOQL relation surface lowers to it. It carries no `CompiledRelation` child — it is a leaf,
    /// like `OrderedListWalk`.
    OrderedArrayWalk {
        /// The bound array-owning subject; must be a node IRI (a literal can never own an array edge).
        /// Matches the store plan's `subject` field ([`CanonicalObject::Node`]).
        subject: CanonicalObject,
        /// The bound array-valued predicate IRI bytes (the `P` of `t(S,P,cell)`). Matches the store
        /// plan's `predicate` field.
        predicate: Vec<u8>,
        /// SCHEMA-GRAPH FOUNDATION — which committed graph this walk reads. Always `Instance` in this
        /// increment (a stored `sys:Array` lives in the instance graph); carried so the exhaustive
        /// graph/digest machinery treats it uniformly with `OrderedListWalk`. The selector is bound
        /// ADDITIVELY into `relation_query_digest` (`Instance` folds nothing) and is NOT forwarded into
        /// the store plan kind (the store recursion is graph-blind — the prove/verify ENTRY picks the
        /// matching `LayerCommitment`). See [`relation_graph`].
        graph: ProofGraph,
    },
    GroupSum {
        input: Box<CompiledRelation>,
        key_columns: Vec<usize>,
        value_column: usize,
        datatype: CanonicalDatatype,
    },
    GroupCount {
        input: Box<CompiledRelation>,
        key_columns: Vec<usize>,
    },
    CollectionCount {
        input: Box<CompiledRelation>,
    },
    CollectionSum {
        input: Box<CompiledRelation>,
        template: CollectionSumTemplate,
        datatype: CanonicalDatatype,
    },
    CollectionExtrema {
        input: Box<CompiledRelation>,
        value_column: usize,
        datatype: CanonicalDatatype,
        kind: GroupExtremaKind,
    },
    /// WOQL `Eval` of an INTEGER arithmetic tree (`Plus`/`Minus`/`Times`/`Div`/`Exp` over
    /// integer literals and authenticated integer child columns; `Div` is floor division and
    /// `Exp` is integer power over a non-negative exponent — see [`EvalExpr`]). The child
    /// relation `input` binds
    /// AND authenticates every operand variable; this statement carries through the child
    /// columns and appends exactly ONE derived signed-integer column (`GeneratedDecimalI64`)
    /// that the verifier RE-EVALUATES elementwise. See [`EvalExpr`] for the soundness model.
    Eval {
        input: Box<CompiledRelation>,
        expr: EvalExpr,
    },
    /// WOQL `Typecast` of ONE authenticated integer column to a fixed-width integer target
    /// type. The child relation `input` binds AND authenticates the source column; this
    /// statement carries through the child columns and appends exactly ONE derived integer
    /// column holding the cast value, which the verifier RE-COMPUTES elementwise from the
    /// root-authenticated source value.
    ///
    /// SOUNDNESS (verifier recompute — no novel gadget, no value change): an integer→integer
    /// cast preserves the numeric VALUE (Prolog `typecast` is a lossless identity on the value
    /// when it fits, and THROWS `casting_error` when it does not — never wraps/truncates; see
    /// `casting.pl`). The verifier decodes the source value from the trusted root dictionary
    /// (exactly as Sum/Eval do, re-checking `source_datatype`), asserts it lies within the
    /// TARGET type's exact XSD range, and appends the value in the target's generated namespace
    /// (`GeneratedDecimalI64` for signed targets, `GeneratedDecimalU64` for unsigned). A value
    /// outside the target range FAILS CLOSED (matching the Prolog throw); there is no biasable
    /// witness. `source_datatype`/`target_datatype` are bound into the query digest.
    Typecast {
        input: Box<CompiledRelation>,
        /// The (nonnullable, fixed-width-integer) child result column that holds the source.
        source_column: usize,
        /// The exact fixed-width integer datatype the compiler pinned for `source_column`;
        /// re-checked against the AUTHENTICATED dictionary value at recompute time.
        source_datatype: CanonicalDatatype,
        /// The fixed-width integer target type. Its signedness selects the derived column's
        /// generated namespace and its XSD range bounds the recompute's fail-closed check.
        target_datatype: CanonicalDatatype,
    },
    /// WOQL `Concatenate` (`concat(List, Result)`) — the FOUNDATION of the read-only string
    /// value-circuit family, using the SAME verifier-recompute architecture as `Eval`/`Typecast`
    /// but over string BYTES instead of integers (no novel proof gadget).
    ///
    /// `interpolate_string/2` (TerminusDB `src/core/util/utils.pl`) concatenates the list
    /// elements left-to-right with NO separator; this is DETERMINISTIC, semantically
    /// UNAMBIGUOUS, and — unlike `upper`/`lower` — LOCALE-INDEPENDENT (pure `string_concat`,
    /// no `towupper`/`towlower`). The child relation `input` binds AND authenticates every
    /// operand VARIABLE (each is a nonnullable node/value store column); `elements` is the
    /// ordered concat template of query-literal bytes and authenticated child string columns.
    ///
    /// TERMINAL RESULT — the synthesized string is REVEALED, not committed. A synthesized
    /// string does not fit a single field element (unlike an integer's `GeneratedDecimalI64`),
    /// and it feeds no downstream authenticated operation, so — exactly as the task's
    /// byte-authentication foundation dictates — NO new committed column/format is added. The
    /// committed `result_columns` are the child's operand columns verbatim (already root-bound);
    /// the per-row concatenation is DELIVERED alongside and the verifier RE-DERIVES it from the
    /// root-authenticated operands (see [`expected_concat_objects`]). Because the inputs are
    /// pinned to the trusted root and byte concatenation is deterministic, there is no biasable
    /// witness — the identical soundness model the arithmetic circuits use.
    Concat {
        input: Box<CompiledRelation>,
        /// Ordered concat template. Order + literal-vs-column + literal bytes + column indices
        /// are all bound into the query digest, so a proof cannot be replayed as a reordered or
        /// differently-literal concatenation.
        elements: Vec<ConcatElement>,
        /// The fresh result VARIABLE name that `concat` binds. Tracked only for query identity
        /// (digest) and terminal-string exposure; it is deliberately NOT a committed schema
        /// column (the result is revealed, not committed — see the variant docs).
        result: String,
    },
    /// WOQL `Join` (`join(List, Sep, Result)`) — a SEPARATOR-interposed extension of `Concat`.
    ///
    /// FIDELITY (reference `utils:join/3`, `src/core/util/utils.pl:777`): `join` is
    /// `intersperse(Sep, List, New_List)` then `interpolate_string(New_List, Result)`. For a list
    /// of xsd:string elements `[e0, e1, ..., e_{n-1}]` and an xsd:string separator `Sep`,
    /// `intersperse` yields `[e0, Sep, e1, Sep, ..., Sep, e_{n-1}]` (SEPARATOR only BETWEEN
    /// adjacent elements — a single-element list is just `[e0]`, so `Sep` is not emitted) and
    /// `interpolate_string` `string_concat`s them left-to-right. Because every element and the
    /// separator are already xsd:strings, `interpolate_string` takes its `string(H)` branch and
    /// concatenates them VERBATIM (no `term_to_atom`/`atom_string` stringification) — a pure,
    /// LOCALE-INDEPENDENT byte concatenation, exactly like `Concat`. Hence the recompute is
    /// literally `Concat`'s byte recompute over the INTERPOSED element sequence (see
    /// [`expected_join_objects`]). The mode is `[ground, ground, any]` (`join(X,S,Y)`,
    /// `woql_compile.pl:2902`): List + Sep must be ground (a concrete inline string list and a
    /// string separator), Result fresh — which is exactly the compile-time shape guard.
    ///
    /// SOUNDNESS: identical to `Concat` — operand bytes are pinned to the trusted root, the
    /// separator + elements are bound into the digest under a DISTINCT `/string-join-bytes` tag
    /// (so a join proof cannot be replayed as a concat, nor as a reordered/re-separated join),
    /// and the verifier RE-DERIVES the per-row result. No new committed column/format.
    Join {
        input: Box<CompiledRelation>,
        /// The RAW list elements (NOT yet interposed), in order. Each is an xsd:string literal or
        /// an authenticated child string column, exactly like a `Concat` element.
        elements: Vec<ConcatElement>,
        /// The separator interposed between consecutive `elements` (an xsd:string literal or an
        /// authenticated child string column). Bound into the digest DISTINCTLY from the elements.
        separator: ConcatElement,
        /// The fresh result VARIABLE name that `join` binds — terminal/revealed, NOT committed.
        result: String,
    },
    /// WOQL `Substring` (`sub_string(String, Before, Length, After, SubString)`) — a per-row,
    /// CONSTANT-index, CHARACTER-based substring map over an authenticated xsd:string column.
    ///
    /// FIDELITY (reference `utils:sub_string/5` = SWI-Prolog builtin `sub_string/5`; the
    /// `utils:` module qualifier resolves to the system builtin — utils.pl defines NO
    /// `sub_string/5`, it only USES the builtin, e.g. `utils.pl:943`). SWI `sub_string/5` counts
    /// CODE POINTS (Unicode characters), NOT bytes, and `Before + Length + After ==
    /// string_length(String)` in characters. In the supported mode `Before`/`Length` are ground
    /// non-negative integers, `After`/`SubString` are outputs: `After = total_chars - Before -
    /// Length`, `SubString` = the `Length` characters starting at character offset `Before`. Rust
    /// `char` is a Unicode scalar value (= code point for valid UTF-8, which has no surrogates),
    /// so `text.chars().skip(Before).take(Length)` reproduces SWI's substring byte-for-byte (see
    /// [`expected_substring_objects`]). When `Before + Length > total_chars` SWI `sub_string`
    /// FAILS (the row yields no solution / is dropped); we FAIL CLOSED at recompute rather than
    /// fabricate an out-of-range substring — so a proof is only ever produced when NO row is
    /// dropped, in which case the per-row map equals the reference result exactly. Negative
    /// indices are rejected at COMPILE time (a negative literal); `After`/`SubString` bound
    /// (adding a length-equality/occurrence-enumeration constraint) and variable `Before`/`Length`
    /// are rejected closed (other modes we do not prove faithful).
    ///
    /// SOUNDNESS: the operand bytes are pinned to the trusted root; `Before`/`Length` are trusted
    /// query constants bound into the digest under a DISTINCT `/string-substring-bytes` tag; the
    /// verifier RE-DERIVES the substring. No prover latitude, no new committed column/format.
    Substring {
        input: Box<CompiledRelation>,
        /// Index of the authenticated child xsd:string column being sliced.
        string_column: usize,
        /// Character offset (`Before`) — a trusted, compile-time, non-negative constant.
        before: usize,
        /// Character count (`Length`) — a trusted, compile-time, non-negative constant.
        length: usize,
        /// The fresh result VARIABLE name that `sub_string` binds — terminal/revealed, NOT
        /// committed. (The `After` output variable is likewise fresh and NOT revealed.)
        result: String,
    },
    /// WOQL `Pad` (`pad(String, Char, Times, Result)`) — a per-row LEFT-pad of an authenticated
    /// xsd:string column to a CONSTANT target CHARACTER length, using a CONSTANT single-character
    /// pad.
    ///
    /// FIDELITY (reference `utils:pad/4`, utils.pl:320). `pad(T, C, L, A2)`:
    ///   `format(atom(A),'~w',[T])` stringifies the operand (identity on an xsd:string's content);
    ///   `C` MUST interpolate to a string of `string_length == 1` — i.e. exactly ONE CODE POINT
    ///   (else `throw(pad_not_length_one)`); `L1 = char_length(A)`; `L2 is L - L1`; if `L2 < 0`
    ///   (operand already LONGER than the target) `throw(bad_pad_length)` — the op FAILS; otherwise
    ///   `L2` copies of `C` are PREPENDED (`append(Padding, AtomList, Total)`) and the result is
    ///   `Padding ++ A`. Length is in CHARACTERS (code points), locale-INDEPENDENT. Note the AST arg
    ///   is named `times` but is semantically the TARGET TOTAL LENGTH `L` (`utils.pl` uses
    ///   `L2 is L - L1`), so we treat it as the target character length. `L2 == 0` (operand exactly
    ///   the target) yields the operand unchanged. Rust `char` = a Unicode scalar value (= code
    ///   point for valid UTF-8), so `text.chars().count()` reproduces SWI's `string_length`, and
    ///   prepending `pad_char`'s exact UTF-8 bytes `L2` times reproduces `repeat_term`/`append`
    ///   byte-for-byte (see [`expected_pad_objects`]). Operand-longer-than-target FAILS CLOSED at
    ///   recompute (never fabricate a truncation the reference would reject).
    ///
    /// SOUNDNESS: the operand bytes are pinned to the trusted root; `pad_char` and `target_len` are
    /// trusted query constants bound into the digest under a DISTINCT `/string-pad-bytes` tag; the
    /// verifier RE-DERIVES the padded string. No prover latitude, no new committed column/format.
    /// The reference's WOQL-level marshalling (`woql_compile.pl:2806`) is idiosyncratic (it passes
    /// the compiled goal as a 5th argument), but the proven semantics are exactly `utils:pad/4`, and
    /// the result is the documented "result of the padding as a string" (xsd:string).
    Pad {
        input: Box<CompiledRelation>,
        /// Index of the authenticated child xsd:string column being padded.
        string_column: usize,
        /// The single-character pad, as its raw UTF-8 bytes. Its CODE-POINT count is exactly 1
        /// (validated at compile), so repeating these bytes N times repeats the char N times.
        pad_char: Vec<u8>,
        /// The target TOTAL character length `L` — a trusted, compile-time, non-negative constant.
        target_len: usize,
        /// The fresh result VARIABLE name that `pad` binds — terminal/revealed, NOT committed.
        result: String,
    },
    /// WOQL `HashKey`/`hash` (`hash(Base, Args, Id)`) — a per-row document-ID hash: SHA-256 over the
    /// `idgen`-canonicalized `Args`, appended to a raw `Base`.
    ///
    /// FIDELITY (reference `idgen_hash/3`, `src/core/document/json.pl:467`, reached via
    /// `woql_compile.pl:2719` `hash(Base,Args,Id) -> idgen_hash(BaseL, ArgsL, IdS)` after
    /// `literally` strips every operand's `^^type` to its raw string content). `idgen_hash(Base,
    /// Values, ID)`:
    ///   `idgen_suffix(Values, Suffix)` = `maplist(raw, Values, Raw)` (identity on plain strings —
    ///     `raw/2`'s last clause; only JSON-LD dicts are transformed, which `literally` never
    ///     produces here, so NO set-sorting/normalization applies to the WOQL `hash` path),
    ///     then `maplist(encode_id_fragment, Raw, Encoded)`, then `merge_separator_split(Suffix,
    ///     '+', Encoded)` = the encoded fragments joined by a literal `'+'`;
    ///   `crypto_data_hash(Suffix, Hash, [algorithm(sha256)])` = lowercase 64-char hex SHA-256 of
    ///     the Suffix's bytes (the Suffix is ALWAYS pure ASCII — see [`encode_id_fragment`] — so the
    ///     hashed bytes are unambiguous);
    ///   `format(string(ID), "~w~w", [Base, Hash])` = the RAW base string (NOT encoded) followed by
    ///     the hex digest.
    /// `encode_id_fragment` = SWI `uri_encoded(segment, ...)` then `re_replace('+' -> '%2B')`; its
    /// exact byte behavior was captured from the reference's own SWI builtins and is reproduced in
    /// [`encode_id_fragment`]. The result is bound to a `NodeValue` (a URI/node), so we reveal it as
    /// a [`CanonicalObject::Node`].
    ///
    /// SOUNDNESS: `Base` and every `Arg` are byte-authenticated against the trusted root (literal
    /// bytes come from the trusted compiled query); the verifier RE-COMPUTES the exact `idgen_hash`
    /// recipe and reveals the result. The op kind + operand structure are bound into the digest
    /// under a DISTINCT `/string-idgen-hash-sha256` tag. No prover latitude, no new committed
    /// column/format, no novel crypto (SHA-256 is already the digest primitive).
    HashKey {
        input: Box<CompiledRelation>,
        /// The `Base` string, RAW-prepended (never `idgen`-encoded) — an authenticated child string
        /// column or a trusted query literal.
        base: ConcatElement,
        /// The `Args`/`key_list`, in order (NO sorting): each an authenticated child string column
        /// or a trusted query literal, `idgen`-canonicalized then joined by `'+'`.
        args: Vec<ConcatElement>,
        /// The fresh result VARIABLE name that `hash` binds — terminal/revealed, NOT committed.
        result: String,
    },
    /// WOQL `Eval floor(x)` — the INTEGER-RESULT face of the exact-rational value layer. The child
    /// relation `input` binds AND authenticates every operand column; this statement carries the
    /// child columns through and appends exactly ONE derived signed-integer column
    /// (`GeneratedDecimalI64`, EXACTLY as `Eval`) holding `floor(x)`, which the verifier
    /// RE-DERIVES elementwise in EXACT rational arithmetic. See [`DecimalExpr`] for the operand and
    /// operator set.
    ///
    /// FIDELITY (SWI `floor/1` under `prefer_rationals`): `floor(x)` rounds toward −∞ to an
    /// INTEGER (`floor(7 rdiv 2)=3`, `floor(-7 rdiv 2)=-4`). An integer IS a rational, so
    /// `infer_result_type` labels the result `xsd:decimal` — but its VALUE is integral, matching the
    /// existing `Eval` integer-result delivery (a whole rational renders as an integer, see
    /// `jsonld:value_jsonld`). SOUNDNESS: the argument is re-derived in exact `rug::Rational` from
    /// ROOT-authenticated operands, floored toward −∞, and narrowed to the declared `i64` generated
    /// domain; a result outside `i64` FAILS CLOSED (no field wraparound). The delivered column must
    /// equal that recomputation — no biasable witness.
    FloorEval {
        input: Box<CompiledRelation>,
        /// The rational argument of `floor`, over authenticated numeric child columns and query
        /// literals. Bound into the query digest so a proof cannot be replayed under a different
        /// expression.
        expr: DecimalExpr,
    },
    /// WOQL exact-rational arithmetic whose result is `xsd:decimal` and POSSIBLY FRACTIONAL —
    /// `Divide` (`/` -> `rdiv`) and `Plus`/`Minus`/`Times`/`Exp` that involve a stored `xsd:decimal`
    /// operand or a decimal literal. Uses the SAME verifier-recompute architecture as `Eval`, but —
    /// like `Concat` — the result is a TERMINAL, REVEALED xsd:decimal value, not a committed column
    /// (a general rational does not encode into a single field element and feeds nothing downstream).
    ///
    /// FIDELITY (the load-bearing fidelity question). With `prefer_rationals=true`
    /// (`terminus_config.pl:191`) WOQL `+`,`-`,`*`,`rdiv` yield EXACT rationals typed `xsd:decimal`
    /// (`json_woql.pl`, `woql_compile:infer_result_type`). The CLIENT-facing lexical is produced by
    /// `casting:rational_to_decimal_string(R, S, 20)` via `json:json_write_hook/4`
    /// (`document/json.pl:135`): a decimal string with UP TO 20 FRACTIONAL digits, ROUND-HALF-UP at
    /// the 20th (`(Rem*10^20*2 + Den) // (Den*2)`), TRAILING ZEROS TRIMMED, emitted as an unquoted
    /// JSON number (whole rationals render as the bare integer). Terminating `7 rdiv 2` -> "3.5",
    /// `6 rdiv 2` -> "3", `1 rdiv 4` -> "0.25"; non-terminating `1 rdiv 3` -> "0.33333333333333333333"
    /// (twenty 3s). This is reproduced BYTE-EXACT by [`rational_to_decimal_string_20`] (rug/GMP is
    /// the same big-integer library SWI uses). The verifier RE-DERIVES the exact rational from the
    /// ROOT-authenticated operands and re-renders this lexical; a mismatch, a zero divisor, or a
    /// non-numeric operand row FAILS CLOSED. Delivered as [`CanonicalObject::Value`] tagged
    /// `xsd:decimal` with the rendered lexical bytes — the identical terminal-revealed discipline
    /// `Concat` uses (no committed result column, no new format).
    RationalArith {
        input: Box<CompiledRelation>,
        /// The exact-rational arithmetic expression. Bound into the query digest (operator, operand
        /// order, column indices, literal bytes) so a proof cannot be replayed under a different
        /// expression.
        expr: DecimalExpr,
        /// The fresh result VARIABLE name the arithmetic binds — terminal/revealed, NOT committed
        /// (tracked only for query identity / terminal exposure, like `Concat`'s `result`).
        result: String,
    },
    /// WOQL single-input INTEGER-result CALENDAR ops (`Weekday`/`WeekdaySundayStart`/`IsoWeek`) — the
    /// integer face of the read-only calendar value layer, delivered EXACTLY like [`Self::Eval`]. The
    /// child relation `input` binds AND authenticates the `xsd:date` operand column; this statement
    /// carries the child columns through and appends ONE derived signed-integer column
    /// (`GeneratedDecimalI64`) PER `outputs` entry, which the verifier RE-DERIVES from the
    /// root-authenticated date (see [`expected_calendar_int_columns`]). `outputs` holds one function
    /// per bound integer output — one for Weekday/WeekdaySundayStart, two (`[IsoWeekYear, IsoWeekWeek]`)
    /// for IsoWeek — in the SAME order the corresponding result variables were appended to the schema.
    ///
    /// SOUNDNESS: identical to `Eval` — the operand date is pinned to the trusted root, each calendar
    /// function is deterministic and ported byte-exact from the SWI reference, so there is no biasable
    /// witness. The op kind + operand column are bound into the query digest under a DISTINCT tag.
    CalendarInt {
        input: Box<CompiledRelation>,
        /// Index of the authenticated child `xsd:date` operand column.
        date_column: usize,
        /// The integer function per appended output column, in schema-append order. Bound into the
        /// query digest so a proof cannot be replayed as a different calendar op / column layout.
        outputs: Vec<CalendarIntFn>,
    },
    /// WOQL single-input DATE-result CALENDAR ops (`DayAfter`/`DayBefore`/`MonthStartDate`/
    /// `MonthEndDate`) — the date face of the read-only calendar value layer, delivered EXACTLY like
    /// [`Self::RationalArith`]/[`Self::Concat`]: a TERMINAL, REVEALED `xsd:date` value (NOT a committed
    /// column). The child relation `input` binds AND authenticates the operand column (an `xsd:date`
    /// for `Day*`, an `xsd:gYearMonth` for `Month*`); the child columns pass through verbatim and the
    /// per-row result is DELIVERED alongside, re-derived by the verifier (see
    /// [`expected_calendar_date_objects`]) as the client-facing `YYYY-MM-DD` lexical.
    ///
    /// SOUNDNESS: identical to `Concat` — the operand is pinned to the trusted root, the calendar
    /// arithmetic is deterministic and ported byte-exact, and the delivered result must equal the
    /// recomputation. The op kind + operand column are bound into the query digest under a DISTINCT tag.
    CalendarDate {
        input: Box<CompiledRelation>,
        /// Which date function this statement computes.
        op: CalendarDateFn,
        /// Index of the authenticated child operand column (`xsd:date` for `Day*`, `xsd:gYearMonth`
        /// for `Month*`).
        input_column: usize,
        /// The fresh result VARIABLE name the op binds — terminal/revealed, NOT a committed column
        /// (tracked only for query identity / terminal exposure, like `Concat`'s `result`).
        result: String,
    },
    /// WOQL `Typecast(NumericVal, xsd:string, Result)` — a per-row NUMBER→STRING cast, delivered
    /// EXACTLY like [`Self::RationalArith`]/[`Self::Concat`]: a TERMINAL, REVEALED `xsd:string`
    /// value (NOT a committed column — a synthesized lexical does not encode into a field element
    /// and feeds nothing downstream). The child relation `input` binds AND authenticates the
    /// numeric source column; the child columns pass through verbatim and the per-row cast string
    /// is DELIVERED alongside and re-derived by the verifier (see [`expected_cast_to_string_objects`]).
    ///
    /// FIDELITY (`casting:typecast_switch`, `terminusdb/src/core/triple/casting.pl`): an integer
    /// source (fixed-width `xsd:int`/`long`/… or arbitrary `xsd:integer`) casts to string via
    /// `format(string(S),"~w",[Val])` = base-10, `-` sign, no leading zeros; an `xsd:decimal`
    /// source casts via `rational_to_decimal_string(Val, S, 20)` — the SAME byte-exact renderer
    /// [`rational_to_decimal_string_20`] the decimal layer already uses. A `float`/`double` source
    /// (SWI's own float `~w` formatting) or any non-numeric source FAILS CLOSED at recompute
    /// (reject-closed over approximate). SOUNDNESS is identical to `RationalArith`: the operand is
    /// pinned to the trusted root, the render is deterministic and byte-exact, and the delivered
    /// value must equal the recomputation.
    TypecastNumberToString {
        input: Box<CompiledRelation>,
        /// Index of the authenticated (nonnullable node/value) numeric source column.
        source_column: usize,
        /// The fresh result VARIABLE name the cast binds — terminal/revealed, NOT a committed column.
        result: String,
    },
    /// WOQL `Typecast(StringVal, xsd:integer | xsd:decimal, Result)` — a per-row STRING→NUMBER cast,
    /// delivered like [`Self::RationalArith`]/[`Self::Concat`]: a TERMINAL, REVEALED numeric value
    /// (NOT a committed column — an arbitrary `xsd:integer`/`xsd:decimal` does not encode into a
    /// single field element and feeds nothing downstream). The child relation `input` binds AND
    /// authenticates the `xsd:string` source column; the child columns pass through verbatim and the
    /// parsed value is DELIVERED alongside and re-derived by the verifier (see
    /// [`expected_cast_from_string_objects`]).
    ///
    /// FIDELITY (`casting:typecast_switch`): string→`xsd:integer` parses the `integer//1` grammar
    /// (`^[+-]?[0-9]+$`, leading zeros allowed) — we ship exactly that (`+` accepted, matching SWI);
    /// string→`xsd:decimal` parses `casting:string_decimal_to_rational/2` — we ship a BYTE-EXACT
    /// SAFE SUBSET (`-?` sign, `D+` / `D+.D+` / `.D+`; trailing-dot `D+.` REJECTED because SWI's
    /// `atom_number("")` fails; leading `+`, scientific `1e5` — which SWI's `atom_number` turns into
    /// a FLOAT — hex, and Prolog rational literals all REJECTED closed). A malformed/out-of-grammar
    /// string FAILS CLOSED at recompute (SWI `typecast` throws → the query fails → the proof fails
    /// closed, never a wrong/silent value). The result is delivered as its client-facing lexical
    /// (`xsd:integer` = base-10; `xsd:decimal` = [`rational_to_decimal_string_20`]).
    TypecastStringToNumber {
        input: Box<CompiledRelation>,
        /// Index of the authenticated (nonnullable node/value) `xsd:string` source column.
        source_column: usize,
        /// The numeric target type the string is parsed into. Bound into the query digest.
        target: StringNumericTarget,
        /// The fresh result VARIABLE name the cast binds — terminal/revealed, NOT a committed column.
        result: String,
    },
}

/// The numeric target of a WOQL `xsd:string`→number [`CompiledRelationStatement::TypecastStringToNumber`]
/// cast. Each parses a DISTINCT reference grammar and delivers a DISTINCT result datatype, so the
/// discriminant is bound into the query digest (a proof for one target cannot be replayed as the
/// other).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StringNumericTarget {
    /// `xsd:integer` (arbitrary precision) via the `integer//1` grammar; delivered as base-10.
    Integer,
    /// `xsd:decimal` (exact rational) via `string_decimal_to_rational/2`; delivered via
    /// [`rational_to_decimal_string_20`].
    Decimal,
}

/// One element of a WOQL `concat` template.
///
/// A leaf is either an xsd:string LITERAL taken from the trusted compiled query (its bytes are
/// already known to the verifier) or a reference to an authenticated child string result column
/// (its per-row value is root-authenticated at recompute time and REQUIRED to be an xsd:string —
/// a non-string row FAILS CLOSED rather than being stringified; whole-list stringification of
/// numbers/atoms/nodes is a separate circuit, deferred).
#[derive(Clone, Debug)]
pub enum ConcatElement {
    /// A query-local xsd:string literal, as its raw UTF-8 bytes (trusted; part of the compiled
    /// query the verifier reconstructs).
    Literal(Vec<u8>),
    /// The authenticated xsd:string value of child result column `index`. At recompute time the
    /// column is resolved through the root-authenticated node/value dictionary and each row is
    /// required to be a nonnull `xsd:string` VALUE; any other object (node IRI, non-string
    /// value, null) rejects closed.
    Column { index: usize },
}

/// A compiled INTEGER arithmetic expression tree for WOQL `Eval`.
///
/// Every leaf is either a query-local integer literal (already trusted — it is part of the
/// compiled query the verifier reconstructs) or a reference to an authenticated integer
/// child result column. The interior nodes are the checked-integer operators the verifier
/// RE-EVALUATES per row.
///
/// SOUNDNESS (verifier recompute — no novel arithmetic gadget): this tree carries NO prover
/// witness. During verification the operand columns are resolved through the
/// root-authenticated node/value dictionary (`resolve_result_values`, exactly as Sum does),
/// then the whole tree is re-evaluated elementwise in checked `i128` and the final value is
/// narrowed to the declared `i64` generated domain. The delivered result column must equal
/// that recomputation, so there is nothing a malicious prover can bias: the inputs are
/// pinned to the trusted root and the function is deterministic. This mirrors the
/// `plan_collection_sum_form`/`plan_group_sum_form` "verifier re-derivation" discipline.
#[derive(Clone, Debug)]
pub enum EvalExpr {
    /// The authenticated integer value of child result column `index`. `datatype` is the
    /// exact fixed-width integer datatype the compiler inferred for that column; it is
    /// re-checked against the AUTHENTICATED dictionary value at recompute time, so a wrong
    /// datatype claim rejects closed rather than decoding a different integer.
    Column {
        index: usize,
        datatype: CanonicalDatatype,
    },
    /// A query-local integer literal, normalized to `i128` (all supported fixed-width XSD
    /// integer literals fit exactly).
    Literal(i128),
    Plus(Box<EvalExpr>, Box<EvalExpr>),
    Minus(Box<EvalExpr>, Box<EvalExpr>),
    Times(Box<EvalExpr>, Box<EvalExpr>),
    /// WOQL `Div` — INTEGER FLOOR division (`left div right`), matching SWI-Prolog `div/2`
    /// (rounds toward NEGATIVE INFINITY), NOT truncation-toward-zero and NOT Rust's
    /// `div_euclid` (they disagree when the divisor is negative, e.g. `7 div -2 = -4` under
    /// floor but `-3` under Euclid). `left` is the dividend, `right` the divisor. A zero
    /// divisor makes Prolog `is/2` throw `evaluation_error(zero_divisor)`, so the WOQL query
    /// fails — the verifier recompute FAILS CLOSED on any zero-divisor row (never drops it).
    Div(Box<EvalExpr>, Box<EvalExpr>),
    /// WOQL `Exp` — integer power (`left ** right`), matching SWI-Prolog `**` under
    /// `prefer_rationals=true`: an integer base raised to a NON-NEGATIVE INTEGER exponent is an
    /// integer. `left` is the base, `right` the exponent. A negative exponent yields a rational
    /// (out of the integer value layer) and an oversized result overflows the declared width;
    /// both FAIL CLOSED in the recompute (no field wraparound, no silent rational coercion).
    Exp(Box<EvalExpr>, Box<EvalExpr>),
}

/// A compiled EXACT-RATIONAL arithmetic expression tree for the WOQL `xsd:decimal`/rational value
/// layer (`Floor`, `Divide`, decimal operands). It is the rational analogue of [`EvalExpr`]: every
/// leaf is a query-local literal (trusted) or an authenticated numeric child column, and the
/// interior nodes are re-evaluated per row by the verifier in EXACT `rug::Rational`.
///
/// SOUNDNESS (verifier recompute — no novel gadget, no crypto). This tree carries NO prover
/// witness. At verification each operand column is resolved through the ROOT-authenticated
/// node/value dictionary (`resolve_result_values`, exactly as `Eval`/`Sum` do); its EXACT datatype
/// is read from the trusted dictionary value and decoded to an exact rational (integer family via
/// the order-preserving decode, `xsd:decimal` via the store's own `storage_to_decimal` lexical
/// decoder). The tree is then re-evaluated deterministically in exact rationals. The delivered
/// result (a floored `i64` for [`CompiledRelationStatement::FloorEval`], a rendered decimal lexical
/// for [`CompiledRelationStatement::RationalArith`]) must equal that recomputation, so there is
/// nothing a malicious prover can bias: the inputs are pinned to the trusted root and the function
/// is deterministic. A zero divisor, a non-numeric operand row, or an unparseable decimal FAILS
/// CLOSED — never a fabricated or dropped row.
///
/// The compiler does NOT pin operand datatypes at compile time (a `xsd:decimal` operand has no
/// order-preserving range-pin mechanism); instead the recompute reads the exact datatype from the
/// trusted root value and rejects any non-numeric row. This is sound because the value is
/// root-bound. A `Column` is admitted at compile time only when it is a nonnullable node/value
/// store column (never a node IRI, predicate, or nullable Optional column).
#[derive(Clone, Debug)]
pub enum DecimalExpr {
    /// The authenticated numeric value of child result column `index`. At recompute the exact
    /// datatype is read from the ROOT-authenticated dictionary value (integer family OR
    /// `xsd:decimal`) and decoded to an exact rational; any other datatype, a node IRI, or a null
    /// rejects closed. No compile-time datatype claim is bound (the value is root-authenticated).
    Column { index: usize },
    /// A query-local exact integer literal (all supported fixed-width XSD integer literals fit
    /// `i128`), interpreted as the rational `value/1`.
    IntegerLiteral(i128),
    /// A query-local exact `xsd:decimal` literal, as its CANONICAL decimal string (optional leading
    /// `-`, digits, optional `.`+digits; NO exponent). Parsed to an exact rational — a
    /// non-canonical/scientific/garbage form rejects closed at compile time.
    DecimalLiteral(String),
    Plus(Box<DecimalExpr>, Box<DecimalExpr>),
    Minus(Box<DecimalExpr>, Box<DecimalExpr>),
    Times(Box<DecimalExpr>, Box<DecimalExpr>),
    /// WOQL `Divide` — EXACT RATIONAL division (`rdiv`), NOT integer/floor division. `left` is the
    /// dividend, `right` the divisor. A zero divisor makes Prolog `rdiv`/`is` throw
    /// (`evaluation_error(zero_divisor)`), so any zero-divisor row FAILS CLOSED in the recompute
    /// (never dropped). A static zero literal divisor is also rejected at compile time.
    Divide(Box<DecimalExpr>, Box<DecimalExpr>),
    /// WOQL `Exp` — integer power (`**`) of a rational base raised to a NON-NEGATIVE INTEGER
    /// exponent (computed as `numer^e / denom^e`). The exponent must be a static non-negative
    /// integer literal (a negative/fractional/column exponent is out of this layer and rejects at
    /// compile time). `left` is the base, `right` the exponent.
    Exp(Box<DecimalExpr>, Box<DecimalExpr>),
    /// `floor(x)` toward −∞ (SWI `floor/1`), yielding an integral rational. May appear nested (e.g.
    /// `Divide(Floor(x), y)`); a TOP-LEVEL `floor` is lowered to
    /// [`CompiledRelationStatement::FloorEval`] (integer delivery) instead.
    Floor(Box<DecimalExpr>),
}

impl CompiledRelation {
    pub fn as_bgp(&self) -> Option<&LayerBgpPlan> {
        match &self.statement {
            CompiledRelationStatement::Bgp(plan) => Some(plan),
            _ => None,
        }
    }

    pub fn as_bag_union(&self) -> Option<(&[CompiledRelation], &[Vec<usize>])> {
        match &self.statement {
            CompiledRelationStatement::BagUnion { branches, mappings } => {
                Some((branches, mappings))
            }
            _ => None,
        }
    }

    pub fn as_left_anti(
        &self,
    ) -> Option<(&CompiledRelation, &CompiledRelation, &[(usize, usize)])> {
        match &self.statement {
            CompiledRelationStatement::LeftAnti {
                left,
                right,
                key_pairs,
            } => Some((left, right, key_pairs)),
            _ => None,
        }
    }

    pub fn as_left_anti_mut(&mut self) -> Option<&mut Vec<(usize, usize)>> {
        match &mut self.statement {
            CompiledRelationStatement::LeftAnti { key_pairs, .. } => Some(key_pairs),
            _ => None,
        }
    }

    pub fn as_left_outer(
        &self,
    ) -> Option<(&CompiledRelation, &CompiledRelation, &[(usize, usize)])> {
        match &self.statement {
            CompiledRelationStatement::LeftOuter {
                left,
                right,
                key_pairs,
            } => Some((left, right, key_pairs)),
            _ => None,
        }
    }

    pub fn as_left_outer_mut(&mut self) -> Option<&mut Vec<(usize, usize)>> {
        match &mut self.statement {
            CompiledRelationStatement::LeftOuter { key_pairs, .. } => Some(key_pairs),
            _ => None,
        }
    }

    pub fn as_equality(&self) -> Option<(&CompiledRelation, &RelationEqualityPlan)> {
        match &self.statement {
            CompiledRelationStatement::Equality { input, constraint } => Some((input, constraint)),
            _ => None,
        }
    }

    pub fn as_exact_scalar_order(&self) -> Option<(&CompiledRelation, CanonicalDatatype)> {
        match &self.statement {
            CompiledRelationStatement::ExactScalarOrder { input, datatype, .. } => {
                Some((input, *datatype))
            }
            _ => None,
        }
    }

    pub fn as_exact_ordered_slice(&self) -> Option<(&CompiledRelation, u64, Option<u64>)> {
        match &self.statement {
            CompiledRelationStatement::ExactOrderedSlice { input, offset, limit, .. } => {
                Some((input, *offset, *limit))
            }
            _ => None,
        }
    }
}

impl CompiledBgp {
    /// Return the Store plan only when the authoritative relation is a leaf BGP.
    pub fn bgp_plan(&self) -> &LayerBgpPlan {
        self.relation
            .as_bgp()
            .expect("compiled relation is not a leaf BGP")
    }
}

pub(crate) fn compiled_result_descriptors(
    compiled: &CompiledBgp,
) -> std::io::Result<Vec<ResultColumnDescriptor>> {
    relation_result_descriptors(&compiled.relation)
}

/// An in-memory proof result produced only when a caller explicitly invokes
/// [`CompiledBgp::prove`] or [`CompiledBgp::prove_on_layer`]. It intentionally contains
/// no scheduling or persistence policy; applications own when and where proving runs.
#[derive(Debug)]
pub struct ProvedBgp {
    pub statement: ProvedQueryStatement,
    pub result_commitments: Vec<ArkGT>,
    /// Real, unpadded Store ID columns in the compiled schema order.
    pub result_columns: Vec<Vec<ArkFr>>,
    pub result_log_domain: usize,
    pub result_len: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExecutedResultValue {
    Id(u64),
    /// A generated nonnegative integral xsd:decimal, not a Store dictionary ID.
    GeneratedDecimalU64(u64),
    /// A generated signed integral xsd:decimal, not a Store dictionary ID.
    GeneratedDecimalI64(i64),
    Null,
}

/// Untyped foreign/executor wire value. Generated decimals deliberately remain lexical
/// until the independently compiled result descriptor selects the signed or unsigned
/// generated domain.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExecutedResultInput {
    Id(u64),
    GeneratedDecimal(String),
    Null,
}

pub(crate) fn parse_canonical_u64_decimal(value: &str) -> std::io::Result<u64> {
    let parsed = value.parse::<u64>().map_err(|_| std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        "generated unsigned decimal is outside canonical u64 range",
    ))?;
    if parsed.to_string() != value {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "generated unsigned decimal has a noncanonical integer lexical form",
        ));
    }
    Ok(parsed)
}

pub(crate) fn parse_canonical_i64_decimal(value: &str) -> std::io::Result<i64> {
    let parsed = value.parse::<i64>().map_err(|_| std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        "generated signed decimal is outside canonical i64 range",
    ))?;
    if parsed.to_string() != value {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "generated signed decimal has a noncanonical integer lexical form",
        ));
    }
    Ok(parsed)
}

pub fn decode_executed_result_inputs(
    compiled: &CompiledBgp,
    variables: &[String],
    rows: &[Vec<ExecutedResultInput>],
) -> std::io::Result<Vec<Vec<ExecutedResultValue>>> {
    if let Some(count_variable) = &compiled.count_variable {
        if variables != std::slice::from_ref(count_variable)
            || rows.iter().any(|row| row.len() != 1)
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "executed Count input shape does not match compiled count binding",
            ));
        }
        return rows
            .iter()
            .map(|row| match &row[0] {
                ExecutedResultInput::GeneratedDecimal(value) => parse_canonical_u64_decimal(value)
                    .map(|value| vec![ExecutedResultValue::Id(value)]),
                _ => Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "executed Count requires an explicit generated decimal",
                )),
            })
            .collect();
    }
    let projection = compiled
        .projection
        .clone()
        .unwrap_or_else(|| (0..compiled.relation.schema.len()).collect());
    let expected_variables = projection
        .iter()
        .map(|index| compiled.relation.schema[*index].as_str())
        .collect::<Vec<_>>();
    if variables.iter().map(String::as_str).collect::<Vec<_>>() != expected_variables
        || rows.iter().any(|row| row.len() != projection.len())
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "executed input shape does not match compiled projection",
        ));
    }
    let descriptors = compiled_result_descriptors(compiled)?;
    let projected = projection
        .iter()
        .map(|index| descriptors[*index])
        .collect::<Vec<_>>();
    rows.iter()
        .map(|row| {
            row.iter()
                .zip(&projected)
                .map(|(value, descriptor)| match (value, descriptor.namespace) {
                    (
                        ExecutedResultInput::Id(id),
                        ResultIdNamespace::NodeValue | ResultIdNamespace::Predicate,
                    ) if *id != 0 => Ok(ExecutedResultValue::Id(*id)),
                    (
                        ExecutedResultInput::GeneratedDecimal(value),
                        ResultIdNamespace::GeneratedDecimalU64,
                    ) => parse_canonical_u64_decimal(value)
                        .map(ExecutedResultValue::GeneratedDecimalU64),
                    (
                        ExecutedResultInput::GeneratedDecimal(value),
                        ResultIdNamespace::GeneratedDecimalI64,
                    ) => parse_canonical_i64_decimal(value)
                        .map(ExecutedResultValue::GeneratedDecimalI64),
                    (ExecutedResultInput::Null, _) if descriptor.nullable => {
                        Ok(ExecutedResultValue::Null)
                    }
                    _ => Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "executed foreign value kind does not match compiled result descriptor",
                    )),
                })
                .collect()
        })
        .collect()
}

#[derive(Debug)]
pub enum ProvedQueryStatement {
    LayerTripleCount,
    GeneratedU64Filter {
        input: Box<ProvedBgp>,
    },
    GeneratedI64Filter {
        input: Box<ProvedBgp>,
    },
    Single {
        proof: Box<LayerBgpProof>,
        predicate_ids: Vec<u64>,
    },
    BagUnion {
        branches: Vec<ProvedBgp>,
        proof: Box<BagUnionProof>,
    },
    BagProjection {
        input: Box<ProvedBgp>,
    },
    Distinct {
        input: Box<ProvedBgp>,
        proof: Box<RelationDistinctProof>,
    },
    LeftAnti {
        left: Box<ProvedBgp>,
        right: Box<ProvedBgp>,
        /// Correlated anti joins carry a proof; global Not is length-pinned.
        proof: Option<Box<LeftAntiJoinProof>>,
    },
    InnerJoin {
        left: Box<ProvedBgp>,
        right: Box<ProvedBgp>,
        proof: Box<InnerJoinProof>,
    },
    /// A proven top-level [`CompiledRelationStatement::CrossRootHashJoin`]. The whole two-root proof is
    /// the store [`RecursiveRelationEnvelope`] from `prove_cross_root`, carried wholesale (not
    /// decomposed) and verified by `verify_cross_root`, which binds BOTH per-graph roots to the trusted
    /// `database_root`. The ONE statement that engages two commitment roots.
    CrossRootHashJoin {
        envelope: Box<RecursiveRelationEnvelope>,
    },
    LeftOuter {
        left: Box<ProvedBgp>,
        right: Box<ProvedBgp>,
        proof: Box<LeftOuterJoinProof>,
    },
    Equality {
        input: Box<ProvedBgp>,
        proof: Box<RelationEqualityProof>,
    },
    TupleDisequality {
        input: Box<ProvedBgp>,
        proof: Box<TupleDisequalityProof>,
    },
    ExactScalarOrder {
        input: Box<ProvedBgp>,
        proof: Box<ExactScalarOrderProof>,
    },
    ExactMultiOrder {
        input: Box<ProvedBgp>,
        proof: Box<ExactMultiOrderProof>,
    },
    ExactOrderedSlice {
        input: Box<ProvedBgp>,
    },
    GroupExtrema {
        input: Box<ProvedBgp>,
    },
    /// A proved bounded transitive closure. The woql2 layer does NOT re-implement the closure
    /// orchestration: it carries the store proof pieces (the edge sub-relation mirrored as a
    /// `ProvedBgp`, plus the seed / rounds / empty-delta fixpoint), reconstructs the store
    /// `RecursiveRelationEnvelope`, and DELEGATES to the audited store verify. See
    /// [`plan_transitive_closure_path`].
    TransitiveClosure {
        edge: Box<ProvedBgp>,
        seed: TransitiveClosureSeed,
        rounds: Vec<TransitiveClosureRound>,
        fixpoint: Box<TransitiveClosureFixpoint>,
    },
    /// A proved ordered `rdf:List` walk. The store's `OrderedListWalk` composite proof is
    /// store-private (it cannot be decomposed into this tree the way a closure decomposes into
    /// edge/seed/rounds/fixpoint), so — exactly like [`Self::CrossRootHashJoin`] — the whole store
    /// [`RecursiveRelationEnvelope`] is carried WHOLESALE and verified by re-running the store's own
    /// recursive verify (which binds it to the trusted root AND the statement's query digest). See
    /// [`CompiledRelationStatement::OrderedListWalk`].
    OrderedListWalk {
        envelope: Box<RecursiveRelationEnvelope>,
    },
    /// A proved ordered 1-D DENSE `sys:Array` walk. The store's `OrderedArrayWalk` composite proof is
    /// store-private (it cannot be decomposed into this tree the way a closure decomposes into
    /// edge/seed/rounds/fixpoint), so — exactly like [`Self::OrderedListWalk`] / [`Self::CrossRootHashJoin`]
    /// — the whole store [`RecursiveRelationEnvelope`] is carried WHOLESALE and verified by re-running the
    /// store's own recursive verify (which binds it to the trusted root AND the statement's query digest).
    /// See [`CompiledRelationStatement::OrderedArrayWalk`].
    OrderedArrayWalk {
        envelope: Box<RecursiveRelationEnvelope>,
    },
    GroupSum {
        input: Box<ProvedBgp>,
    },
    GroupCount {
        input: Box<ProvedBgp>,
    },
    CollectionCount {
        input: Box<ProvedBgp>,
    },
    CollectionSum {
        input: Box<ProvedBgp>,
    },
    CollectionExtrema {
        input: Box<ProvedBgp>,
    },
    /// A proved WOQL `Eval`: the authenticated child relation plus one appended derived
    /// integer column. The proof carries no arithmetic-specific witness — verification
    /// re-derives the derived column from the child's root-authenticated columns.
    Eval {
        input: Box<ProvedBgp>,
    },
    /// A proved WOQL `Typecast`: the authenticated child relation plus one appended derived
    /// cast-value column. Like `Eval`, the proof carries no cast-specific witness —
    /// verification re-derives the cast column (and its target-range check) from the child's
    /// root-authenticated source column.
    Typecast {
        input: Box<ProvedBgp>,
    },
    /// A proved WOQL `Concatenate`: the authenticated operand-binding child relation plus the
    /// DELIVERED per-row concatenation. The committed `result_columns` are the child's columns
    /// verbatim (no committed result column — the concat result is terminal/revealed). The
    /// delivered `result_objects` carry NO independent trust: verification RE-DERIVES them from
    /// the child's root-authenticated operand columns (+ query literals) and requires an exact
    /// match, so a tampered/wrong result-string column fails closed.
    Concat {
        input: Box<ProvedBgp>,
        /// Per-row synthesized xsd:string results, revealed and re-derived by the verifier.
        result_objects: Vec<CanonicalObject>,
    },
    /// A proved WOQL `Join`: like `Concat`, the authenticated operand-binding child plus the
    /// DELIVERED per-row separator-interposed join. The committed columns are the child's verbatim
    /// (the join result is terminal/revealed); verification RE-DERIVES `result_objects` from the
    /// child's root-authenticated operand columns (+ query literals) with the separator interposed
    /// and requires an exact match, so a tampered/wrong result column fails closed.
    Join {
        input: Box<ProvedBgp>,
        /// Per-row synthesized xsd:string joins, revealed and re-derived by the verifier.
        result_objects: Vec<CanonicalObject>,
    },
    /// A proved WOQL `Substring`: like `Concat`, the authenticated operand-binding child plus the
    /// DELIVERED per-row character-substring. The committed columns are the child's verbatim (the
    /// substring is terminal/revealed); verification RE-DERIVES `result_objects` by char-slicing
    /// the child's root-authenticated string column at the trusted constant offsets and requires
    /// an exact match. Any out-of-range/invalid-UTF-8 row fails closed at re-derivation.
    Substring {
        input: Box<ProvedBgp>,
        /// Per-row synthesized xsd:string substrings, revealed and re-derived by the verifier.
        result_objects: Vec<CanonicalObject>,
    },
    /// A proved WOQL `Pad`: like `Concat`, the authenticated operand-binding child plus the
    /// DELIVERED per-row left-padded string. The committed columns are the child's verbatim (the
    /// padded string is terminal/revealed); verification RE-DERIVES `result_objects` by
    /// left-padding the child's root-authenticated string column to the trusted target length with
    /// the trusted pad character and requires an exact match. Any operand-longer-than-target or
    /// invalid-UTF-8 row FAILS CLOSED at re-derivation.
    Pad {
        input: Box<ProvedBgp>,
        /// Per-row synthesized xsd:string padded results, revealed and re-derived by the verifier.
        result_objects: Vec<CanonicalObject>,
    },
    /// A proved WOQL `HashKey`/`hash`: like `Concat`, the authenticated operand-binding child plus
    /// the DELIVERED per-row document-ID hash. The committed columns are the child's verbatim (the
    /// id is terminal/revealed); verification RE-DERIVES `result_objects` by running the exact
    /// `idgen_hash` recipe (SHA-256 over the `idgen`-encoded, `'+'`-joined authenticated args,
    /// appended to the raw authenticated base) and requires an exact match. Each id is revealed as
    /// a [`CanonicalObject::Node`] (the reference binds a URI/node).
    HashKey {
        input: Box<ProvedBgp>,
        /// Per-row synthesized URI/node ids, revealed and re-derived by the verifier.
        result_objects: Vec<CanonicalObject>,
    },
    /// A proved WOQL `floor(x)`: like `Eval`, the authenticated child relation plus ONE appended
    /// derived integer column. The proof carries no floor-specific witness — verification re-derives
    /// the floored column in exact rationals from the child's root-authenticated operand columns.
    FloorEval {
        input: Box<ProvedBgp>,
    },
    /// A proved WOQL exact-rational arithmetic (`Divide`/decimal-operand `Plus`/`Minus`/`Times`/
    /// `Exp`): like `Concat`, the authenticated operand-binding child plus the DELIVERED per-row
    /// `xsd:decimal` result. The committed columns are the child's verbatim (the result is
    /// terminal/revealed); verification RE-DERIVES `result_objects` by re-evaluating the exact
    /// rational over the root-authenticated operands and re-rendering the canonical 20-fractional-
    /// digit decimal lexical, requiring an exact match. A zero divisor, a non-numeric operand row,
    /// or the astronomically-implausible carry-overflow case FAIL CLOSED at re-derivation.
    RationalArith {
        input: Box<ProvedBgp>,
        /// Per-row synthesized xsd:decimal results, revealed and re-derived by the verifier.
        result_objects: Vec<CanonicalObject>,
    },
    /// A proved WOQL INTEGER-result calendar op (`Weekday`/`WeekdaySundayStart`/`IsoWeek`): like
    /// `Eval`, the authenticated child relation plus one-or-more appended derived integer columns.
    /// The proof carries no calendar-specific witness — verification re-derives the derived column(s)
    /// from the child's root-authenticated `xsd:date` column.
    CalendarInt {
        input: Box<ProvedBgp>,
    },
    /// A proved WOQL DATE-result calendar op (`DayAfter`/`DayBefore`/`MonthStartDate`/`MonthEndDate`):
    /// like `Concat`, the authenticated operand-binding child plus the DELIVERED per-row `xsd:date`
    /// result. The committed columns are the child's verbatim (the result is terminal/revealed);
    /// verification RE-DERIVES `result_objects` from the child's root-authenticated operand and
    /// requires an exact match, so a tampered/wrong result fails closed.
    CalendarDate {
        input: Box<ProvedBgp>,
        /// Per-row synthesized xsd:date results, revealed and re-derived by the verifier.
        result_objects: Vec<CanonicalObject>,
    },
    /// A proved WOQL NUMBER→STRING `Typecast`: like `Concat`, the authenticated source-binding child
    /// plus the DELIVERED per-row `xsd:string` cast. The committed columns are the child's verbatim
    /// (the cast string is terminal/revealed); verification RE-DERIVES `result_objects` from the
    /// child's root-authenticated numeric source column and requires an exact match, so a
    /// tampered/wrong result fails closed.
    TypecastNumberToString {
        input: Box<ProvedBgp>,
        /// Per-row synthesized xsd:string casts, revealed and re-derived by the verifier.
        result_objects: Vec<CanonicalObject>,
    },
    /// A proved WOQL STRING→NUMBER `Typecast`: like `Concat`, the authenticated source-binding child
    /// plus the DELIVERED per-row numeric (`xsd:integer`/`xsd:decimal`) cast. The committed columns
    /// are the child's verbatim (the numeric value is terminal/revealed); verification RE-DERIVES
    /// `result_objects` by re-parsing the child's root-authenticated `xsd:string` source column with
    /// the exact reference grammar and requires an exact match. A malformed/out-of-grammar string
    /// FAILS CLOSED at re-derivation.
    TypecastStringToNumber {
        input: Box<ProvedBgp>,
        /// Per-row synthesized numeric casts, revealed and re-derived by the verifier.
        result_objects: Vec<CanonicalObject>,
    },
}

/// A decoded envelope that has already been checked against the caller's query, trusted
/// root and layer. Canonical results remain alongside the reconstructed proof result.
#[derive(Debug)]
pub struct VerifiedBgpEnvelope {
    pub proved: ProvedBgp,
    pub result_objects: Vec<Vec<CanonicalObject>>,
    pub result_values: Option<Vec<Vec<CanonicalResultValue>>>,
}

