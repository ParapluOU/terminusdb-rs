//! Compile a WOQL conjunctive BGP AST into a safe, variable-blind [`LayerBgpPlan`].
//!
//! See the crate docs for why the PLAN is the sole safety line. This module accepts
//! only the fragment the authenticated left-deep join chain can encode faithfully and
//! rejects everything else closed:
//!
//! * zero or more nested outer [`Select`]s (projection is read-side only — the proof
//!   still proves the FULL BGP), wrapping
//! * an [`And`] (nested `And`s flattened) of `k >= 1` [`Triple`]s, each a
//!   `<subj> <predicate> <obj>` atom where the predicate is either a constant IRI or a
//!   query variable retained from the authenticated P column, and each of subject/object
//!   is EITHER a query VARIABLE (a kept output column) OR an
//!   aligned CONSTANT — a subject/object node IRI ([`AtomTerm::NodeConstant`]) or an
//!   object typed literal of a round-trip-aligned xsd datatype ([`AtomTerm::ValueConstant`]:
//!   all scalar `XSDAnySimpleType` variants, routed through the Store's
//!   typed-value construction). Each atom keeps one output column per variable (S,P,O order);
//!   the atoms join into a left-deep chain. Shared variables use ordinary natural joins;
//!   disconnected components use the Store's verifier-known UNIT-key Cartesian product.
//!   Two-variable graph keys use the Store's injective two-u64 packing and remain bound
//!   to the original columns.
//!
//! Fully-ground atoms are authenticated zero-arity existence relations and gate the
//! relational result. Every emitted constant is one the store
//! can authenticate against its committed node/value dictionary, canonicalized IDENTICALLY
//! to how the store stored it (node IRIs by their IRI bytes; `xsd:string` literals by
//! routing through the store's own value construction).

use chrono::Datelike;
use dory_pcs::backends::arkworks::{ArkFr, ArkGT};
use dory_pcs::primitives::arithmetic::Field;
use dory_pcs::primitives::transcript::Transcript;
use sha2::{Digest, Sha256};
use tdb_succinct::{AnyURI, Date, HexBinary, TdbDataType, TypedDictEntry};
use terminus_store::proof::anti::{
    verify_global_left_outer_join, verify_global_not, verify_left_anti_join_with_keys,
    verify_inner_join_with_keys, verify_left_outer_join_with_keys, InnerJoinProof,
    LeftAntiJoinProof, LeftOuterJoinProof,
};
use terminus_store::proof::argument::{commit_column, StoreProverContext, Transcript254};
use terminus_store::proof::bgp::{JoinStep, RelationCommit};
use terminus_store::proof::commitment::{
    LayerCommitment, VerifierCommitment, VerifierLayer,
};
use terminus_store::proof::envelope::{
    plan_digest, resolve_relation_equality_constraint, resolve_tuple_disequality_pairs,
    CollectionSumTemplate, EqualityValueDomain,
    GeneratedU64ComparisonOp, LayerBgpEnvelope,
    RecursiveRelationEnvelope, RecursiveRelationProof, RelationEqualityPlan, RelationProofPlan,
    RelationProofPlanKind, TransitiveClosureFixpoint, TransitiveClosureRound, TransitiveClosureSeed,
    TupleDisequalityNode, TupleDisequalityPairPlan,
    MAX_RELATION_PROOF_DEPTH, MAX_RELATION_PROOF_NODES,
};
use terminus_store::proof::disequality::{verify_tuple_disequality, TupleDisequalityProof};
use terminus_store::proof::equality::{
    verify_relation_equality, RelationEqualityProof,
};
use terminus_store::proof::distinct::{verify_relation_distinct, RelationDistinctProof};
use terminus_store::proof::group::GroupExtremaKind;
use terminus_store::proof::order::{verify_exact_scalar_order, ExactScalarOrderProof, OrderDirection};
use terminus_store::proof::multiorder::{verify_exact_multi_order, ExactMultiOrderProof};
use terminus_store::proof::query::{
    resolve_result_values, resolve_typed_result_objects, result_id_namespaces, verify_layer_bgp,
    verify_layer_bgp_result_log_domain,
    verify_result_columns, generated_decimal_i64_field, generated_decimal_i64_value,
    generated_decimal_u64_value, AtomTerm, CanonicalResultValue, ColumnSource, LayerBgpAtom,
    LayerBgpPlan,
    LayerBgpProof, ObjectKindFilter, ObjectRangeBound, ObjectRangeSpec,
    ResultColumnDescriptor, ResultIdNamespace,
};
use terminus_store::proof::range::{RangeOp, OBJECT_RANGE_B_BITS};
use terminus_store::proof::union::{verify_bag_union, BagUnionProof};
use terminus_store::proof::{node_value_op_value, CanonicalDatatype, CanonicalObject, ProofHash};
use terminus_store::store::sync::SyncStoreLayer;
use terminus_store::ValueTriple;

// Exact PURE-RUST big rationals + the store's decimal storage decoder for the xsd:decimal/rational
// value layer (`Floor`, `Divide`, decimal operands). `num-bigint`/`num-rational` are GMP-free (the
// same backend the workspace's `rug` shim uses) so the proof compiler stays cross-compilable. The
// reference's `casting:rational_to_decimal_string/3` is itself exact integer arithmetic, so the
// `BigInt` reproduction in `rational_to_decimal_string_20` matches it bit-for-bit.
// `storage_to_decimal` is the store's own `xsd:decimal` lexical decoder (BCD storage bytes ->
// canonical decimal string), the exact inverse of the encoding a stored decimal's
// `CanonicalObject::Value` lexical carries.
use num_bigint::{BigInt, Sign};
use num_integer::Integer as _;
use num_rational::BigRational;
use num_traits::{Num as _, One as _, Pow as _, Signed as _, ToPrimitive as _, Zero as _};
use tdb_succinct::tfc::decimal::storage_to_decimal;
// The store's own arbitrary-precision integer decoder (exact inverse of `bigint_to_storage`) — used
// to render a root-authenticated `xsd:integer` (`CanonicalDatatype::BigInt`) source to base-10 for
// the number→string cast. `rug::Integer` Display is base-10, matching SWI `format ~w`.
use tdb_succinct::tfc::integer::storage_to_bigint;

use crate::expression::{ArithmeticExpression, ArithmeticValue};
use crate::misc::InRange;
use crate::query::{And, Query};
use crate::triple::Triple;
use crate::value::{DataValue, ListOrVariable, NodeValue, Value};
use terminusdb_schema::{GraphType, XSDAnySimpleType};

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

fn compiled_result_descriptors(
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

fn parse_canonical_u64_decimal(value: &str) -> std::io::Result<u64> {
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

fn parse_canonical_i64_decimal(value: &str) -> std::io::Result<i64> {
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

fn digest_bytes(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

/// Stable identity of all public WOQL interpretation metadata layered over the Store
/// plan. This prevents reusing one envelope under a different schema, projection,
/// collection, count binding, or `Distinct` interpretation.
pub fn compiled_query_digest(compiled: &CompiledBgp) -> std::io::Result<ProofHash> {
    let mut hasher = Sha256::new();
    hasher.update(b"terminusdb-woql2/query-proof-envelope/v2");
    hasher.update(relation_query_digest(&compiled.relation)?);
    match &compiled.projection {
        None => hasher.update([0]),
        Some(indices) => {
            hasher.update([1]);
            hasher.update((indices.len() as u64).to_le_bytes());
            for index in indices {
                hasher.update((*index as u64).to_le_bytes());
            }
        }
    }
    for value in [&compiled.collection, &compiled.count_variable] {
        match value {
            None => hasher.update([0]),
            Some(value) => {
                hasher.update([1]);
                digest_bytes(&mut hasher, value.as_bytes());
            }
        }
    }
    hasher.update([compiled.full_row_distinct as u8]);
    Ok(hasher.finalize().into())
}

fn relation_query_digest(relation: &CompiledRelation) -> std::io::Result<ProofHash> {
    if relation.domains.len() != relation.schema.len() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "WOQL relation domain metadata shape mismatch",
        ));
    }
    let mut hasher = Sha256::new();
    hasher.update(b"terminusdb-woql2/relation/v1");
    hasher.update((relation.schema.len() as u64).to_le_bytes());
    for name in &relation.schema {
        digest_bytes(&mut hasher, name.as_bytes());
    }
    hasher.update((relation.domains.len() as u64).to_le_bytes());
    for domain in &relation.domains {
        digest_equality_domain(&mut hasher, *domain);
    }
    for descriptor in relation_result_descriptors(relation)? {
        hasher.update([match descriptor.namespace {
            ResultIdNamespace::NodeValue => 0,
            ResultIdNamespace::Predicate => 1,
            ResultIdNamespace::GeneratedDecimalU64 => 2,
            ResultIdNamespace::GeneratedDecimalI64 => 3,
        }]);
        hasher.update([descriptor.nullable as u8]);
    }
    match &relation.statement {
        CompiledRelationStatement::LayerTripleCount => {
            hasher.update(b"/layer-triple-count");
        }
        CompiledRelationStatement::GeneratedU64Filter {
            input,
            column,
            op,
            constant,
            constant_on_left,
        } => {
            hasher.update(b"/generated-u64-filter");
            hasher.update(relation_query_digest(input)?);
            hasher.update(column.to_le_bytes());
            hasher.update([match op {
                GeneratedU64ComparisonOp::Eq => 0,
                GeneratedU64ComparisonOp::Lt => 1,
                GeneratedU64ComparisonOp::Le => 2,
                GeneratedU64ComparisonOp::Gt => 3,
                GeneratedU64ComparisonOp::Ge => 4,
            }]);
            hasher.update(constant.to_le_bytes());
            hasher.update([*constant_on_left as u8]);
        }
        CompiledRelationStatement::GeneratedI64Filter {
            input,
            column,
            op,
            constant,
            constant_on_left,
        } => {
            hasher.update(b"/generated-i64-filter");
            hasher.update(relation_query_digest(input)?);
            hasher.update(column.to_le_bytes());
            hasher.update([match op {
                GeneratedU64ComparisonOp::Eq => 0,
                GeneratedU64ComparisonOp::Lt => 1,
                GeneratedU64ComparisonOp::Le => 2,
                GeneratedU64ComparisonOp::Gt => 3,
                GeneratedU64ComparisonOp::Ge => 4,
            }]);
            hasher.update(constant.to_le_bytes());
            hasher.update([*constant_on_left as u8]);
        }
        CompiledRelationStatement::Bgp(plan) => {
            hasher.update(b"/bgp");
            hasher.update(plan_digest(plan)?);
        }
        CompiledRelationStatement::BagUnion { branches, mappings } => {
            hasher.update(b"/bag-union");
            hasher.update((branches.len() as u64).to_le_bytes());
            for (branch, mapping) in branches.iter().zip(mappings) {
                hasher.update(relation_query_digest(branch)?);
                hasher.update((mapping.len() as u64).to_le_bytes());
                for index in mapping {
                    hasher.update((*index as u64).to_le_bytes());
                }
            }
        }
        CompiledRelationStatement::BagProjection { input, indices } => {
            hasher.update(b"/bag-projection");
            hasher.update(relation_query_digest(input)?);
            hasher.update((indices.len() as u64).to_le_bytes());
            for index in indices {
                hasher.update((*index as u64).to_le_bytes());
            }
        }
        CompiledRelationStatement::Distinct { input, key_columns } => {
            hasher.update(b"/distinct-unordered");
            hasher.update(relation_query_digest(input)?);
            hasher.update((key_columns.len() as u64).to_le_bytes());
            for column in key_columns {
                hasher.update((*column as u64).to_le_bytes());
            }
        }
        CompiledRelationStatement::When { expanded } => {
            hasher.update(b"/when");
            hasher.update(relation_query_digest(expanded)?);
        }
        CompiledRelationStatement::LeftAnti {
            left,
            right,
            key_pairs,
        } => {
            hasher.update(b"/left-anti");
            hasher.update(relation_query_digest(left)?);
            hasher.update(relation_query_digest(right)?);
            digest_key_pairs(&mut hasher, key_pairs);
        }
        CompiledRelationStatement::InnerJoin { left, right, key_pairs } => {
            hasher.update(b"/inner-join");
            hasher.update(relation_query_digest(left)?);
            hasher.update(relation_query_digest(right)?);
            digest_key_pairs(&mut hasher, key_pairs);
        }
        CompiledRelationStatement::CrossRootHashJoin { left, right, left_key_col, right_key_col } => {
            hasher.update(b"/cross-root-hash-join");
            hasher.update(relation_query_digest(left)?);
            hasher.update(relation_query_digest(right)?);
            hasher.update((*left_key_col as u64).to_le_bytes());
            hasher.update((*right_key_col as u64).to_le_bytes());
        }
        CompiledRelationStatement::LeftOuter {
            left,
            right,
            key_pairs,
        } => {
            hasher.update(b"/left-outer");
            hasher.update(relation_query_digest(left)?);
            hasher.update(relation_query_digest(right)?);
            digest_key_pairs(&mut hasher, key_pairs);
        }
        CompiledRelationStatement::Equality { input, constraint } => {
            hasher.update(b"/equality");
            hasher.update(relation_query_digest(input)?);
            digest_equality_plan(&mut hasher, constraint);
        }
        CompiledRelationStatement::TupleDisequality { input, pairs } => {
            hasher.update(b"/tuple-disequality");
            hasher.update(relation_query_digest(input)?);
            digest_tuple_disequality_pairs(&mut hasher, pairs);
        }
        CompiledRelationStatement::ExactScalarOrder { input, datatype, direction } => {
            hasher.update(b"/exact-scalar-order");
            hasher.update(relation_query_digest(input)?);
            hasher.update([*datatype as u8]);
            hasher.update([match direction { OrderDirection::Ascending => 0, OrderDirection::Descending => 1 }]);
        }
        CompiledRelationStatement::ExactMultiOrder { input, key_indices, datatypes, directions } => {
            hasher.update(b"/exact-multi-order"); hasher.update(relation_query_digest(input)?);
            hasher.update((datatypes.len() as u64).to_le_bytes());
            for datatype in datatypes { hasher.update([*datatype as u8]); }
            hasher.update((key_indices.len() as u64).to_le_bytes());
            for (key_index, direction) in key_indices.iter().zip(directions) {
                hasher.update((*key_index as u64).to_le_bytes());
                hasher.update([match direction { OrderDirection::Ascending => 0, OrderDirection::Descending => 1 }]);
            }
        }
        CompiledRelationStatement::ExactOrderedSlice { input, offset, limit, once } => {
            hasher.update(if *once {
                &b"/once-ordered-slice"[..]
            } else {
                &b"/exact-ordered-slice"[..]
            });
            hasher.update(relation_query_digest(input)?);
            hasher.update(offset.to_le_bytes());
            match limit {
                None => hasher.update([0]),
                Some(limit) => {
                    hasher.update([1]);
                    hasher.update(limit.to_le_bytes());
                }
            }
        }
        CompiledRelationStatement::GroupExtrema {
            input,
            key_columns,
            value_column,
            datatype,
            kind,
        } => {
            hasher.update(b"/group-extrema");
            hasher.update(relation_query_digest(input)?);
            hasher.update((key_columns.len() as u64).to_le_bytes());
            for key_column in key_columns {
                hasher.update((*key_column as u64).to_le_bytes());
            }
            hasher.update((*value_column as u64).to_le_bytes());
            hasher.update([*datatype as u8]);
            hasher.update([match kind {
                GroupExtremaKind::Min => 0,
                GroupExtremaKind::Max => 1,
            }]);
        }
        CompiledRelationStatement::TransitiveClosure {
            edge,
            seed,
            reflexive,
            inverted,
            graph,
        } => {
            // K-INDEPENDENT digest: only the closure PATTERN (edge, seed, star/plus, direction). The
            // number of rounds K is a prover choice carried in the proof, never in this identity, so the
            // verifier reconstructs this digest from its own trusted AST without knowing K.
            hasher.update(b"/transitive-closure");
            hasher.update(plan_digest(edge)?);
            digest_canonical_object(&mut hasher, seed);
            hasher.update([*reflexive as u8]);
            hasher.update([*inverted as u8]);
            // SCHEMA-GRAPH FOUNDATION — bind the graph selector ADDITIVELY. `Instance` (every
            // pre-existing path closure) folds NOTHING, so its digest is byte-identical to today; only a
            // `Schema` closure folds a domain-separated tag. This authenticates the read-graph choice
            // (the verifier reconstructs the same tag from the trusted query) without perturbing any
            // instance-only proof identity, and makes a schema closure's identity distinct from an
            // otherwise-identical instance closure so neither can be substituted for the other.
            if let ProofGraph::Schema = graph {
                hasher.update(b"/graph-schema");
            }
        }
        CompiledRelationStatement::GroupSum {
            input,
            key_columns,
            value_column,
            datatype,
        } => {
            hasher.update(b"/group-sum");
            hasher.update(relation_query_digest(input)?);
            hasher.update((key_columns.len() as u64).to_le_bytes());
            for key_column in key_columns {
                hasher.update((*key_column as u64).to_le_bytes());
            }
            hasher.update((*value_column as u64).to_le_bytes());
            hasher.update([*datatype as u8]);
        }
        CompiledRelationStatement::GroupCount { input, key_columns } => {
            hasher.update(b"/group-count");
            hasher.update(relation_query_digest(input)?);
            hasher.update((key_columns.len() as u64).to_le_bytes());
            for key_column in key_columns {
                hasher.update((*key_column as u64).to_le_bytes());
            }
        }
        CompiledRelationStatement::CollectionCount { input } => {
            hasher.update(b"/collection-count");
            hasher.update(relation_query_digest(input)?);
        }
        CompiledRelationStatement::CollectionSum {
            input,
            template,
            datatype,
        } => {
            hasher.update(b"/collection-sum");
            hasher.update(relation_query_digest(input)?);
            match template {
                CollectionSumTemplate::Column(column) => {
                    hasher.update([0]);
                    hasher.update((*column as u64).to_le_bytes());
                }
                CollectionSumTemplate::UnsignedConstant(value) => {
                    hasher.update([1]);
                    hasher.update(value.to_le_bytes());
                }
                CollectionSumTemplate::SignedConstant(value) => {
                    hasher.update([2]);
                    hasher.update(value.to_le_bytes());
                }
            }
            hasher.update([*datatype as u8]);
        }
        CompiledRelationStatement::CollectionExtrema {
            input,
            value_column,
            datatype,
            kind,
        } => {
            hasher.update(b"/collection-extrema");
            hasher.update(relation_query_digest(input)?);
            hasher.update((*value_column as u64).to_le_bytes());
            hasher.update([*datatype as u8]);
            hasher.update([match kind {
                GroupExtremaKind::Min => 0,
                GroupExtremaKind::Max => 1,
            }]);
        }
        CompiledRelationStatement::Eval { input, expr } => {
            hasher.update(b"/eval-arithmetic-i64");
            hasher.update(relation_query_digest(input)?);
            digest_eval_expr(&mut hasher, expr);
        }
        CompiledRelationStatement::Typecast {
            input,
            source_column,
            source_datatype,
            target_datatype,
        } => {
            // Bind the source column, its pinned datatype, and the target datatype so a proof
            // for one cast (e.g. int→long) cannot be replayed as another (e.g. long→byte) or
            // over a different source column.
            hasher.update(b"/typecast-integer-width");
            hasher.update(relation_query_digest(input)?);
            hasher.update((*source_column as u64).to_le_bytes());
            hasher.update([*source_datatype as u8]);
            hasher.update([*target_datatype as u8]);
        }
        CompiledRelationStatement::Concat { input, elements, .. } => {
            // Bind the operator kind, the child, and the EXACT element sequence (order +
            // literal-vs-column + literal bytes + column index) so a proof for one concat
            // cannot be replayed as a different string op or a reordered/re-literalled concat.
            // The result variable NAME is not bound (it is a label, not security-relevant — the
            // output is fully determined by the recompute), matching Eval's digest discipline.
            hasher.update(b"/string-concat-bytes");
            hasher.update(relation_query_digest(input)?);
            hasher.update((elements.len() as u64).to_le_bytes());
            for element in elements {
                digest_concat_element(&mut hasher, element);
            }
        }
        CompiledRelationStatement::Join {
            input,
            elements,
            separator,
            ..
        } => {
            // DISTINCT tag from concat so a join can never be replayed as a concat (even a
            // single-element join, whose interposed sequence equals a one-element concat). Bind
            // the child, the exact element sequence, AND the separator under its own `/sep` tag
            // so a proof cannot be replayed as a different join (reordered/re-literalled elements
            // or a swapped separator). The result variable NAME is a label, not bound (the output
            // is fully determined by the recompute), matching Concat/Eval digest discipline.
            hasher.update(b"/string-join-bytes");
            hasher.update(relation_query_digest(input)?);
            hasher.update((elements.len() as u64).to_le_bytes());
            for element in elements {
                digest_concat_element(&mut hasher, element);
            }
            hasher.update(b"/sep");
            digest_concat_element(&mut hasher, separator);
        }
        CompiledRelationStatement::Substring {
            input,
            string_column,
            before,
            length,
            ..
        } => {
            // DISTINCT tag; bind the child, the sliced column, and the constant character offsets
            // so a proof cannot be replayed as a different string op or a different-offset slice.
            hasher.update(b"/string-substring-bytes");
            hasher.update(relation_query_digest(input)?);
            hasher.update((*string_column as u64).to_le_bytes());
            hasher.update((*before as u64).to_le_bytes());
            hasher.update((*length as u64).to_le_bytes());
        }
        CompiledRelationStatement::Pad {
            input,
            string_column,
            pad_char,
            target_len,
            ..
        } => {
            // DISTINCT tag; bind the child, the padded column, the exact pad-char bytes, and the
            // target length so a proof cannot be replayed as a different string op or a
            // different-char/different-length pad.
            hasher.update(b"/string-pad-bytes");
            hasher.update(relation_query_digest(input)?);
            hasher.update((*string_column as u64).to_le_bytes());
            hasher.update((*target_len as u64).to_le_bytes());
            digest_bytes(&mut hasher, pad_char);
        }
        CompiledRelationStatement::HashKey {
            input,
            base,
            args,
            ..
        } => {
            // DISTINCT tag; bind the child, the (raw-prepended) base, and the EXACT arg sequence
            // (order + literal-vs-column + literal bytes + column index) so a proof cannot be
            // replayed as a different string op or a reordered/re-based/re-argumented hash.
            hasher.update(b"/string-idgen-hash-sha256");
            hasher.update(relation_query_digest(input)?);
            hasher.update(b"/base");
            digest_concat_element(&mut hasher, base);
            hasher.update((args.len() as u64).to_le_bytes());
            for arg in args {
                digest_concat_element(&mut hasher, arg);
            }
        }
        CompiledRelationStatement::FloorEval { input, expr } => {
            // DISTINCT tag; bind the child + the floored expression so a floor proof cannot be
            // replayed as a different op or a different argument. The result variable NAME is a
            // label (the output is fully determined by the recompute), matching Eval's discipline.
            hasher.update(b"/decimal-floor-i64");
            hasher.update(relation_query_digest(input)?);
            digest_decimal_expr(&mut hasher, expr);
        }
        CompiledRelationStatement::RationalArith { input, expr, .. } => {
            // DISTINCT tag from every other op; bind the child + the EXACT rational expression
            // (operator, operand order, column indices, literal bytes) so a proof cannot be
            // replayed as a different arithmetic. The result variable NAME is a label (the output
            // is fully determined by the recompute), matching Concat/Eval digest discipline.
            hasher.update(b"/decimal-rational-arith");
            hasher.update(relation_query_digest(input)?);
            digest_decimal_expr(&mut hasher, expr);
        }
        CompiledRelationStatement::CalendarInt {
            input,
            date_column,
            outputs,
        } => {
            // DISTINCT tag; bind the child, the authenticated operand column, and the EXACT output
            // function sequence (order + kind) so a proof cannot be replayed as a different calendar
            // op, a different operand column, or a reordered/re-functioned output set. The result
            // variable NAMES are labels (the outputs are fully determined by the recompute), matching
            // Eval's digest discipline; the appended descriptors are already re-hashed above.
            hasher.update(b"/calendar-int-i64");
            hasher.update(relation_query_digest(input)?);
            hasher.update((*date_column as u64).to_le_bytes());
            hasher.update((outputs.len() as u64).to_le_bytes());
            for function in outputs {
                hasher.update([match function {
                    CalendarIntFn::Weekday => 0u8,
                    CalendarIntFn::WeekdaySundayStart => 1u8,
                    CalendarIntFn::IsoWeekYear => 2u8,
                    CalendarIntFn::IsoWeekWeek => 3u8,
                }]);
            }
        }
        CompiledRelationStatement::CalendarDate {
            input,
            op,
            input_column,
            ..
        } => {
            // DISTINCT tag; bind the child, the op kind, and the authenticated operand column so a
            // proof cannot be replayed as a different date op or over a different operand column. The
            // result variable NAME is a label (the output is fully determined by the recompute),
            // matching Concat/RationalArith digest discipline.
            hasher.update(b"/calendar-date");
            hasher.update(relation_query_digest(input)?);
            hasher.update([match op {
                CalendarDateFn::DayAfter => 0u8,
                CalendarDateFn::DayBefore => 1u8,
                CalendarDateFn::MonthStartDate => 2u8,
                CalendarDateFn::MonthEndDate => 3u8,
            }]);
            hasher.update((*input_column as u64).to_le_bytes());
        }
        CompiledRelationStatement::TypecastNumberToString {
            input,
            source_column,
            ..
        } => {
            // DISTINCT tag; bind the child + the authenticated numeric source column so a proof
            // cannot be replayed as a different op or over a different source column. The source
            // DATATYPE is NOT bound (it is read from the root-authenticated dictionary at recompute,
            // like Concat's operand columns); the result variable NAME is a label (the output is
            // fully determined by the recompute), matching Concat/RationalArith digest discipline.
            hasher.update(b"/typecast-num-to-string");
            hasher.update(relation_query_digest(input)?);
            hasher.update((*source_column as u64).to_le_bytes());
        }
        CompiledRelationStatement::TypecastStringToNumber {
            input,
            source_column,
            target,
            ..
        } => {
            // DISTINCT tag; bind the child, the authenticated xsd:string source column, AND the
            // numeric target discriminant so a proof for string→integer cannot be replayed as
            // string→decimal (they parse different grammars and deliver different datatypes). The
            // result variable NAME is a label (output fully determined by the recompute).
            hasher.update(b"/typecast-string-to-num");
            hasher.update(relation_query_digest(input)?);
            hasher.update((*source_column as u64).to_le_bytes());
            hasher.update([match target {
                StringNumericTarget::Integer => 0u8,
                StringNumericTarget::Decimal => 1u8,
            }]);
        }
    }
    Ok(hasher.finalize().into())
}

/// Bind the exact rational arithmetic tree into the relation's query identity so a proof cannot be
/// replayed under a different expression, operand column, literal, or operator/operand order. Each
/// node carries a DISTINCT leading tag (anti-substitution), disjoint from [`digest_eval_expr`]'s
/// tags because this whole statement already lives under a distinct `/decimal-*` statement tag.
fn digest_decimal_expr(hasher: &mut Sha256, expr: &DecimalExpr) {
    match expr {
        DecimalExpr::Column { index } => {
            hasher.update([0u8]);
            hasher.update((*index as u64).to_le_bytes());
        }
        DecimalExpr::IntegerLiteral(value) => {
            hasher.update([1u8]);
            hasher.update(value.to_le_bytes());
        }
        DecimalExpr::DecimalLiteral(lexical) => {
            hasher.update([2u8]);
            digest_bytes(hasher, lexical.as_bytes());
        }
        DecimalExpr::Plus(left, right) => {
            hasher.update([3u8]);
            digest_decimal_expr(hasher, left);
            digest_decimal_expr(hasher, right);
        }
        DecimalExpr::Minus(left, right) => {
            hasher.update([4u8]);
            digest_decimal_expr(hasher, left);
            digest_decimal_expr(hasher, right);
        }
        DecimalExpr::Times(left, right) => {
            hasher.update([5u8]);
            digest_decimal_expr(hasher, left);
            digest_decimal_expr(hasher, right);
        }
        DecimalExpr::Divide(left, right) => {
            hasher.update([6u8]);
            digest_decimal_expr(hasher, left);
            digest_decimal_expr(hasher, right);
        }
        DecimalExpr::Exp(base, exponent) => {
            hasher.update([7u8]);
            digest_decimal_expr(hasher, base);
            digest_decimal_expr(hasher, exponent);
        }
        DecimalExpr::Floor(argument) => {
            hasher.update([8u8]);
            digest_decimal_expr(hasher, argument);
        }
    }
}

/// Bind one concat element into the relation's query identity (anti-substitution): a literal's
/// exact bytes, or a column's exact index, each under a distinct tag so a literal cannot be
/// confused with a column and vice versa.
fn digest_concat_element(hasher: &mut Sha256, element: &ConcatElement) {
    match element {
        ConcatElement::Literal(bytes) => {
            hasher.update([0u8]);
            digest_bytes(hasher, bytes);
        }
        ConcatElement::Column { index } => {
            hasher.update([1u8]);
            hasher.update((*index as u64).to_le_bytes());
        }
    }
}

/// Bind the exact arithmetic tree into the relation's query identity so a proof cannot be
/// replayed under a different expression, operand column, datatype, or literal.
fn digest_eval_expr(hasher: &mut Sha256, expr: &EvalExpr) {
    match expr {
        EvalExpr::Column { index, datatype } => {
            hasher.update([0u8]);
            hasher.update((*index as u64).to_le_bytes());
            hasher.update([*datatype as u8]);
        }
        EvalExpr::Literal(value) => {
            hasher.update([1u8]);
            hasher.update(value.to_le_bytes());
        }
        EvalExpr::Plus(left, right) => {
            hasher.update([2u8]);
            digest_eval_expr(hasher, left);
            digest_eval_expr(hasher, right);
        }
        EvalExpr::Minus(left, right) => {
            hasher.update([3u8]);
            digest_eval_expr(hasher, left);
            digest_eval_expr(hasher, right);
        }
        EvalExpr::Times(left, right) => {
            hasher.update([4u8]);
            digest_eval_expr(hasher, left);
            digest_eval_expr(hasher, right);
        }
        // Distinct tags 5/6 for Div/Exp: a proof for `a div b` (or `a ** b`) cannot be replayed
        // as `a - b`, `a * b`, or with the operands swapped — the exact operator and left/right
        // operand order are bound into the relation's query identity (anti-substitution).
        EvalExpr::Div(left, right) => {
            hasher.update([5u8]);
            digest_eval_expr(hasher, left);
            digest_eval_expr(hasher, right);
        }
        EvalExpr::Exp(base, exponent) => {
            hasher.update([6u8]);
            digest_eval_expr(hasher, base);
            digest_eval_expr(hasher, exponent);
        }
    }
}

fn digest_equality_domain(hasher: &mut Sha256, domain: Option<EqualityValueDomain>) {
    match domain {
        None => hasher.update([0]),
        Some(EqualityValueDomain::Predicate) => hasher.update([1]),
        Some(EqualityValueDomain::Node) => hasher.update([2]),
        Some(EqualityValueDomain::Datatype(datatype)) => {
            hasher.update([3]);
            hasher.update([datatype as u8]);
        }
    }
}

fn digest_canonical_object(hasher: &mut Sha256, value: &CanonicalObject) {
    match value {
        CanonicalObject::Node(iri) => {
            hasher.update([0]);
            digest_bytes(hasher, iri);
        }
        CanonicalObject::Value { datatype, lexical } => {
            hasher.update([1, *datatype as u8]);
            digest_bytes(hasher, lexical);
        }
    }
}

fn digest_equality_plan(hasher: &mut Sha256, plan: &RelationEqualityPlan) {
    match plan {
        RelationEqualityPlan::ColumnConstant {
            column,
            domain,
            value,
            constant_on_left,
        } => {
            hasher.update([0]);
            hasher.update((*column as u64).to_le_bytes());
            hasher.update([*constant_on_left as u8]);
            digest_equality_domain(hasher, Some(*domain));
            digest_canonical_object(hasher, value);
        }
        RelationEqualityPlan::Columns {
            left,
            right,
            domain,
        } => {
            hasher.update([1]);
            hasher.update((*left as u64).to_le_bytes());
            hasher.update((*right as u64).to_le_bytes());
            digest_equality_domain(hasher, Some(*domain));
        }
        RelationEqualityPlan::Constants { left, right } => {
            hasher.update([2]);
            digest_canonical_object(hasher, left);
            digest_canonical_object(hasher, right);
        }
    }
}

fn digest_tuple_disequality_pairs(hasher: &mut Sha256, pairs: &[TupleDisequalityPairPlan]) {
    let node = |hasher: &mut Sha256, node: &TupleDisequalityNode| match node {
        TupleDisequalityNode::Column(column) => {
            hasher.update([0]);
            hasher.update((*column as u64).to_le_bytes());
        }
        TupleDisequalityNode::Constant(value) => {
            hasher.update([1]);
            digest_canonical_object(hasher, value);
        }
    };
    hasher.update((pairs.len() as u64).to_le_bytes());
    for pair in pairs {
        node(hasher, &pair.a0);
        node(hasher, &pair.a1);
        node(hasher, &pair.b0);
        node(hasher, &pair.b1);
    }
}

fn digest_key_pairs(hasher: &mut Sha256, key_pairs: &[(usize, usize)]) {
    hasher.update((key_pairs.len() as u64).to_le_bytes());
    for (left, right) in key_pairs {
        hasher.update((*left as u64).to_le_bytes());
        hasher.update((*right as u64).to_le_bytes());
    }
}

fn relation_result_descriptors(
    relation: &CompiledRelation,
) -> std::io::Result<Vec<ResultColumnDescriptor>> {
    match &relation.statement {
        CompiledRelationStatement::LayerTripleCount => Ok(vec![ResultColumnDescriptor {
            namespace: ResultIdNamespace::GeneratedDecimalU64,
            nullable: false,
        }]),
        CompiledRelationStatement::GeneratedU64Filter { input, .. }
        | CompiledRelationStatement::GeneratedI64Filter { input, .. } => {
            relation_result_descriptors(input)
        }
        CompiledRelationStatement::Bgp(plan) => {
            terminus_store::proof::query::result_column_descriptors(plan)
        }
        CompiledRelationStatement::TransitiveClosure { .. } => {
            // The reachable set is one nonnullable node column.
            Ok(vec![ResultColumnDescriptor {
                namespace: ResultIdNamespace::NodeValue,
                nullable: false,
            }])
        }
        CompiledRelationStatement::BagUnion { branches, mappings } => {
            let mut expected = None;
            for (branch, mapping) in branches.iter().zip(mappings) {
                let descriptors = relation_result_descriptors(branch)?;
                let mapped = mapping
                    .iter()
                    .map(|index| {
                        descriptors.get(*index).copied().ok_or_else(|| {
                            std::io::Error::new(
                                std::io::ErrorKind::InvalidData,
                                "WOQL union schema mapping out of range",
                            )
                        })
                    })
                    .collect::<std::io::Result<Vec<_>>>()?;
                if expected.as_ref().is_some_and(|prior| prior != &mapped) {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "WOQL union branch descriptor mismatch",
                    ));
                }
                expected = Some(mapped);
            }
            Ok(expected.unwrap_or_default())
        }
        CompiledRelationStatement::BagProjection { input, indices } => {
            let descriptors = relation_result_descriptors(input)?;
            indices
                .iter()
                .map(|index| {
                    descriptors.get(*index).copied().ok_or_else(|| {
                        std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "WOQL bag projection index out of range",
                        )
                    })
                })
                .collect()
        }
        CompiledRelationStatement::Distinct { input, key_columns } => {
            let descriptors = relation_result_descriptors(input)?;
            key_columns
                .iter()
                .map(|index| {
                    descriptors.get(*index).copied().ok_or_else(|| {
                        std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "WOQL distinct key column out of range",
                        )
                    })
                })
                .collect()
        }
        CompiledRelationStatement::When { expanded } => relation_result_descriptors(expanded),
        CompiledRelationStatement::LeftAnti { left, .. } => relation_result_descriptors(left),
        CompiledRelationStatement::InnerJoin { left, right, key_pairs } => {
            let mut descriptors = relation_result_descriptors(left)?;
            let right_descriptors = relation_result_descriptors(right)?;
            let shared_right: Vec<_> = key_pairs.iter().map(|pair| pair.1).collect();
            descriptors.extend(right_descriptors.into_iter().enumerate().filter_map(
                |(index, descriptor)| (!shared_right.contains(&index)).then_some(descriptor)
            ));
            Ok(descriptors)
        }
        CompiledRelationStatement::CrossRootHashJoin { right, right_key_col, .. } => {
            // Output = the INSTANCE-side (`right`) columns minus the class key column.
            let right_descriptors = relation_result_descriptors(right)?;
            Ok(right_descriptors
                .into_iter()
                .enumerate()
                .filter_map(|(index, descriptor)| (index != *right_key_col).then_some(descriptor))
                .collect())
        }
        CompiledRelationStatement::LeftOuter {
            left,
            right,
            key_pairs,
        } => {
            let mut descriptors = relation_result_descriptors(left)?;
            let right_descriptors = relation_result_descriptors(right)?;
            let shared_right: Vec<_> = key_pairs.iter().map(|pair| pair.1).collect();
            descriptors.extend(right_descriptors.into_iter().enumerate().filter_map(
                |(index, descriptor)| {
                    (!shared_right.contains(&index)).then_some(ResultColumnDescriptor {
                        namespace: descriptor.namespace,
                        nullable: true,
                    })
                },
            ));
            Ok(descriptors)
        }
        CompiledRelationStatement::Equality { input, .. } => relation_result_descriptors(input),
        CompiledRelationStatement::TupleDisequality { input, .. } => {
            relation_result_descriptors(input)
        }
        CompiledRelationStatement::ExactScalarOrder { input, .. } => {
            relation_result_descriptors(input)
        }
        CompiledRelationStatement::ExactMultiOrder { input, .. } => relation_result_descriptors(input),
        CompiledRelationStatement::ExactOrderedSlice { input, .. } => {
            relation_result_descriptors(input)
        }
        CompiledRelationStatement::GroupExtrema {
            input, key_columns, ..
        } => {
            let child = relation_result_descriptors(input)?;
            let mut result = key_columns.iter().map(|key_column| {
                child.get(*key_column).copied().ok_or_else(|| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "group-extrema key column out of range",
                    )
                })
            }).collect::<std::io::Result<Vec<_>>>()?;
            result.push(ResultColumnDescriptor {
                namespace: ResultIdNamespace::NodeValue,
                nullable: false,
            });
            Ok(result)
        }
        CompiledRelationStatement::GroupSum {
            input, key_columns, datatype, ..
        } => {
            let child = relation_result_descriptors(input)?;
            let generated = ResultColumnDescriptor {
                namespace: if matches!(
                    datatype,
                    CanonicalDatatype::Int8
                        | CanonicalDatatype::Int16
                        | CanonicalDatatype::Int32
                        | CanonicalDatatype::Int64
                ) {
                    ResultIdNamespace::GeneratedDecimalI64
                } else {
                    ResultIdNamespace::GeneratedDecimalU64
                },
                nullable: false,
            };
            let mut result = key_columns
                .iter()
                .map(|key_column| {
                    child.get(*key_column).copied().ok_or_else(|| {
                        std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "group-sum key column out of range",
                        )
                    })
                })
                .collect::<std::io::Result<Vec<_>>>()?;
            result.push(generated);
            Ok(result)
        }
        CompiledRelationStatement::GroupCount { input, key_columns } => {
            let child = relation_result_descriptors(input)?;
            let mut result = key_columns
                .iter()
                .map(|key_column| {
                    child.get(*key_column).copied().ok_or_else(|| {
                        std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "group-count key column out of range",
                        )
                    })
                })
                .collect::<std::io::Result<Vec<_>>>()?;
            result.push(ResultColumnDescriptor {
                namespace: ResultIdNamespace::GeneratedDecimalU64,
                nullable: false,
            });
            Ok(result)
        }
        CompiledRelationStatement::CollectionCount { .. } => Ok(vec![ResultColumnDescriptor {
            namespace: ResultIdNamespace::GeneratedDecimalU64,
            nullable: false,
        }]),
        CompiledRelationStatement::CollectionSum { datatype, .. } => {
            Ok(vec![ResultColumnDescriptor {
                namespace: if matches!(
                    datatype,
                    CanonicalDatatype::Int8
                        | CanonicalDatatype::Int16
                        | CanonicalDatatype::Int32
                        | CanonicalDatatype::Int64
                ) {
                    ResultIdNamespace::GeneratedDecimalI64
                } else {
                    ResultIdNamespace::GeneratedDecimalU64
                },
                nullable: false,
            }])
        }
        CompiledRelationStatement::CollectionExtrema {
            input, value_column, ..
        } => {
            let child = relation_result_descriptors(input)?;
            Ok(vec![*child.get(*value_column).ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "collection-extrema value column out of range",
                )
            })?])
        }
        CompiledRelationStatement::Eval { input, .. } => {
            // Every child column is carried through unchanged; the appended derived column
            // is a signed generated integer (Eval accumulates in signed i128 and narrows to
            // the i64 generated domain, so `Minus` results and negatives are representable).
            let mut descriptors = relation_result_descriptors(input)?;
            descriptors.push(ResultColumnDescriptor {
                namespace: ResultIdNamespace::GeneratedDecimalI64,
                nullable: false,
            });
            Ok(descriptors)
        }
        CompiledRelationStatement::FloorEval { input, .. } => {
            // Identical shape to `Eval`: child columns pass through, and `floor(x)` appends ONE
            // signed generated-integer column (a floored rational is integral; negatives are
            // representable in the i64 generated domain).
            let mut descriptors = relation_result_descriptors(input)?;
            descriptors.push(ResultColumnDescriptor {
                namespace: ResultIdNamespace::GeneratedDecimalI64,
                nullable: false,
            });
            Ok(descriptors)
        }
        CompiledRelationStatement::Typecast {
            input,
            target_datatype,
            ..
        } => {
            // Child columns pass through; the appended cast column uses the SIGNED generated
            // namespace for signed targets and the UNSIGNED one for unsigned targets (an
            // unsigned target like xsd:unsignedLong can hold values above `i64::MAX`).
            let mut descriptors = relation_result_descriptors(input)?;
            descriptors.push(ResultColumnDescriptor {
                namespace: integer_target_namespace(*target_datatype)?,
                nullable: false,
            });
            Ok(descriptors)
        }
        CompiledRelationStatement::Concat { input, .. } => {
            // The committed result columns are exactly the child's operand columns; the concat
            // result is TERMINAL/revealed and is NOT a committed column (a synthesized string
            // does not encode into a field element, and nothing downstream consumes it). So the
            // descriptors are the child's, unchanged — keeping schema/descriptor/column shapes
            // aligned for the generic resolve path.
            relation_result_descriptors(input)
        }
        // Join/Substring, like Concat, carry the child's committed columns through VERBATIM (their
        // synthesized result is terminal/revealed, not a committed column) — descriptors are the
        // child's, unchanged.
        CompiledRelationStatement::Join { input, .. }
        | CompiledRelationStatement::Substring { input, .. }
        // Pad/HashKey, like Concat, carry the child's committed columns through VERBATIM (their
        // synthesized result is terminal/revealed, not a committed column).
        | CompiledRelationStatement::Pad { input, .. }
        | CompiledRelationStatement::HashKey { input, .. }
        // RationalArith, like Concat, carries the child's committed columns through VERBATIM — its
        // synthesized xsd:decimal result is terminal/revealed, not a committed column.
        | CompiledRelationStatement::RationalArith { input, .. } => {
            relation_result_descriptors(input)
        }
        CompiledRelationStatement::CalendarInt { input, outputs, .. } => {
            // Child columns pass through; ONE signed generated-integer column is appended PER output
            // (weekday/iso-week values are small positive integers, but the signed domain matches
            // Eval's and keeps the descriptor family uniform).
            let mut descriptors = relation_result_descriptors(input)?;
            for _ in outputs {
                descriptors.push(ResultColumnDescriptor {
                    namespace: ResultIdNamespace::GeneratedDecimalI64,
                    nullable: false,
                });
            }
            Ok(descriptors)
        }
        // CalendarDate, like Concat/RationalArith, carries the child's committed columns through
        // VERBATIM — its synthesized xsd:date result is terminal/revealed, not a committed column.
        CompiledRelationStatement::CalendarDate { input, .. }
        // TypecastNumberToString/TypecastStringToNumber, like Concat/RationalArith, carry the child's
        // committed columns through VERBATIM — the cast result is terminal/revealed, not committed.
        | CompiledRelationStatement::TypecastNumberToString { input, .. }
        | CompiledRelationStatement::TypecastStringToNumber { input, .. } => {
            relation_result_descriptors(input)
        }
    }
}

fn relation_proof_plan(
    relation: &CompiledRelation,
    query_digest: ProofHash,
) -> std::io::Result<RelationProofPlan> {
    let child = |relation: &CompiledRelation| {
        let digest = relation_query_digest(relation)?;
        relation_proof_plan(relation, digest)
    };
    let kind = match &relation.statement {
        CompiledRelationStatement::LayerTripleCount => RelationProofPlanKind::LayerTripleCount,
        CompiledRelationStatement::GeneratedU64Filter {
            input,
            column,
            op,
            constant,
            constant_on_left,
        } => RelationProofPlanKind::GeneratedU64Filter {
            input: Box::new(child(input)?),
            column: *column,
            op: *op,
            constant: *constant,
            constant_on_left: *constant_on_left,
        },
        CompiledRelationStatement::GeneratedI64Filter {
            input,
            column,
            op,
            constant,
            constant_on_left,
        } => RelationProofPlanKind::GeneratedI64Filter {
            input: Box::new(child(input)?),
            column: *column,
            op: *op,
            constant: *constant,
            constant_on_left: *constant_on_left,
        },
        CompiledRelationStatement::Bgp(plan) => RelationProofPlanKind::Bgp(plan.clone()),
        CompiledRelationStatement::BagUnion { branches, mappings } => {
            RelationProofPlanKind::BagUnion {
                branches: branches.iter().map(child).collect::<std::io::Result<_>>()?,
                mappings: mappings.clone(),
            }
        }
        CompiledRelationStatement::BagProjection { input, indices } => {
            RelationProofPlanKind::BagProjection {
                input: Box::new(child(input)?),
                indices: indices.clone(),
            }
        }
        CompiledRelationStatement::Distinct { input, key_columns } => {
            RelationProofPlanKind::Distinct {
                input: Box::new(child(input)?),
                key_columns: key_columns.clone(),
            }
        }
        CompiledRelationStatement::When { expanded } => {
            return relation_proof_plan(expanded, query_digest);
        }
        CompiledRelationStatement::LeftAnti {
            left,
            right,
            key_pairs,
        } => RelationProofPlanKind::LeftAnti {
            left: Box::new(child(left)?),
            right: Box::new(child(right)?),
            key_pairs: key_pairs.clone(),
        },
        CompiledRelationStatement::InnerJoin { left, right, key_pairs } => {
            RelationProofPlanKind::InnerJoin {
                left: Box::new(child(left)?), right: Box::new(child(right)?),
                key_pairs: key_pairs.clone(),
            }
        }
        CompiledRelationStatement::CrossRootHashJoin { left, right, left_key_col, right_key_col } => {
            RelationProofPlanKind::CrossRootHashJoin {
                left: Box::new(child(left)?),
                right: Box::new(child(right)?),
                left_key_col: *left_key_col,
                right_key_col: *right_key_col,
            }
        }
        CompiledRelationStatement::Equality { input, constraint } => {
            RelationProofPlanKind::Equality {
                input: Box::new(child(input)?),
                constraint: constraint.clone(),
            }
        }
        CompiledRelationStatement::TupleDisequality { input, pairs } => {
            RelationProofPlanKind::ExactTupleDisequality {
                input: Box::new(child(input)?),
                pairs: pairs.clone(),
            }
        }
        CompiledRelationStatement::ExactScalarOrder { input, datatype, direction } => {
            RelationProofPlanKind::ExactScalarOrder {
                input: Box::new(child(input)?),
                datatype: *datatype,
                direction: *direction,
            }
        }
        CompiledRelationStatement::ExactMultiOrder { input, key_indices, datatypes, directions } => {
            RelationProofPlanKind::ExactMultiOrder { input: Box::new(child(input)?), datatypes: datatypes.clone(), key_indices: key_indices.clone(), directions: directions.clone() }
        }
        CompiledRelationStatement::ExactOrderedSlice { input, offset, limit, .. } => {
            RelationProofPlanKind::ExactOrderedSlice {
                input: Box::new(child(input)?),
                offset: *offset,
                limit: *limit,
            }
        }
        CompiledRelationStatement::LeftOuter {
            left,
            right,
            key_pairs,
        } => RelationProofPlanKind::LeftOuter {
            left: Box::new(child(left)?),
            right: Box::new(child(right)?),
            key_pairs: key_pairs.clone(),
        },
        CompiledRelationStatement::GroupExtrema {
            input,
            key_columns,
            value_column,
            datatype,
            kind,
        } => RelationProofPlanKind::GroupExtrema {
            input: Box::new(child(input)?),
            key_columns: key_columns.clone(),
            value_column: *value_column,
            datatype: *datatype,
            kind: *kind,
        },
        CompiledRelationStatement::TransitiveClosure {
            edge,
            seed,
            reflexive,
            inverted,
            // The graph selector is NOT carried into the store's `RelationProofPlanKind`: the store
            // recursion is graph-blind and reads whichever single `LayerCommitment` (instance or schema)
            // the prove/verify ENTRY selects for this query (see [`relation_graph`]). The selector's
            // soundness is anchored by (a) its fold into `relation_query_digest` above and (b) the
            // verifier picking the matching commitment + `commitment_root` from the trusted query.
            graph: _,
        } => RelationProofPlanKind::TransitiveClosure {
            edge: edge.clone(),
            seed: seed.clone(),
            reflexive: *reflexive,
            inverted: *inverted,
        },
        CompiledRelationStatement::GroupSum {
            input,
            key_columns,
            value_column,
            datatype,
        } => RelationProofPlanKind::GroupSum {
            input: Box::new(child(input)?),
            key_columns: key_columns.clone(),
            value_column: *value_column,
            datatype: *datatype,
        },
        CompiledRelationStatement::GroupCount { input, key_columns } => {
            RelationProofPlanKind::GroupCount {
                input: Box::new(child(input)?),
                key_columns: key_columns.clone(),
            }
        }
        CompiledRelationStatement::CollectionCount { input } => {
            RelationProofPlanKind::CollectionCount {
                input: Box::new(child(input)?),
            }
        }
        CompiledRelationStatement::CollectionSum {
            input,
            template,
            datatype,
        } => RelationProofPlanKind::CollectionSum {
            input: Box::new(child(input)?),
            template: *template,
            datatype: *datatype,
        },
        CompiledRelationStatement::CollectionExtrema {
            input,
            value_column,
            datatype,
            kind,
        } => RelationProofPlanKind::CollectionExtrema {
            input: Box::new(child(input)?),
            value_column: *value_column,
            datatype: *datatype,
            kind: *kind,
        },
        // Eval is a woql2-side verifier-recompute layered OVER a Store-proven child; it has
        // no Store `RelationProofPlanKind`. `CompiledBgp::prove_with_context` intercepts a
        // top-level Eval and proves its child through the Store, so this arm is never reached
        // for a well-formed plan. Reject closed rather than silently mis-plan.
        CompiledRelationStatement::Eval { .. } => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "WOQL Eval relations are proved by the query wrapper, not the Store recursive planner",
            ))
        }
        // Typecast, like Eval, is a woql2-side verifier-recompute layered OVER a Store-proven
        // child; `CompiledBgp::prove_with_context` intercepts a top-level Typecast, so this arm
        // is never reached for a well-formed plan. Reject closed rather than silently mis-plan.
        CompiledRelationStatement::Typecast { .. } => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "WOQL Typecast relations are proved by the query wrapper, not the Store recursive planner",
            ))
        }
        // Concat, like Eval/Typecast, is a woql2-side verifier-recompute over a Store-proven
        // child; `CompiledBgp::prove_with_context` intercepts a top-level Concat, so this arm is
        // never reached for a well-formed plan. Reject closed rather than silently mis-plan.
        CompiledRelationStatement::Concat { .. } => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "WOQL Concat relations are proved by the query wrapper, not the Store recursive planner",
            ))
        }
        // Join/Substring are woql2-side verifier-recompute ops (like Concat): the top-level
        // `prove_with_context` intercepts them, so this recursive-planner arm is never reached for
        // a well-formed plan. Reject closed rather than silently mis-plan.
        CompiledRelationStatement::Join { .. } => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "WOQL Join relations are proved by the query wrapper, not the Store recursive planner",
            ))
        }
        CompiledRelationStatement::Substring { .. } => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "WOQL Substring relations are proved by the query wrapper, not the Store recursive planner",
            ))
        }
        // Pad/HashKey are woql2-side verifier-recompute ops (like Concat): the top-level
        // `prove_with_context` intercepts them, so this recursive-planner arm is never reached for
        // a well-formed plan. Reject closed rather than silently mis-plan.
        CompiledRelationStatement::Pad { .. } => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "WOQL Pad relations are proved by the query wrapper, not the Store recursive planner",
            ))
        }
        CompiledRelationStatement::HashKey { .. } => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "WOQL HashKey relations are proved by the query wrapper, not the Store recursive planner",
            ))
        }
        // FloorEval/RationalArith are woql2-side verifier-recompute ops (like Eval/Concat): the
        // top-level `prove_with_context` intercepts them, so this recursive-planner arm is never
        // reached for a well-formed plan. Reject closed rather than silently mis-plan.
        CompiledRelationStatement::FloorEval { .. } => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "WOQL floor relations are proved by the query wrapper, not the Store recursive planner",
            ))
        }
        CompiledRelationStatement::RationalArith { .. } => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "WOQL rational-arithmetic relations are proved by the query wrapper, not the Store recursive planner",
            ))
        }
        // Calendar ops are woql2-side verifier-recompute layered OVER a Store-proven child; the
        // top-level `prove_with_context` intercepts them, so these recursive-planner arms are never
        // reached for a well-formed plan. Reject closed rather than silently mis-plan.
        CompiledRelationStatement::CalendarInt { .. } => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "WOQL calendar-integer relations are proved by the query wrapper, not the Store recursive planner",
            ))
        }
        // Number↔string Typecasts are woql2-side verifier-recompute ops (like Concat): the top-level
        // `prove_with_context` intercepts them, so these recursive-planner arms are never reached for
        // a well-formed plan. Reject closed rather than silently mis-plan.
        CompiledRelationStatement::TypecastNumberToString { .. } => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "WOQL number→string Typecast relations are proved by the query wrapper, not the Store recursive planner",
            ))
        }
        CompiledRelationStatement::TypecastStringToNumber { .. } => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "WOQL string→number Typecast relations are proved by the query wrapper, not the Store recursive planner",
            ))
        }
        CompiledRelationStatement::CalendarDate { .. } => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "WOQL calendar-date relations are proved by the query wrapper, not the Store recursive planner",
            ))
        }
    };
    Ok(RelationProofPlan { query_digest, kind })
}

fn generated_u64_comparison(
    value: u64,
    op: GeneratedU64ComparisonOp,
    constant: u64,
    constant_on_left: bool,
) -> bool {
    let (left, right) = if constant_on_left {
        (constant, value)
    } else {
        (value, constant)
    };
    match op {
        GeneratedU64ComparisonOp::Eq => left == right,
        GeneratedU64ComparisonOp::Lt => left < right,
        GeneratedU64ComparisonOp::Le => left <= right,
        GeneratedU64ComparisonOp::Gt => left > right,
        GeneratedU64ComparisonOp::Ge => left >= right,
    }
}

fn expected_generated_u64_filter_columns(
    input: &ProvedBgp,
    column: usize,
    op: GeneratedU64ComparisonOp,
    constant: u64,
    constant_on_left: bool,
) -> std::io::Result<Vec<Vec<ArkFr>>> {
    let source = input.result_columns.get(column).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "generated-u64 comparison column is out of range",
        )
    })?;
    if source.len() < input.result_len {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "generated-u64 comparison input column is short",
        ));
    }
    let mut keep = Vec::new();
    for (row, field) in source.iter().take(input.result_len).enumerate() {
        let value = generated_decimal_u64_value(*field)?;
        if generated_u64_comparison(value, op, constant, constant_on_left) {
            keep.push(row);
        }
    }
    input
        .result_columns
        .iter()
        .map(|input_column| {
            keep.iter()
                .map(|row| {
                    input_column.get(*row).copied().ok_or_else(|| {
                        std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "generated-u64 comparison input column is short",
                        )
                    })
                })
                .collect()
        })
        .collect()
}

fn expected_generated_i64_filter_columns(
    input: &ProvedBgp,
    column: usize,
    op: GeneratedU64ComparisonOp,
    constant: i64,
    constant_on_left: bool,
) -> std::io::Result<Vec<Vec<ArkFr>>> {
    let source = input.result_columns.get(column).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "generated-i64 comparison column is out of range",
        )
    })?;
    if source.len() < input.result_len {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "generated-i64 comparison input column is short",
        ));
    }
    let mut keep = Vec::new();
    for (row, field) in source.iter().take(input.result_len).enumerate() {
        let value = generated_decimal_i64_value(*field)?;
        let (left, right) = if constant_on_left {
            (constant, value)
        } else {
            (value, constant)
        };
        let selected = match op {
            GeneratedU64ComparisonOp::Eq => left == right,
            GeneratedU64ComparisonOp::Lt => left < right,
            GeneratedU64ComparisonOp::Le => left <= right,
            GeneratedU64ComparisonOp::Gt => left > right,
            GeneratedU64ComparisonOp::Ge => left >= right,
        };
        if selected {
            keep.push(row);
        }
    }
    input
        .result_columns
        .iter()
        .map(|input_column| {
            keep.iter()
                .map(|row| {
                    input_column.get(*row).copied().ok_or_else(|| {
                        std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "generated-i64 comparison input column is short",
                        )
                    })
                })
                .collect()
        })
        .collect()
}

#[derive(Clone, Copy)]
enum GroupSumValue {
    Unsigned(u64),
    Signed(i128),
}

impl GroupSumValue {
    fn zero(datatype: CanonicalDatatype) -> std::io::Result<Self> {
        match datatype {
            CanonicalDatatype::UInt8
            | CanonicalDatatype::UInt16
            | CanonicalDatatype::UInt32
            | CanonicalDatatype::UInt64 => Ok(Self::Unsigned(0)),
            CanonicalDatatype::Int8
            | CanonicalDatatype::Int16
            | CanonicalDatatype::Int32
            | CanonicalDatatype::Int64 => Ok(Self::Signed(0)),
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "group-sum datatype is outside the integral subset",
            )),
        }
    }

    fn checked_add(self, rhs: Self) -> std::io::Result<Self> {
        match (self, rhs) {
            (Self::Unsigned(left), Self::Unsigned(right)) => left
                .checked_add(right)
                .map(Self::Unsigned)
                .ok_or_else(|| std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "group-sum unsigned result exceeds u64",
                )),
            (Self::Signed(left), Self::Signed(right)) => left
                .checked_add(right)
                .map(Self::Signed)
                .ok_or_else(|| std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "group-sum signed accumulator exceeds i128",
                )),
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "group-sum mixes signed and unsigned values",
            )),
        }
    }

    fn field(self) -> std::io::Result<ArkFr> {
        match self {
            Self::Unsigned(value) => Ok(ArkFr::from_u64(value)),
            Self::Signed(value) => Ok(generated_decimal_i64_field(i64::try_from(value).map_err(
                |_| std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "group-sum signed result exceeds i64",
                ),
            )?)),
        }
    }

    fn cmp_same_domain(self, rhs: Self) -> std::io::Result<std::cmp::Ordering> {
        match (self, rhs) {
            (Self::Unsigned(left), Self::Unsigned(right)) => Ok(left.cmp(&right)),
            (Self::Signed(left), Self::Signed(right)) => Ok(left.cmp(&right)),
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "collection-extrema mixes signed and unsigned values",
            )),
        }
    }
}

fn integral_group_sum_value(
    value: &CanonicalResultValue,
    datatype: CanonicalDatatype,
) -> std::io::Result<GroupSumValue> {
    let CanonicalResultValue::Object(CanonicalObject::Value {
        datatype: actual,
        lexical,
    }) = value
    else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "group-sum input is not a typed Store literal",
        ));
    };
    if *actual != datatype {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "group-sum input has the wrong exact datatype",
        ));
    }
    Ok(match datatype {
        CanonicalDatatype::UInt8 if lexical.len() == 1 => GroupSumValue::Unsigned(u64::from(lexical[0])),
        CanonicalDatatype::UInt16 if lexical.len() == 2 => {
            GroupSumValue::Unsigned(u64::from(u16::from_be_bytes(lexical.as_slice().try_into().unwrap())))
        }
        CanonicalDatatype::UInt32 if lexical.len() == 4 => {
            GroupSumValue::Unsigned(u64::from(u32::from_be_bytes(lexical.as_slice().try_into().unwrap())))
        }
        CanonicalDatatype::UInt64 if lexical.len() == 8 => {
            GroupSumValue::Unsigned(u64::from_be_bytes(lexical.as_slice().try_into().unwrap()))
        }
        CanonicalDatatype::Int8 if lexical.len() == 1 => {
            GroupSumValue::Signed(i128::from((lexical[0] ^ 0x80) as i8))
        }
        CanonicalDatatype::Int16 if lexical.len() == 2 => {
            let encoded = u16::from_be_bytes(lexical.as_slice().try_into().unwrap());
            GroupSumValue::Signed(i128::from((encoded ^ (1 << 15)) as i16))
        }
        CanonicalDatatype::Int32 if lexical.len() == 4 => {
            let encoded = u32::from_be_bytes(lexical.as_slice().try_into().unwrap());
            GroupSumValue::Signed(i128::from((encoded ^ (1 << 31)) as i32))
        }
        CanonicalDatatype::Int64 if lexical.len() == 8 => {
            let encoded = u64::from_be_bytes(lexical.as_slice().try_into().unwrap());
            GroupSumValue::Signed(i128::from((encoded ^ (1 << 63)) as i64))
        }
        CanonicalDatatype::UInt8
        | CanonicalDatatype::UInt16
        | CanonicalDatatype::UInt32
        | CanonicalDatatype::UInt64
        | CanonicalDatatype::Int8
        | CanonicalDatatype::Int16
        | CanonicalDatatype::Int32
        | CanonicalDatatype::Int64 => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "group-sum input has a noncanonical lexical width",
            ))
        }
        _ => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "group-sum datatype is outside the integral subset",
            ))
        }
    })
}

fn expected_group_sum_columns(
    input: &ProvedBgp,
    input_plan: &CompiledRelation,
    key_columns: &[usize],
    value_column: usize,
    datatype: CanonicalDatatype,
    layer: &impl VerifierLayer,
    expected_root: ProofHash,
) -> std::io::Result<Vec<Vec<ArkFr>>> {
    let descriptors = relation_result_descriptors(input_plan)?;
    let values = resolve_result_values(
        layer,
        expected_root,
        &input.result_columns,
        &descriptors,
    )?;
    let numbers = values.get(value_column).ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, "group-sum value out of range")
    })?;
    if numbers.len() != input.result_len {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "group-sum input length mismatch",
        ));
    }
    if key_columns.is_empty() {
        if input.result_len == 0 {
            return Ok(vec![Vec::new()]);
        }
        let mut sum = GroupSumValue::zero(datatype)?;
        for number in numbers {
            sum = sum.checked_add(integral_group_sum_value(number, datatype)?)?;
        }
        return Ok(vec![vec![sum.field()?]]);
    }
    let keys = key_columns
        .iter()
        .map(|key_column| {
            let keys = values.get(*key_column).ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, "group-sum key out of range")
            })?;
            let fields = input.result_columns.get(*key_column).ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, "group-sum key field out of range")
            })?;
            if keys.len() != input.result_len || fields.len() != input.result_len {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "group-sum key length mismatch",
                ));
            }
            Ok((keys, fields))
        })
        .collect::<std::io::Result<Vec<_>>>()?;
    let mut sums = std::collections::BTreeMap::<Vec<Vec<u8>>, (Vec<ArkFr>, GroupSumValue)>::new();
    for row in 0..input.result_len {
        let mut canonical = Vec::with_capacity(keys.len());
        let mut fields = Vec::with_capacity(keys.len());
        for (values, key_fields) in &keys {
            let CanonicalResultValue::Object(CanonicalObject::Node(iri)) = &values[row] else {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "group-sum key is not a node IRI",
                ));
            };
            canonical.push(iri.clone());
            fields.push(key_fields[row]);
        }
        let value = integral_group_sum_value(&numbers[row], datatype)?;
        let entry = sums
            .entry(canonical)
            .or_insert((fields.clone(), GroupSumValue::zero(datatype)?));
        if entry.0 != fields {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "group-sum canonical node tuple maps to multiple Store IDs",
            ));
        }
        entry.1 = entry.1.checked_add(value)?;
    }
    let mut output = vec![Vec::with_capacity(sums.len()); key_columns.len() + 1];
    for (fields, sum) in sums.values() {
        for (column, field) in output.iter_mut().zip(fields) {
            column.push(*field);
        }
        output[key_columns.len()].push(sum.field()?);
    }
    Ok(output)
}

fn expected_group_extrema_columns(
    input: &ProvedBgp,
    input_plan: &CompiledRelation,
    key_columns: &[usize],
    value_column: usize,
    datatype: CanonicalDatatype,
    kind: GroupExtremaKind,
    layer: &impl VerifierLayer,
    expected_root: ProofHash,
) -> std::io::Result<Vec<Vec<ArkFr>>> {
    let descriptors = relation_result_descriptors(input_plan)?;
    let values = resolve_result_values(layer, expected_root, &input.result_columns, &descriptors)?;
    let numbers = values.get(value_column).ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, "group-extrema value out of range")
    })?;
    let fields = input.result_columns.get(value_column).ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, "group-extrema field out of range")
    })?;
    if numbers.len() != input.result_len || fields.len() != input.result_len {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "group-extrema input length mismatch",
        ));
    }
    let select = |rows: &[usize]| -> std::io::Result<Option<ArkFr>> {
        let mut selected: Option<(ArkFr, GroupSumValue)> = None;
        for row in rows {
            let value = integral_group_sum_value(&numbers[*row], datatype)?;
            let replace = match &selected {
                None => true,
                Some((_, current)) => match kind {
                    GroupExtremaKind::Min => value.cmp_same_domain(*current)? == std::cmp::Ordering::Less,
                    GroupExtremaKind::Max => value.cmp_same_domain(*current)? == std::cmp::Ordering::Greater,
                },
            };
            if replace {
                selected = Some((fields[*row], value));
            }
        }
        Ok(selected.map(|(field, _)| field))
    };
    if key_columns.is_empty() {
        let rows = (0..input.result_len).collect::<Vec<_>>();
        return Ok(vec![select(&rows)?.into_iter().collect()]);
    }
    let keys = key_columns.iter().map(|key_column| {
        let values = values.get(*key_column).ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, "group-extrema key out of range")
        })?;
        let fields = input.result_columns.get(*key_column).ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, "group-extrema key field out of range")
        })?;
        if values.len() != input.result_len || fields.len() != input.result_len {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "group-extrema key length mismatch",
            ));
        }
        Ok((values, fields))
    }).collect::<std::io::Result<Vec<_>>>()?;
    let mut groups = std::collections::BTreeMap::<Vec<Vec<u8>>, (Vec<ArkFr>, Vec<usize>)>::new();
    for row in 0..input.result_len {
        let mut canonical = Vec::with_capacity(keys.len());
        let mut key_fields = Vec::with_capacity(keys.len());
        for (key_values, fields) in &keys {
            let CanonicalResultValue::Object(CanonicalObject::Node(iri)) = &key_values[row] else {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "group-extrema key is not a node IRI",
                ));
            };
            canonical.push(iri.clone());
            key_fields.push(fields[row]);
        }
        let entry = groups.entry(canonical).or_insert((key_fields.clone(), Vec::new()));
        if entry.0 != key_fields {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "group-extrema canonical node tuple maps to multiple Store IDs",
            ));
        }
        entry.1.push(row);
    }
    let mut output = vec![Vec::with_capacity(groups.len()); key_columns.len() + 1];
    for (key_fields, rows) in groups.values() {
        for (column, field) in output.iter_mut().zip(key_fields) {
            column.push(*field);
        }
        output[key_columns.len()].push(select(rows)?.ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, "empty group-extrema group")
        })?);
    }
    Ok(output)
}

fn expected_group_count_columns(
    input: &ProvedBgp,
    input_plan: &CompiledRelation,
    key_columns: &[usize],
    layer: &impl VerifierLayer,
    expected_root: ProofHash,
) -> std::io::Result<Vec<Vec<ArkFr>>> {
    if key_columns.is_empty() {
        return expected_cardinality_column(input.result_len, false);
    }
    let descriptors = relation_result_descriptors(input_plan)?;
    let values = resolve_result_values(
        layer,
        expected_root,
        &input.result_columns,
        &descriptors,
    )?;
    let keys = key_columns
        .iter()
        .map(|key_column| {
            let values = values.get(*key_column).ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, "group-count key out of range")
            })?;
            let fields = input.result_columns.get(*key_column).ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "group-count key field out of range",
                )
            })?;
            if values.len() != input.result_len || fields.len() != input.result_len {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "group-count key length mismatch",
                ));
            }
            Ok((values, fields))
        })
        .collect::<std::io::Result<Vec<_>>>()?;
    let mut groups = std::collections::BTreeMap::<Vec<Vec<u8>>, (Vec<ArkFr>, u64)>::new();
    for row in 0..input.result_len {
        let mut canonical = Vec::with_capacity(keys.len());
        let mut fields = Vec::with_capacity(keys.len());
        for (values, key_fields) in &keys {
            let CanonicalResultValue::Object(CanonicalObject::Node(iri)) = &values[row] else {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "group-count key is not a node IRI",
                ));
            };
            canonical.push(iri.clone());
            fields.push(key_fields[row]);
        }
        let entry = groups.entry(canonical).or_insert((fields.clone(), 0));
        if entry.0 != fields {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "group-count canonical node tuple maps to multiple Store IDs",
            ));
        }
        entry.1 = entry.1.checked_add(1).ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, "group-count exceeds u64")
        })?;
    }
    let mut output = vec![Vec::with_capacity(groups.len()); key_columns.len() + 1];
    for (fields, count) in groups.values() {
        for (column, field) in output.iter_mut().zip(fields) {
            column.push(*field);
        }
        output[key_columns.len()].push(ArkFr::from_u64(*count));
    }
    Ok(output)
}

/// Decode ONE root-authenticated operand value to its exact integer, as `i128`.
///
/// Reuses the audited fixed-width lexical decoder [`integral_group_sum_value`] (the same one
/// Sum/GroupSum use). That decoder also asserts the AUTHENTICATED dictionary value carries
/// exactly `datatype`, so a compiler datatype claim that disagrees with the trusted root
/// value rejects closed here rather than decoding a different integer. Every supported
/// fixed-width integer (`u8..u64`, `i8..i64`) fits `i128` exactly.
fn eval_operand_i128(
    value: &CanonicalResultValue,
    datatype: CanonicalDatatype,
) -> std::io::Result<i128> {
    Ok(match integral_group_sum_value(value, datatype)? {
        GroupSumValue::Unsigned(v) => i128::from(v),
        GroupSumValue::Signed(v) => v,
    })
}

/// Re-evaluate the arithmetic tree for ONE row over the authenticated operand columns.
///
/// All accumulation is CHECKED `i128`: an intermediate that escapes `i128` rejects closed
/// (`i128` cannot overflow silently here — every operand is a fixed-width integer and the
/// tree is bounded, but `Times` chains are still guarded). The declared-width narrowing to
/// `i64` happens once at the top (see [`expected_eval_columns`]).
fn eval_expr_row(
    expr: &EvalExpr,
    values: &[Vec<CanonicalResultValue>],
    row: usize,
) -> std::io::Result<i128> {
    match expr {
        EvalExpr::Column { index, datatype } => {
            let column = values.get(*index).ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "WOQL Eval operand column out of range",
                )
            })?;
            let value = column.get(row).ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "WOQL Eval operand column is short",
                )
            })?;
            eval_operand_i128(value, *datatype)
        }
        EvalExpr::Literal(value) => Ok(*value),
        EvalExpr::Plus(left, right) => eval_expr_row(left, values, row)?
            .checked_add(eval_expr_row(right, values, row)?)
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "WOQL Eval addition overflows the i128 accumulator",
                )
            }),
        EvalExpr::Minus(left, right) => eval_expr_row(left, values, row)?
            .checked_sub(eval_expr_row(right, values, row)?)
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "WOQL Eval subtraction overflows the i128 accumulator",
                )
            }),
        EvalExpr::Times(left, right) => eval_expr_row(left, values, row)?
            .checked_mul(eval_expr_row(right, values, row)?)
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "WOQL Eval multiplication overflows the i128 accumulator",
                )
            }),
        EvalExpr::Div(left, right) => {
            let dividend = eval_expr_row(left, values, row)?;
            let divisor = eval_expr_row(right, values, row)?;
            // Div-by-zero: Prolog `X div 0` THROWS (`evaluation_error(zero_divisor)`), so the
            // WOQL query fails. FAIL CLOSED for this row (do NOT drop it — that would silently
            // change the relation). Static zero literals are also rejected at compile time.
            if divisor == 0 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "WOQL Eval Div by zero: Prolog `div/2` throws, so the query fails closed",
                ));
            }
            // FLOOR division (SWI-Prolog `div/2`, rounds toward −∞). Rust `/` truncates toward
            // zero, so correct the quotient down by one when the truncated remainder is nonzero
            // and its sign differs from the divisor's (the exact floor-vs-truncate gap; this is
            // NOT `div_euclid`, which would round `7 div -2` to −3 instead of the correct −4).
            // `checked_div`/`checked_rem` also fail closed on the sole i128 overflow case
            // (`i128::MIN / -1`), unreachable for fixed-width operands but guarded anyway.
            let truncated = dividend.checked_div(divisor).ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "WOQL Eval Div overflows the i128 accumulator",
                )
            })?;
            let remainder = dividend.checked_rem(divisor).ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "WOQL Eval Div overflows the i128 accumulator",
                )
            })?;
            if remainder != 0 && ((remainder < 0) != (divisor < 0)) {
                truncated.checked_sub(1).ok_or_else(|| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "WOQL Eval Div floor correction overflows the i128 accumulator",
                    )
                })
            } else {
                Ok(truncated)
            }
        }
        EvalExpr::Exp(base, exponent) => {
            let base = eval_expr_row(base, values, row)?;
            let exponent = eval_expr_row(exponent, values, row)?;
            // Only NON-NEGATIVE INTEGER exponents give an INTEGER result under Prolog `**`
            // (with `prefer_rationals`); a negative exponent yields a rational, which is outside
            // this integer value layer. FAIL CLOSED on a negative or absurdly large exponent
            // (`u32::try_from` rejects both — an exponent past `u32::MAX` cannot possibly fit
            // the declared width and would only overflow below). This also covers a runtime
            // column exponent that turns out negative: the query is rejected, never coerced.
            let exponent = u32::try_from(exponent).map_err(|_| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "WOQL Eval Exp requires a non-negative integer exponent within u32 (a negative exponent is rational; a huge exponent overflows)",
                )
            })?;
            // Checked integer power in i128: an overflow beyond i128 FAILS CLOSED here, and the
            // final narrowing to the declared i64 width (in `expected_eval_columns`) rejects any
            // result outside that width — no field wraparound at any stage.
            base.checked_pow(exponent).ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "WOQL Eval Exp overflows the i128 accumulator",
                )
            })
        }
    }
}

/// VERIFIER RECOMPUTE for WOQL `Eval`: carry the child columns through unchanged and append
/// the derived integer column, re-derived elementwise from ROOT-AUTHENTICATED operands.
///
/// SOUNDNESS: `resolve_result_values` binds every child column value to the trusted root's
/// node/value dictionary (the same authentication Sum uses), so an operand's value is not
/// prover-chosen — it is exactly the store-derived value at the trusted root. The tree is
/// then re-evaluated deterministically in checked `i128` and narrowed to the declared `i64`
/// width; an out-of-`i64` result rejects closed (NO field wraparound). The caller compares
/// the delivered result column against this recomputation, so there is no biasable witness.
/// The child columns are passed through verbatim (they are already root-bound), so the outer
/// equality check also pins them.
fn expected_eval_columns(
    input: &ProvedBgp,
    input_plan: &CompiledRelation,
    expr: &EvalExpr,
    layer: &impl VerifierLayer,
    expected_root: ProofHash,
) -> std::io::Result<Vec<Vec<ArkFr>>> {
    let descriptors = relation_result_descriptors(input_plan)?;
    // Authenticate EVERY child column against the trusted root (ties operands to the root).
    let values = resolve_result_values(layer, expected_root, &input.result_columns, &descriptors)?;
    if input.result_columns.iter().any(|column| column.len() != input.result_len) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "WOQL Eval child column length mismatch",
        ));
    }
    // Pass the child columns through unchanged; the outer equality check pins them.
    let mut columns = input.result_columns.clone();
    let mut derived = Vec::with_capacity(input.result_len);
    for row in 0..input.result_len {
        let value = eval_expr_row(expr, &values, row)?;
        // Checked-narrow to the declared i64 integer width (mirrors the signed-sum path):
        // any result outside i64 rejects closed rather than wrapping in the field.
        let narrowed = i64::try_from(value).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "WOQL Eval result exceeds the declared i64 integer width",
            )
        })?;
        derived.push(generated_decimal_i64_field(narrowed));
    }
    columns.push(derived);
    Ok(columns)
}

// ============================================================================================
// Exact-rational (xsd:decimal) value layer: shared recompute for `Floor`, `Divide`, and decimal
// operands. Every function here is verifier-recompute: it consumes ONLY root-authenticated operand
// values and trusted query literals, and is deterministic — there is no prover witness.
// ============================================================================================

/// Parse a CANONICAL `xsd:decimal` lexical string (optional leading `-`, decimal digits, optional
/// `.`+digits; NO exponent) to an EXACT `rug::Rational`. This mirrors the reference
/// `casting:string_decimal_to_rational/2` (`numerator = int*10^fraclen + frac`, `denom = 10^fraclen`)
/// exactly, using GMP big integers so ANY precision is preserved. A leading-`.` form (`.5`) is
/// accepted as `0.5`. FAILS CLOSED on scientific notation, empty/garbage, or a non-digit character —
/// never guesses a value. The store's own decimals decode to this canonical form
/// (`storage_to_decimal`), and stored decimals are the only decimal OPERAND source; decimal LITERALS
/// are canonicalized by the compiler before reaching here.
fn parse_decimal_to_rational(text: &str) -> std::io::Result<BigRational> {
    let invalid = || {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "xsd:decimal lexical is not a canonical parseable decimal",
        )
    };
    let (neg, body) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    if body.is_empty() {
        return Err(invalid());
    }
    // Split into integer / fractional digit runs; reject anything that is not pure decimal digits.
    let (int_str, frac_str) = match body.split_once('.') {
        Some((int_part, frac_part)) => (int_part, frac_part),
        None => (body, ""),
    };
    // Leading `.5` -> integer part empty; treat as 0. A trailing `5.` -> fraction empty; treat as 0.
    let int_digits = if int_str.is_empty() { "0" } else { int_str };
    if !int_digits.bytes().all(|b| b.is_ascii_digit())
        || (!frac_str.is_empty() && !frac_str.bytes().all(|b| b.is_ascii_digit()))
    {
        return Err(invalid());
    }
    let denom = BigInt::from(10).pow(frac_str.len() as u32);
    // numerator = int_digits ++ frac_str  (as one big decimal integer), signed.
    let mut digits = String::with_capacity(int_digits.len() + frac_str.len());
    digits.push_str(int_digits);
    digits.push_str(frac_str);
    let mut numer = BigInt::from_str_radix(&digits, 10).map_err(|_| invalid())?;
    if neg {
        numer = -numer;
    }
    // `BigRational::new` auto-reduces to lowest terms with a positive denominator.
    Ok(BigRational::new(numer, denom))
}

/// Decode ONE root-authenticated child value to an EXACT rational for the decimal/rational layer.
/// The exact datatype is read from the TRUSTED dictionary value (never a compiler claim): the
/// fixed-width integer family decodes via the SAME order-preserving path `Sum`/`Eval` use
/// ([`integral_group_sum_value`]); `xsd:decimal` decodes via the store's own lexical decoder
/// ([`storage_to_decimal`]) then [`parse_decimal_to_rational`]. Any other datatype, a node IRI, a
/// null, or a generated value FAILS CLOSED — the recompute never fabricates or coerces a numeric.
fn numeric_operand_rational(value: &CanonicalResultValue) -> std::io::Result<BigRational> {
    let non_numeric = || {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "WOQL decimal-layer operand row is not an authenticated numeric (integer/xsd:decimal) value",
        )
    };
    let CanonicalResultValue::Object(CanonicalObject::Value { datatype, lexical }) = value else {
        return Err(non_numeric());
    };
    match datatype {
        CanonicalDatatype::Decimal => {
            // The store's xsd:decimal lexical is the BCD storage encoding; decode it to the
            // canonical decimal string, then to an exact rational. Always terminating/finite.
            let decoded = storage_to_decimal(&mut &lexical[..]);
            parse_decimal_to_rational(&decoded)
        }
        CanonicalDatatype::UInt8
        | CanonicalDatatype::UInt16
        | CanonicalDatatype::UInt32
        | CanonicalDatatype::UInt64
        | CanonicalDatatype::Int8
        | CanonicalDatatype::Int16
        | CanonicalDatatype::Int32
        | CanonicalDatatype::Int64 => match integral_group_sum_value(value, *datatype)? {
            GroupSumValue::Unsigned(v) => Ok(BigRational::from_integer(BigInt::from(v))),
            GroupSumValue::Signed(v) => {
                // Every supported signed fixed-width value fits i128 exactly (already is one).
                Ok(BigRational::from_integer(BigInt::from(v)))
            }
        },
        _ => Err(non_numeric()),
    }
}

/// Re-evaluate a [`DecimalExpr`] for ONE row over the root-authenticated operand columns, in EXACT
/// `rug::Rational`. All the fail-closed cases (zero divisor, non-numeric/unparseable operand,
/// out-of-scope Exp exponent) surface here as errors — never a dropped or fabricated row.
fn eval_decimal_expr_row(
    expr: &DecimalExpr,
    values: &[Vec<CanonicalResultValue>],
    row: usize,
) -> std::io::Result<BigRational> {
    match expr {
        DecimalExpr::Column { index } => {
            let column = values.get(*index).ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "WOQL decimal-layer operand column out of range",
                )
            })?;
            let value = column.get(row).ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "WOQL decimal-layer operand column is short",
                )
            })?;
            numeric_operand_rational(value)
        }
        DecimalExpr::IntegerLiteral(value) => Ok(BigRational::from_integer(BigInt::from(*value))),
        DecimalExpr::DecimalLiteral(lexical) => parse_decimal_to_rational(lexical),
        DecimalExpr::Plus(left, right) => Ok(eval_decimal_expr_row(left, values, row)?
            + eval_decimal_expr_row(right, values, row)?),
        DecimalExpr::Minus(left, right) => Ok(eval_decimal_expr_row(left, values, row)?
            - eval_decimal_expr_row(right, values, row)?),
        DecimalExpr::Times(left, right) => Ok(eval_decimal_expr_row(left, values, row)?
            * eval_decimal_expr_row(right, values, row)?),
        DecimalExpr::Divide(left, right) => {
            let dividend = eval_decimal_expr_row(left, values, row)?;
            let divisor = eval_decimal_expr_row(right, values, row)?;
            // Zero divisor: Prolog `rdiv`/`is` THROWS (`evaluation_error(zero_divisor)`), so the
            // WOQL query fails. FAIL CLOSED for this row (never drop it; never panic on the
            // divide). Static zero literal divisors are also rejected at compile time.
            if divisor.numer().is_zero() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "WOQL Divide by zero: Prolog `rdiv` throws, so the query fails closed",
                ));
            }
            Ok(dividend / divisor)
        }
        DecimalExpr::Exp(base, exponent) => {
            let base = eval_decimal_expr_row(base, values, row)?;
            let exponent = eval_decimal_expr_row(exponent, values, row)?;
            // Only a NON-NEGATIVE INTEGER exponent gives an exact rational power in this layer;
            // the compiler already required a static non-negative integer literal, but re-check the
            // recomputed rational to be safe (a fractional/negative exponent FAILS CLOSED).
            if !exponent.is_integer() || exponent.numer().is_negative() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "WOQL Exp in the decimal layer requires a non-negative integer exponent",
                ));
            }
            let e = exponent.numer().to_u32().filter(|e| *e <= DECIMAL_LAYER_MAX_EXP).ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "WOQL Exp exponent is too large for the decimal layer",
                )
            })?;
            // base^e by exact repeated multiplication (base^0 = 1); rationals stay exact.
            let mut acc = BigRational::one();
            for _ in 0..e {
                acc = acc * base.clone();
            }
            Ok(acc)
        }
        DecimalExpr::Floor(argument) => {
            // floor toward −∞ (SWI `floor/1`); `BigRational::floor` returns the integral rational.
            let value = eval_decimal_expr_row(argument, values, row)?;
            Ok(value.floor())
        }
    }
}

/// Render an EXACT rational to the reference's client-facing `xsd:decimal` lexical, reproducing
/// `casting:rational_to_decimal_string(R, S, 20)` BYTE-EXACT (validated differentially against
/// SWI-Prolog): sign, integer part, then up to 20 fractional digits ROUNDED HALF-UP at the 20th
/// (`(Rem*10^20*2 + Den) // (Den*2)`), with LEADING-zero padding to 20 and TRAILING zeros trimmed;
/// a whole rational renders as the bare integer (no decimal point). `rug`/GMP is the SAME big-integer
/// library SWI uses, and every operand here is non-negative (abs taken), so `//`/`mod` == floor ==
/// rug truncating division — identical arithmetic. FAILS CLOSED on the astronomically-implausible
/// carry-overflow case (the fraction rounds up to a full unit, `FracDigits == 10^20`, requiring a
/// reduced denominator ≥ 2·10^20) rather than reproduce the reference's carry BUG there.
const DECIMAL_LAYER_MAX_FRACTION_DIGITS: u32 = 20;
/// Upper bound on a decimal-layer `Exp` exponent (keeps exact `rug` powers from blowing up; larger
/// exponents reject closed rather than compute an astronomically large rational).
const DECIMAL_LAYER_MAX_EXP: u32 = 256;

fn rational_to_decimal_string_20(value: &BigRational) -> std::io::Result<String> {
    let max_decimals = DECIMAL_LAYER_MAX_FRACTION_DIGITS;
    let negative = value.numer().sign() == Sign::Minus;
    let abs = value.abs();
    // num-rational canonicalizes: denom > 0, gcd(numer,denom) = 1, sign in numer. After abs, >= 0.
    let num = abs.numer().clone();
    let den = abs.denom().clone();
    let (int_part, remainder) = num.div_rem(&den);
    if remainder.is_zero() {
        // Whole number: no fractional part. (A negative whole renders with its sign.)
        let sign = if negative && !int_part.is_zero() {
            "-"
        } else {
            ""
        };
        return Ok(format!("{sign}{int_part}"));
    }
    let multiplier = BigInt::from(10).pow(max_decimals);
    // Round-half-up: (Rem * Mult * 2 + Den) // (Den * 2). All operands positive => BigInt `/`
    // truncates toward zero == floor == SWI `//`.
    let frac_digits: BigInt =
        (remainder * &multiplier * BigInt::from(2) + &den) / (den * BigInt::from(2));
    if frac_digits >= multiplier {
        // The fraction rounded up to a whole unit (needs denom >= 2·10^20). The reference has a
        // carry BUG here; refuse rather than emit a value that might mismatch.
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "xsd:decimal render carry overflow (denominator >= 2·10^20); refusing rather than reproduce the reference carry bug",
        ));
    }
    // Pad with leading zeros to `max_decimals`, then trim trailing zeros.
    let mut frac = frac_digits.to_string();
    if (frac.len() as u32) < max_decimals {
        let pad = (max_decimals as usize) - frac.len();
        frac = format!("{}{}", "0".repeat(pad), frac);
    }
    let trimmed = frac.trim_end_matches('0');
    // NOTE: `trimmed` CAN be empty. A tiny-but-nonzero remainder rounds to 0 at
    // `max_decimals` digits once the reduced denominator exceeds 2·10^20, so `frac_digits`
    // is 0, and after padding + trimming nothing survives — the output is then `"<int>."`
    // (e.g. `"0."`). That trailing-dot form is exactly what the SWI reference emits here
    // (confirmed byte-for-byte), so it is produced DELIBERATELY; "normalizing" it away
    // (dropping the dot, or re-adding a `0`) would DIVERGE from the reference.
    let sign = if negative { "-" } else { "" };
    Ok(format!("{sign}{int_part}.{trimmed}"))
}

/// VERIFIER RECOMPUTE for WOQL `floor(x)`: carry the child columns through unchanged and append the
/// derived integer column, re-derived elementwise from ROOT-authenticated operands in exact
/// rationals then floored toward −∞ and narrowed to the declared `i64` generated domain (a result
/// outside `i64` FAILS CLOSED — no field wraparound). Mirrors [`expected_eval_columns`].
fn expected_floor_eval_columns(
    input: &ProvedBgp,
    input_plan: &CompiledRelation,
    expr: &DecimalExpr,
    layer: &impl VerifierLayer,
    expected_root: ProofHash,
) -> std::io::Result<Vec<Vec<ArkFr>>> {
    let values = resolve_authenticated_child_values(input, input_plan, layer, expected_root)?;
    let mut columns = input.result_columns.clone();
    let mut derived = Vec::with_capacity(input.result_len);
    for row in 0..input.result_len {
        // `expr` is the floor ARGUMENT (the compiler stores it here and floors it explicitly), so
        // evaluate then floor toward −∞ to an integer.
        let value = eval_decimal_expr_row(expr, &values, row)?;
        let floored = value.floor().to_integer();
        let narrowed = floored.to_i64().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "WOQL floor result exceeds the declared i64 integer width",
            )
        })?;
        derived.push(generated_decimal_i64_field(narrowed));
    }
    columns.push(derived);
    Ok(columns)
}

/// VERIFIER RECOMPUTE for WOQL exact-rational arithmetic (`Divide`/decimal-operand arithmetic): the
/// per-row result is a TERMINAL, REVEALED `xsd:decimal` value (like `Concat`) — re-derive the exact
/// rational from ROOT-authenticated operands and render the canonical 20-fractional-digit lexical.
/// A zero divisor, a non-numeric operand row, or the carry-overflow case FAIL CLOSED, so a proof is
/// only ever produced for rows the reference itself would compute, and the delivered value matches.
fn expected_rational_arith_objects(
    input: &ProvedBgp,
    input_plan: &CompiledRelation,
    expr: &DecimalExpr,
    layer: &impl VerifierLayer,
    expected_root: ProofHash,
) -> std::io::Result<Vec<CanonicalObject>> {
    let values = resolve_authenticated_child_values(input, input_plan, layer, expected_root)?;
    let mut out = Vec::with_capacity(input.result_len);
    for row in 0..input.result_len {
        let value = eval_decimal_expr_row(expr, &values, row)?;
        let lexical = rational_to_decimal_string_20(&value)?.into_bytes();
        out.push(CanonicalObject::Value {
            datatype: CanonicalDatatype::Decimal,
            lexical,
        });
    }
    Ok(out)
}

// ============================================================================================
// WOQL NUMBER↔STRING Typecast value layer (verifier-recompute; TERMINAL revealed result).
//
// Two delivery disciplines, both TERMINAL/revealed exactly like Concat/RationalArith (NO store
// change, NO commitment-format change, NO novel crypto):
//   * NUMBER→STRING (`cast_number_to_string_bytes`): decode the root-authenticated numeric source
//     and RENDER its client-facing lexical — integer family via base-10 (`format ~w`), xsd:decimal
//     via the already-SWI-validated `rational_to_decimal_string_20`. Delivered as `xsd:string`.
//   * STRING→NUMBER (`cast_string_to_integer_bigint` / `cast_string_to_decimal_rational`): PARSE
//     the root-authenticated xsd:string source with the exact reference grammar and deliver the
//     numeric value's client-facing lexical (`xsd:integer` base-10; `xsd:decimal` via
//     `rational_to_decimal_string_20`). A malformed/out-of-grammar string FAILS CLOSED (SWI
//     `typecast` throws → the query fails → the proof fails closed, never a wrong/silent value).
//
// Every grammar/render below was differentially validated against a live SWI-Prolog 10.0.2 running
// the reference predicates (`casting:typecast_switch`, `casting:string_decimal_to_rational/2`,
// `xsd_parser:integer//1`, `casting:rational_to_decimal_string/3`). SOUNDNESS is identical to
// Concat/RationalArith: the operand is pinned to the trusted root (`resolve_authenticated_child_values`),
// the cast is deterministic and byte-exact, and the delivered terminal result must equal the
// recomputation — no biasable witness.

/// Render ONE root-authenticated NUMERIC source value to its client-facing `xsd:string` lexical,
/// reproducing `casting:typecast_switch(xsd:string, SourceType, Val, _, S)` BYTE-EXACT.
///
/// * integer family (fixed-width `Int*`/`UInt*` and arbitrary `BigInt`=xsd:integer): SWI
///   `format(string(S),"~w",[Val])` = base-10 with a leading `-` for negatives and NO leading zeros;
///   Rust `to_string()` on `i128`/`u64`/`rug::Integer` reproduces this exactly.
/// * `xsd:decimal`: SWI `rational_to_decimal_string(Val, S, 20)` — the SAME renderer
///   [`rational_to_decimal_string_20`] the decimal/rational layer already delivers byte-exact.
///
/// FAILS CLOSED (reject-closed over approximate) on a `float`/`double` source — whose SWI `~w`
/// float formatting (shortest round-trip, forced `.0`) we do NOT reproduce — and on any non-numeric
/// source (date/boolean/langString/node), a null, or a generated scalar. There is no biasable
/// witness: the bytes come from the root-authenticated dictionary value.
fn cast_number_to_string_bytes(value: &CanonicalResultValue) -> std::io::Result<Vec<u8>> {
    let not_castable = || {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "WOQL number→string Typecast source row is not an authenticated integer/xsd:decimal value",
        )
    };
    let CanonicalResultValue::Object(CanonicalObject::Value { datatype, lexical }) = value else {
        return Err(not_castable());
    };
    let rendered = match datatype {
        // Fixed-width integer family → base-10 (`format ~w`). `integral_group_sum_value` re-checks
        // the exact datatype AND the canonical byte width against the root-authenticated lexical.
        CanonicalDatatype::UInt8
        | CanonicalDatatype::UInt16
        | CanonicalDatatype::UInt32
        | CanonicalDatatype::UInt64
        | CanonicalDatatype::Int8
        | CanonicalDatatype::Int16
        | CanonicalDatatype::Int32
        | CanonicalDatatype::Int64 => match integral_group_sum_value(value, *datatype)? {
            GroupSumValue::Signed(v) => v.to_string(),
            GroupSumValue::Unsigned(v) => v.to_string(),
        },
        // Arbitrary-precision xsd:integer → base-10 via the store's OWN bigint decoder (the exact
        // inverse of the encoding the root-authenticated lexical carries); `rug::Integer` Display is
        // base-10 with a leading `-`, matching SWI `format ~w`.
        CanonicalDatatype::BigInt => storage_to_bigint(&mut &lexical[..]).to_string(),
        // xsd:decimal → the exact rational (store BCD lexical → canonical string → rational), rendered
        // by the SWI-validated 20-fractional-digit renderer.
        CanonicalDatatype::Decimal => {
            let rational = parse_decimal_to_rational(&storage_to_decimal(&mut &lexical[..]))?;
            rational_to_decimal_string_20(&rational)?
        }
        // float/double (SWI float `~w`) + every other datatype are reject-closed.
        _ => return Err(not_castable()),
    };
    Ok(rendered.into_bytes())
}

/// Parse an `xsd:string` operand into an `xsd:integer` (arbitrary precision), reproducing the
/// reference `xsd_parser:integer//1` grammar (`casting.pl:569` `phrase(integer(Casted),C,[])`)
/// BYTE-EXACT: `sign(+/-/none)` then `natural` (`number_string` over one-or-more digits). That is
/// exactly `^[+-]?[0-9]+$` with LEADING ZEROS allowed (`"007" -> 7`, `"-0" -> 0`, `"+5" -> 5`),
/// differentially confirmed against SWI 10.0.2. Anything else — whitespace, an empty/sign-only
/// string, a decimal point, scientific `1e5`, hex `0x1f`, digit-group `1_000` — FAILS CLOSED (SWI's
/// `phrase(...,[])` leaves input unconsumed and `typecast` throws → the proof fails closed).
fn cast_string_to_integer_bigint(bytes: &[u8]) -> std::io::Result<BigInt> {
    let invalid = || {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "WOQL string→integer Typecast source is not a `[+-]?[0-9]+` integer lexical",
        )
    };
    // xsd:string content is UTF-8; a non-UTF-8 lexical never occurs for a faithful store, and if it
    // did we refuse rather than guess.
    let text = std::str::from_utf8(bytes).map_err(|_| invalid())?;
    let (negative, digits) = match text.as_bytes().first() {
        Some(b'-') => (true, &text[1..]),
        Some(b'+') => (false, &text[1..]),
        _ => (false, text),
    };
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err(invalid());
    }
    // All-ASCII-digit run: base-10 (leading zeros collapse in VALUE, exactly like SWI `number_string`).
    let mut value = BigInt::from_str_radix(digits, 10).map_err(|_| invalid())?;
    if negative {
        value = -value;
    }
    Ok(value)
}

/// Parse an `xsd:string` operand into an `xsd:decimal` (exact rational), reproducing a BYTE-EXACT
/// SAFE SUBSET of the reference `casting:string_decimal_to_rational/2` (`casting.pl:555`).
///
/// Accepted (all differentially confirmed against SWI 10.0.2): an optional single leading `-`, then
/// one of `D+` (integer), `D+.D+` (both runs non-empty), or `.D+` (leading dot, integer part 0). The
/// value is `sign * (intdigits ++ fracdigits) / 10^len(fracdigits)` — arithmetically IDENTICAL to
/// the reference's `IntPart*10^FracLen + FracPart` over `10^FracLen`.
///
/// REJECTED CLOSED (a strict subset of SWI's acceptance — rejecting is incompleteness-safe, never a
/// wrong value): trailing dot `D+.` (SWI's `atom_number("")` FAILS → `typecast` throws), a leading
/// `+` (SWI would route it through `atom_number`, which we do NOT reproduce), scientific `1e5`
/// (SWI's `atom_number` yields a FLOAT — a DIFFERENT value/type), hex `0x1f`, Prolog rational
/// literals `3r4`, multiple dots, whitespace, and the empty/sign-only string.
fn cast_string_to_decimal_rational(bytes: &[u8]) -> std::io::Result<BigRational> {
    let invalid = || {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "WOQL string→decimal Typecast source is not a reproducible decimal lexical (reject-closed subset of string_decimal_to_rational)",
        )
    };
    let text = std::str::from_utf8(bytes).map_err(|_| invalid())?;
    // The reference strips a leading `-` ONLY (never `+`); reproduce exactly.
    let (negative, body) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    if body.is_empty() || body.starts_with('+') {
        return Err(invalid());
    }
    let (int_str, frac_str) = match body.split_once('.') {
        Some((int_part, frac_part)) => {
            // A SECOND '.' -> SWI `split_string` yields 3 parts and the clause fails. Reject.
            if frac_part.contains('.') {
                return Err(invalid());
            }
            // Trailing dot `D+.` -> SWI `atom_number("")` fails -> `typecast` throws. Reject.
            if frac_part.is_empty() {
                return Err(invalid());
            }
            (int_part, frac_part)
        }
        // No '.' -> integer form (SWI `atom_number(String)` over a pure-digit string).
        None => (body, ""),
    };
    // Leading dot `.D+` -> the reference prepends "0"; treat an empty integer part as 0.
    let int_digits = if int_str.is_empty() { "0" } else { int_str };
    if !int_digits.bytes().all(|b| b.is_ascii_digit())
        || (!frac_str.is_empty() && !frac_str.bytes().all(|b| b.is_ascii_digit()))
    {
        return Err(invalid());
    }
    let denominator = BigInt::from(10).pow(frac_str.len() as u32);
    // numerator = (int_digits ++ frac_str) as one big decimal integer, signed.
    let mut digits = String::with_capacity(int_digits.len() + frac_str.len());
    digits.push_str(int_digits);
    digits.push_str(frac_str);
    let mut numerator = BigInt::from_str_radix(&digits, 10).map_err(|_| invalid())?;
    if negative {
        numerator = -numerator;
    }
    // `BigRational::new` auto-reduces to lowest terms with a positive denominator.
    Ok(BigRational::new(numerator, denominator))
}

/// VERIFIER RECOMPUTE for a WOQL NUMBER→STRING `Typecast`: authenticate the numeric source column
/// against the trusted root, RENDER each row's client-facing `xsd:string` lexical
/// ([`cast_number_to_string_bytes`]), and deliver revealed `xsd:string` objects. A float/double or
/// non-numeric operand row FAILS CLOSED, so a proof is only ever produced for rows the reference
/// itself would cast, and the delivered value matches byte-for-byte.
fn expected_cast_to_string_objects(
    input: &ProvedBgp,
    input_plan: &CompiledRelation,
    source_column: usize,
    layer: &impl VerifierLayer,
    expected_root: ProofHash,
) -> std::io::Result<Vec<CanonicalObject>> {
    let values = resolve_authenticated_child_values(input, input_plan, layer, expected_root)?;
    let column = values.get(source_column).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "WOQL number→string Typecast source column out of range",
        )
    })?;
    if column.len() != input.result_len {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "WOQL number→string Typecast source column length mismatch",
        ));
    }
    let mut out = Vec::with_capacity(input.result_len);
    for value in column {
        out.push(CanonicalObject::Value {
            datatype: CanonicalDatatype::String,
            lexical: cast_number_to_string_bytes(value)?,
        });
    }
    Ok(out)
}

/// VERIFIER RECOMPUTE for a WOQL STRING→NUMBER `Typecast`: authenticate the `xsd:string` source
/// column against the trusted root ([`authenticate_string_column`]), PARSE each row with the exact
/// reference grammar for `target`, and deliver the numeric value's client-facing lexical
/// (`xsd:integer` base-10 tagged `BigInt`; `xsd:decimal` via [`rational_to_decimal_string_20`] tagged
/// `Decimal`). A malformed/out-of-grammar string FAILS CLOSED (the reference throws), so a proof is
/// only ever produced for rows the reference itself would cast, and the delivered value matches.
fn expected_cast_from_string_objects(
    input: &ProvedBgp,
    input_plan: &CompiledRelation,
    source_column: usize,
    target: StringNumericTarget,
    layer: &impl VerifierLayer,
    expected_root: ProofHash,
) -> std::io::Result<Vec<CanonicalObject>> {
    let values = resolve_authenticated_child_values(input, input_plan, layer, expected_root)?;
    // Root-authenticated xsd:string bytes per row (a non-string operand row FAILS CLOSED here).
    let source_bytes = authenticate_string_column(&values, source_column, input.result_len)?;
    let mut out = Vec::with_capacity(input.result_len);
    for bytes in &source_bytes {
        let object = match target {
            StringNumericTarget::Integer => CanonicalObject::Value {
                datatype: CanonicalDatatype::BigInt,
                lexical: cast_string_to_integer_bigint(bytes)?.to_string().into_bytes(),
            },
            StringNumericTarget::Decimal => {
                let rational = cast_string_to_decimal_rational(bytes)?;
                CanonicalObject::Value {
                    datatype: CanonicalDatatype::Decimal,
                    lexical: rational_to_decimal_string_20(&rational)?.into_bytes(),
                }
            }
        };
        out.push(object);
    }
    Ok(out)
}

// ============================================================================================
// WOQL CALENDAR value layer (verifier-recompute; xsd:date / xsd:gYearMonth operand only).
//
// Single-input date functions over the newly-committed dates. Two delivery disciplines, exactly
// like the arithmetic/string layers (NO store change, NO commitment-format change, NO novel crypto):
//   * INTEGER results (`Weekday`/`WeekdaySundayStart`/`IsoWeek`) — delivered like `Eval`: the child
//     columns pass through and ONE derived `GeneratedDecimalI64` column is appended PER bound integer
//     (1 for Weekday/WeekdaySundayStart, 2 for IsoWeek's {year, week}); the verifier RE-DERIVES them.
//   * DATE results (`DayAfter`/`DayBefore`/`MonthStartDate`/`MonthEndDate`) — delivered like
//     `Concat`/`RationalArith`: a TERMINAL, REVEALED `xsd:date` value the verifier RE-DERIVES and
//     checks (no committed result column).
//
// Every calendar function below is ported BYTE-FOR-BYTE from the SWI-Prolog reference
// (`terminusdb/src/core/query/woql_compile.pl`, cited per fn) and was differentially validated
// against a live SWI-Prolog 10.0.2 on the reference test vectors. The operand date is authenticated
// against the trusted root (`resolve_authenticated_child_values` — the SAME authentication
// Eval/strings/decimal use), the function is recomputed deterministically, and the delivered
// terminal/derived result must equal that recomputation. There is NO prover witness.
//
// FIDELITY GUARDS (reject-closed / fail-closed rather than approximate):
//   * xsd:date operands ONLY. An `xsd:dateTime` operand is reject-closed: decoding its
//     variable-width store lexical would require reproducing the store's private dateTime decoder,
//     risking drift — reject-closed over approximate. (The reference `weekday`/`iso_week` also accept
//     dateTime; that is a coverage boundary, not a soundness gap — a non-date operand fails closed.)
//   * INPUT year >= 1: the ported integer algorithms use `//`/`mod`, which equal Rust `/`/`.rem_euclid`
//     only over NON-NEGATIVE dividends; year >= 1 keeps every division non-negative, so the port is
//     byte-exact. A year < 1 fails closed rather than risk a negative-division mismatch.
//   * DATE outputs: timezone offset 0 ONLY, output year >= 1. The client lexical is `YYYY-MM-DD`
//     (SWI `date_string`, `literals.pl:290` / `xsd_parser.pl:255`: `~|~`0t~d~4+-~|~`0t~d~2+-...`
//     with an EMPTY offset suffix for offset 0) — reproduced exactly by `format!("{:04}-{:02}-{:02}")`
//     for year >= 1. A non-zero offset (whose `±HH:MM` suffix we do not reproduce) or an out-of-range
//     output year fails closed.

/// Which single-input, INTEGER-result calendar function a [`CompiledRelationStatement::CalendarInt`]
/// output column computes. Each maps a bound `xsd:date` to one `xsd:integer`, matching the SWI
/// reference exactly. `IsoWeek` binds TWO integers, so it carries `[IsoWeekYear, IsoWeekWeek]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CalendarIntFn {
    /// `weekday(Date, W)` (`woql_compile.pl:1657`) via Sakamoto `iso_weekday_number` (`:1637`):
    /// ISO-8601 weekday, Monday = 1 .. Sunday = 7.
    Weekday,
    /// `weekday_sunday_start(Date, W)` (`woql_compile.pl:1675`): US convention `(IsoDay mod 7) + 1`,
    /// Sunday = 1 .. Saturday = 7.
    WeekdaySundayStart,
    /// `iso_week(Date, Year, W)` (`woql_compile.pl:1744` / `iso_week_date` `:1710`): the ISO-8601
    /// week-numbering YEAR component.
    IsoWeekYear,
    /// `iso_week(Date, Year, W)`: the ISO-8601 WEEK-number component (1..53).
    IsoWeekWeek,
}

/// Which single-input, DATE-result calendar function a [`CompiledRelationStatement::CalendarDate`]
/// statement computes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CalendarDateFn {
    /// `day_after(Date, Next)` (`woql_compile.pl:1285`): the calendar day after a bound `xsd:date`.
    DayAfter,
    /// `day_before(Date, Previous)` (`woql_compile.pl:1310`): the calendar day before a bound `xsd:date`.
    DayBefore,
    /// `month_start_date(YearMonth, Date)` (`woql_compile.pl:1205`): first day of the month of a
    /// bound `xsd:gYearMonth` (`date(Y,M,1,Offset)`).
    MonthStartDate,
    /// `month_end_date(YearMonth, Date)` (`woql_compile.pl:1219`): last day of that month
    /// (`date(Y,M,days_in_month(Y,M),Offset)`).
    MonthEndDate,
}

/// SWI `is_leap_year/1` (`woql_compile.pl:1520`): the proleptic Gregorian leap rule.
fn cal_is_leap_year(year: i64) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

/// SWI `days_in_month/3` (`woql_compile.pl:1527`). `None` for a month outside `1..=12` (a corrupt
/// operand lexical — a store `Date`/`GYearMonth` always carries a valid month, so this only guards
/// against corruption and fails closed).
fn cal_days_in_month(year: i64, month: u8) -> Option<u8> {
    Some(match month {
        1 => 31,
        2 => if cal_is_leap_year(year) { 29 } else { 28 },
        3 => 31,
        4 => 30,
        5 => 31,
        6 => 30,
        7 => 31,
        8 => 31,
        9 => 30,
        10 => 31,
        11 => 30,
        12 => 31,
        _ => return None,
    })
}

/// SWI `iso_weekday_number/4` (`woql_compile.pl:1637`, Sakamoto's algorithm): ISO weekday
/// Monday = 1 .. Sunday = 7. The caller guarantees `year >= 1` and `1 <= month <= 12`, so every
/// `//` here is over a NON-NEGATIVE dividend and Rust `/`/`.rem_euclid(7)` equal SWI's `//`/`mod`
/// byte-for-byte (they diverge only on negative operands, which the guards exclude).
fn cal_iso_weekday_number(year: i64, month: u8, day: u8) -> i64 {
    // `nth0(M, [0,0,3,2,5,0,3,5,1,4,6,2,4], T)` — index `month` (index 0 is unused, `month>=1`).
    const MONTH_TABLE: [i64; 13] = [0, 0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let t = MONTH_TABLE[month as usize];
    let y1 = if month < 3 { year - 1 } else { year };
    let dow = (y1 + y1 / 4 - y1 / 100 + y1 / 400 + t + i64::from(day)).rem_euclid(7);
    // DoW: 0 = Sunday .. 6 = Saturday → ISO Monday = 1 .. Sunday = 7.
    if dow == 0 {
        7
    } else {
        dow
    }
}

/// SWI `woql_weekday_sunday_start/2` (`woql_compile.pl:1675`): `USDay is (IsoDay mod 7) + 1`
/// (Sunday = 1 .. Saturday = 7). `iso >= 1`, so `.rem_euclid(7)` equals SWI `mod 7` exactly.
fn cal_us_weekday(year: i64, month: u8, day: u8) -> i64 {
    cal_iso_weekday_number(year, month, day).rem_euclid(7) + 1
}

/// SWI `ordinal_day/4` (`woql_compile.pl:1693`): day-of-year (1..366). `None` on a corrupt month.
fn cal_ordinal_day(year: i64, month: u8, day: u8) -> Option<i64> {
    let mut acc: i64 = 0;
    for m in 1..month {
        acc += i64::from(cal_days_in_month(year, m)?);
    }
    Some(acc + i64::from(day))
}

/// SWI `iso_week_date/5` (`woql_compile.pl:1710`): the ISO-8601 (week-numbering year, week number).
/// The caller guarantees `year >= 1` and `1 <= month <= 12`; every `//` is over a non-negative
/// dividend (`adj - 1 >= 0` and `thursday - 1 >= 0` in their branches), so `.div_euclid(7)` equals
/// SWI `//` byte-for-byte.
fn cal_iso_week_date(year: i64, month: u8, day: u8) -> Option<(i64, i64)> {
    let day_of_week = cal_iso_weekday_number(year, month, day);
    let ordinal = cal_ordinal_day(year, month, day)?;
    // Thursday of this week's ordinal day.
    let thursday_ord = ordinal + (4 - day_of_week);
    Some(if thursday_ord < 1 {
        // Thursday is in the previous year.
        let prev_year = year - 1;
        let prev_days = if cal_is_leap_year(prev_year) { 366 } else { 365 };
        let adj_ord = prev_days + thursday_ord;
        (prev_year, (adj_ord - 1).div_euclid(7) + 1)
    } else {
        let year_days = if cal_is_leap_year(year) { 366 } else { 365 };
        if thursday_ord > year_days {
            // Thursday is in the next year.
            (year + 1, 1)
        } else {
            (year, (thursday_ord - 1).div_euclid(7) + 1)
        }
    })
}

/// SWI `next_month/4` (`woql_compile.pl:1273`).
fn cal_next_month(year: i64, month: u8) -> (i64, u8) {
    if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    }
}

/// SWI `prev_month/4` (`woql_compile.pl:1276`).
fn cal_prev_month(year: i64, month: u8) -> (i64, u8) {
    if month == 1 {
        (year - 1, 12)
    } else {
        (year, month - 1)
    }
}

/// SWI `woql_day_after/2` forward branch (`woql_compile.pl:1285`): given a bound date, the day after.
/// `None` on a corrupt month.
fn cal_day_after(year: i64, month: u8, day: u8) -> Option<(i64, u8, u8)> {
    let max_day = cal_days_in_month(year, month)?;
    Some(if day < max_day {
        (year, month, day + 1)
    } else {
        let (next_year, next_month) = cal_next_month(year, month);
        (next_year, next_month, 1)
    })
}

/// SWI `woql_day_before/2` forward branch (`woql_compile.pl:1310`): given a bound date, the day before.
/// `None` on a corrupt month.
fn cal_day_before(year: i64, month: u8, day: u8) -> Option<(i64, u8, u8)> {
    Some(if day > 1 {
        (year, month, day - 1)
    } else {
        let (prev_year, prev_month) = cal_prev_month(year, month);
        let prev_max = cal_days_in_month(prev_year, prev_month)?;
        (prev_year, prev_month, prev_max)
    })
}

/// Decode ONE root-authenticated `xsd:date` operand row to `(year, month, day, offset)`, reproducing
/// tdb-succinct's `Date` lexical layout BYTE-FOR-BYTE — the SAME 12 bytes the store's
/// `date_lexical_to_op_i64` decodes: `year:i64` offset-binary BE (8) | `month:u8` (1) | `day:u8` (1)
/// | `offset:i16` offset-binary BE (2). FAILS CLOSED on a non-date row (node IRI, non-date value,
/// null/generated), a wrong-width lexical, or an out-of-range calendar month/day (a corrupt lexical).
fn cal_decode_date_operand(value: &CanonicalResultValue) -> std::io::Result<(i64, u8, u8, i16)> {
    let not_date = || {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "WOQL calendar operand row is not an authenticated xsd:date value",
        )
    };
    let CanonicalResultValue::Object(CanonicalObject::Value {
        datatype: CanonicalDatatype::Date,
        lexical,
    }) = value
    else {
        return Err(not_date());
    };
    let bytes: [u8; 12] = lexical.as_slice().try_into().map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "WOQL calendar xsd:date lexical is not exactly 12 bytes",
        )
    })?;
    let year = (u64::from_be_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ]) ^ (1u64 << 63)) as i64;
    let month = bytes[8];
    let day = bytes[9];
    let offset = (u16::from_be_bytes([bytes[10], bytes[11]]) ^ (1u16 << 15)) as i16;
    // A store `Date` is always a valid calendar day; a corrupt lexical (bad month/day) fails closed.
    match cal_days_in_month(year, month) {
        Some(max) if (1..=max).contains(&day) => {}
        _ => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "WOQL calendar xsd:date has an out-of-range month or day",
            ))
        }
    }
    Ok((year, month, day, offset))
}

/// Decode ONE root-authenticated `xsd:gYearMonth` operand row to `(year, month, offset)`, reproducing
/// tdb-succinct's `GYearMonth` lexical layout BYTE-FOR-BYTE: `year:i64` offset-binary BE (8) |
/// `month:u8` (1) | `offset:i16` offset-binary BE (2). FAILS CLOSED on a non-gYearMonth row, a
/// wrong-width lexical, or an out-of-range month.
fn cal_decode_gyearmonth_operand(value: &CanonicalResultValue) -> std::io::Result<(i64, u8, i16)> {
    let not_ym = || {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "WOQL calendar operand row is not an authenticated xsd:gYearMonth value",
        )
    };
    let CanonicalResultValue::Object(CanonicalObject::Value {
        datatype: CanonicalDatatype::GYearMonth,
        lexical,
    }) = value
    else {
        return Err(not_ym());
    };
    let bytes: [u8; 11] = lexical.as_slice().try_into().map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "WOQL calendar xsd:gYearMonth lexical is not exactly 11 bytes",
        )
    })?;
    let year = (u64::from_be_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ]) ^ (1u64 << 63)) as i64;
    let month = bytes[8];
    let offset = (u16::from_be_bytes([bytes[9], bytes[10]]) ^ (1u16 << 15)) as i16;
    if !(1..=12).contains(&month) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "WOQL calendar xsd:gYearMonth has an out-of-range month",
        ));
    }
    Ok((year, month, offset))
}

/// Render an output calendar date to the reference CLIENT lexical `YYYY-MM-DD`. Byte-exact to SWI
/// `date_string` (`literals.pl:290`) for `year >= 1` (zero-padded to 4+, `~4+`), month/day `~2+`, and
/// an offset-0 (empty) suffix. Reject-closed (fail closed) on `year < 1` — a negative/zero year's SWI
/// column-fill rendering (or the offset suffix) is not reproduced here rather than guessed.
fn cal_date_result_object(year: i64, month: u8, day: u8) -> std::io::Result<CanonicalObject> {
    if year < 1 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "WOQL calendar date result year < 1 is out of the byte-exact-rendered range (reject closed)",
        ));
    }
    Ok(CanonicalObject::Value {
        datatype: CanonicalDatatype::Date,
        lexical: format!("{year:04}-{month:02}-{day:02}").into_bytes(),
    })
}

/// VERIFIER RECOMPUTE for the INTEGER-result calendar ops (`Weekday`/`WeekdaySundayStart`/`IsoWeek`),
/// the calendar analogue of [`expected_eval_columns`]. Carry the child columns through unchanged and
/// append ONE derived `GeneratedDecimalI64` column PER `outputs` entry (1 for Weekday/WeekdaySundayStart,
/// 2 for IsoWeek's `[year, week]`), each re-derived elementwise from the ROOT-authenticated `xsd:date`
/// operand column. A non-date operand row, an `year < 1` input, or a corrupt lexical FAILS CLOSED.
fn expected_calendar_int_columns(
    input: &ProvedBgp,
    input_plan: &CompiledRelation,
    date_column: usize,
    outputs: &[CalendarIntFn],
    layer: &impl VerifierLayer,
    expected_root: ProofHash,
) -> std::io::Result<Vec<Vec<ArkFr>>> {
    let values = resolve_authenticated_child_values(input, input_plan, layer, expected_root)?;
    let operand = values.get(date_column).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "WOQL calendar operand column out of range",
        )
    })?;
    if operand.len() != input.result_len {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "WOQL calendar operand column length mismatch",
        ));
    }
    let mut columns = input.result_columns.clone();
    let mut derived: Vec<Vec<ArkFr>> = outputs
        .iter()
        .map(|_| Vec::with_capacity(input.result_len))
        .collect();
    for row in 0..input.result_len {
        let (year, month, day, _offset) = cal_decode_date_operand(&operand[row])?;
        // Weekday/IsoWeek ignore the timezone offset entirely (the reference `extract_ymd` uses only
        // Y/M/D), so any offset is fine; but the year must be >= 1 for the ported algorithms' `//`.
        if year < 1 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "WOQL calendar integer op requires an input year >= 1 (reject closed)",
            ));
        }
        // Compute the ISO week ONCE per row (both components share it); fail closed on a corrupt month.
        let iso_week = if outputs
            .iter()
            .any(|f| matches!(f, CalendarIntFn::IsoWeekYear | CalendarIntFn::IsoWeekWeek))
        {
            Some(cal_iso_week_date(year, month, day).ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "WOQL calendar iso_week has a corrupt operand month",
                )
            })?)
        } else {
            None
        };
        for (index, function) in outputs.iter().enumerate() {
            let value: i64 = match function {
                CalendarIntFn::Weekday => cal_iso_weekday_number(year, month, day),
                CalendarIntFn::WeekdaySundayStart => cal_us_weekday(year, month, day),
                CalendarIntFn::IsoWeekYear => iso_week.expect("iso_week computed above").0,
                CalendarIntFn::IsoWeekWeek => iso_week.expect("iso_week computed above").1,
            };
            derived[index].push(generated_decimal_i64_field(value));
        }
    }
    columns.extend(derived);
    Ok(columns)
}

/// VERIFIER RECOMPUTE for the DATE-result calendar ops (`DayAfter`/`DayBefore`/`MonthStartDate`/
/// `MonthEndDate`), the calendar analogue of [`expected_rational_arith_objects`]: a per-row TERMINAL,
/// REVEALED `xsd:date` value re-derived from the ROOT-authenticated operand (`xsd:date` for `Day*`,
/// `xsd:gYearMonth` for `Month*`). FAILS CLOSED on a wrong-typed operand, a non-zero timezone offset,
/// an `year < 1` input/output, or a corrupt lexical — a proof is only ever produced for rows whose
/// exact reference result this reproduces byte-for-byte, and the delivered value must equal it.
fn expected_calendar_date_objects(
    input: &ProvedBgp,
    input_plan: &CompiledRelation,
    op: CalendarDateFn,
    input_column: usize,
    layer: &impl VerifierLayer,
    expected_root: ProofHash,
) -> std::io::Result<Vec<CanonicalObject>> {
    let values = resolve_authenticated_child_values(input, input_plan, layer, expected_root)?;
    let operand = values.get(input_column).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "WOQL calendar operand column out of range",
        )
    })?;
    if operand.len() != input.result_len {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "WOQL calendar operand column length mismatch",
        ));
    }
    let nonzero_offset = || {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "WOQL calendar date op requires a zero timezone offset (a ±HH:MM suffix is not reproduced; reject closed)",
        )
    };
    let low_year = || {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "WOQL calendar date op requires year >= 1 (reject closed)",
        )
    };
    let corrupt = || {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "WOQL calendar date op operand has a corrupt month",
        )
    };
    let mut out = Vec::with_capacity(input.result_len);
    for row in 0..input.result_len {
        let (year, month, day) = match op {
            CalendarDateFn::DayAfter | CalendarDateFn::DayBefore => {
                let (year, month, day, offset) = cal_decode_date_operand(&operand[row])?;
                // The reference PRESERVES the offset into the output date; we only render offset 0.
                if offset != 0 {
                    return Err(nonzero_offset());
                }
                if year < 1 {
                    return Err(low_year());
                }
                match op {
                    CalendarDateFn::DayAfter => cal_day_after(year, month, day).ok_or_else(corrupt)?,
                    _ => cal_day_before(year, month, day).ok_or_else(corrupt)?,
                }
            }
            CalendarDateFn::MonthStartDate | CalendarDateFn::MonthEndDate => {
                let (year, month, offset) = cal_decode_gyearmonth_operand(&operand[row])?;
                if offset != 0 {
                    return Err(nonzero_offset());
                }
                if year < 1 {
                    return Err(low_year());
                }
                let day = match op {
                    CalendarDateFn::MonthStartDate => 1u8,
                    _ => cal_days_in_month(year, month).ok_or_else(corrupt)?,
                };
                (year, month, day)
            }
        };
        out.push(cal_date_result_object(year, month, day)?);
    }
    Ok(out)
}

/// The exact XSD value range `[min, max]` (as `i128`) and signedness of a fixed-width integer
/// datatype, or `None` for any non-fixed-width-integer datatype. These bounds are the SAME
/// ones the Prolog narrowing typecast enforces (`is_byte`/`is_short`/… in `util/types.pl`), so
/// a value outside the range is exactly what makes Prolog `typecast` throw `casting_error`.
fn integer_datatype_bounds(datatype: CanonicalDatatype) -> Option<(i128, i128, bool)> {
    use CanonicalDatatype::*;
    Some(match datatype {
        Int8 => (i128::from(i8::MIN), i128::from(i8::MAX), true),
        Int16 => (i128::from(i16::MIN), i128::from(i16::MAX), true),
        Int32 => (i128::from(i32::MIN), i128::from(i32::MAX), true),
        Int64 => (i128::from(i64::MIN), i128::from(i64::MAX), true),
        UInt8 => (0, i128::from(u8::MAX), false),
        UInt16 => (0, i128::from(u16::MAX), false),
        UInt32 => (0, i128::from(u32::MAX), false),
        UInt64 => (0, i128::from(u64::MAX), false),
        _ => return None,
    })
}

/// The generated result namespace a fixed-width integer TARGET type is delivered in: signed
/// targets use the signed generated domain, unsigned targets the unsigned one (an unsigned
/// target can hold values above `i64::MAX`). Fails closed for a non-integer datatype.
fn integer_target_namespace(datatype: CanonicalDatatype) -> std::io::Result<ResultIdNamespace> {
    match integer_datatype_bounds(datatype) {
        Some((_, _, true)) => Ok(ResultIdNamespace::GeneratedDecimalI64),
        Some((_, _, false)) => Ok(ResultIdNamespace::GeneratedDecimalU64),
        None => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "WOQL Typecast target is not a fixed-width integer datatype",
        )),
    }
}

/// VERIFIER RECOMPUTE for WOQL `Typecast` of one integer column to a fixed-width integer target:
/// carry the child columns through unchanged and append the derived cast column, re-derived
/// elementwise from the ROOT-AUTHENTICATED source column.
///
/// SOUNDNESS: `resolve_result_values` binds every child column to the trusted root (same
/// authentication Eval/Sum use); the source value is decoded via [`eval_operand_i128`], which
/// also re-checks the pinned `source_datatype` against the authenticated dictionary value. An
/// integer→integer cast preserves the numeric value, so the recompute is: assert the value lies
/// within the target type's exact XSD range and re-emit it in the target's generated namespace.
/// A value outside the range FAILS CLOSED — matching Prolog `typecast`, which THROWS
/// `casting_error` on an out-of-range narrowing rather than wrapping/truncating. The caller
/// compares the delivered result column against this recomputation, so there is no biasable
/// witness; the passed-through child columns are pinned by the outer equality + `verify_columns`.
fn expected_typecast_column(
    input: &ProvedBgp,
    input_plan: &CompiledRelation,
    source_column: usize,
    source_datatype: CanonicalDatatype,
    target_datatype: CanonicalDatatype,
    layer: &impl VerifierLayer,
    expected_root: ProofHash,
) -> std::io::Result<Vec<Vec<ArkFr>>> {
    let descriptors = relation_result_descriptors(input_plan)?;
    // Authenticate EVERY child column against the trusted root (ties the source to the root).
    let values = resolve_result_values(layer, expected_root, &input.result_columns, &descriptors)?;
    if input.result_columns.iter().any(|column| column.len() != input.result_len) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "WOQL Typecast child column length mismatch",
        ));
    }
    let (lo, hi, signed) = integer_datatype_bounds(target_datatype).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "WOQL Typecast target is not a fixed-width integer datatype",
        )
    })?;
    let source = values.get(source_column).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "WOQL Typecast source column out of range",
        )
    })?;
    // Pass the child columns through unchanged; the outer equality check pins them.
    let mut columns = input.result_columns.clone();
    let mut derived = Vec::with_capacity(input.result_len);
    for row in 0..input.result_len {
        let value = eval_operand_i128(&source[row], source_datatype)?;
        // Range check = Prolog's narrowing `casting_error` throw: FAIL CLOSED (never wrap) when
        // the authenticated source value does not fit the target type's exact XSD range.
        if value < lo || value > hi {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "WOQL Typecast value is outside the target integer type's range (Prolog throws casting_error)",
            ));
        }
        // In-range: the value is unchanged (lossless identity), re-emitted in the target's
        // generated namespace. The `try_from`s cannot fail after the range check (the range is
        // a subset of i64/u64), but are kept as a defensive second gate.
        let field = if signed {
            generated_decimal_i64_field(i64::try_from(value).map_err(|_| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "WOQL Typecast signed value exceeds i64",
                )
            })?)
        } else {
            ArkFr::from_u64(u64::try_from(value).map_err(|_| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "WOQL Typecast unsigned value exceeds u64",
                )
            })?)
        };
        derived.push(field);
    }
    columns.push(derived);
    Ok(columns)
}

/// Resolve EVERY child result column through the ROOT-authenticated node/value dictionary,
/// returning the per-row canonical values. This is the exact authentication `Eval`/`Sum`/
/// `Typecast` use ([`resolve_result_values`]): it pins `commitment_root == expected_root`,
/// re-validates the root-bound dictionary, and decodes each committed id to its store value.
/// Every child value is therefore the store's root-derived value, with no prover latitude.
fn resolve_authenticated_child_values(
    input: &ProvedBgp,
    input_plan: &CompiledRelation,
    layer: &impl VerifierLayer,
    expected_root: ProofHash,
) -> std::io::Result<Vec<Vec<CanonicalResultValue>>> {
    let descriptors = relation_result_descriptors(input_plan)?;
    let values = resolve_result_values(layer, expected_root, &input.result_columns, &descriptors)?;
    if input
        .result_columns
        .iter()
        .any(|column| column.len() != input.result_len)
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "WOQL string-op child column length mismatch",
        ));
    }
    Ok(values)
}

/// Extract the per-row authenticated xsd:string BYTES of one already-resolved child column.
///
/// This is the BYTE-AUTHENTICATION FOUNDATION shared by the whole string-op family (concat
/// now; upper/lower/trim/substring later): given the root-authenticated child values (from
/// [`resolve_authenticated_child_values`]), require column `column` to be a nonnull xsd:string
/// VALUE in EVERY row and return its lexical bytes. FAILS CLOSED on a node IRI, a non-string
/// typed value (e.g. an integer/langString — which `interpolate_string` would STRINGIFY, an
/// out-of-scope circuit), a null, or an out-of-range column index. Because the values are
/// root-authenticated, the returned bytes are exactly the store's root-derived string content:
/// there is no biasable witness, matching the arithmetic circuits' soundness model.
fn authenticate_string_column(
    values: &[Vec<CanonicalResultValue>],
    column: usize,
    result_len: usize,
) -> std::io::Result<Vec<Vec<u8>>> {
    let resolved = values.get(column).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "WOQL string-op operand column out of range",
        )
    })?;
    if resolved.len() != result_len {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "WOQL string-op operand column length mismatch",
        ));
    }
    resolved
        .iter()
        .map(|value| match value {
            CanonicalResultValue::Object(CanonicalObject::Value {
                datatype: CanonicalDatatype::String,
                lexical,
            }) => Ok(lexical.clone()),
            // A node IRI, a non-string value, or a null is NOT a provable string operand in v1.
            // SWI `interpolate_string` would STRINGIFY a non-string (number/atom) via
            // `term_to_atom`/`atom_string`; that is a separate stringification circuit, so we
            // REJECT CLOSED here rather than guess the rendering.
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "WOQL string-op operand row is not an authenticated xsd:string value",
            )),
        })
        .collect()
}

/// VERIFIER RECOMPUTE for WOQL `concat` — the byte analogue of [`expected_eval_columns`].
///
/// Re-derives the per-row concatenation from ROOT-AUTHENTICATED operand columns (+ trusted
/// query-literal bytes) by appending each element's bytes in LIST ORDER with NO separator —
/// exactly `interpolate_string/2`'s left-to-right `string_concat` over string elements. The
/// result is returned as revealed `xsd:string` [`CanonicalObject`]s (a TERMINAL value: not a
/// committed column). SOUNDNESS: the operand bytes are pinned to the trusted root via
/// [`resolve_authenticated_child_values`]/[`authenticate_string_column`] and byte concatenation
/// is deterministic, so the caller's exact-match check against the delivered result column
/// leaves no biasable witness. FAILS CLOSED (never stringifies) on any non-string operand row.
fn expected_concat_objects(
    input: &ProvedBgp,
    input_plan: &CompiledRelation,
    elements: &[ConcatElement],
    layer: &impl VerifierLayer,
    expected_root: ProofHash,
) -> std::io::Result<Vec<CanonicalObject>> {
    let values = resolve_authenticated_child_values(input, input_plan, layer, expected_root)?;
    // Authenticate each referenced string column ONCE (root-bound bytes), keyed by column index.
    let mut column_bytes: std::collections::BTreeMap<usize, Vec<Vec<u8>>> =
        std::collections::BTreeMap::new();
    for element in elements {
        if let ConcatElement::Column { index } = element {
            if !column_bytes.contains_key(index) {
                column_bytes.insert(
                    *index,
                    authenticate_string_column(&values, *index, input.result_len)?,
                );
            }
        }
    }
    let mut out = Vec::with_capacity(input.result_len);
    for row in 0..input.result_len {
        let mut lexical: Vec<u8> = Vec::new();
        for element in elements {
            match element {
                // A trusted query literal contributes its bytes verbatim (no separator).
                ConcatElement::Literal(bytes) => lexical.extend_from_slice(bytes),
                // A root-authenticated string column contributes its row's content bytes.
                ConcatElement::Column { index } => {
                    lexical.extend_from_slice(&column_bytes[index][row]);
                }
            }
        }
        out.push(CanonicalObject::Value {
            datatype: CanonicalDatatype::String,
            lexical,
        });
    }
    Ok(out)
}

/// Interpose `separator` between consecutive `elements`, exactly as `utils:intersperse/3`
/// (`utils.pl:227`): `[] -> []`, `[e0] -> [e0]`, `[e0, e1, ...] -> [e0, Sep, e1, Sep, ...]`.
/// The separator is emitted ONLY between adjacent elements (never leading/trailing, never for a
/// single-element list). The resulting `ConcatElement` sequence is then concatenated verbatim,
/// reproducing `interpolate_string/2` over `intersperse`'s output = `utils:join/3`.
fn interpose_join_elements(
    elements: &[ConcatElement],
    separator: &ConcatElement,
) -> Vec<ConcatElement> {
    let mut interposed = Vec::with_capacity(elements.len().saturating_mul(2));
    for (position, element) in elements.iter().enumerate() {
        if position > 0 {
            interposed.push(separator.clone());
        }
        interposed.push(element.clone());
    }
    interposed
}

/// VERIFIER RECOMPUTE for WOQL `join` — `Concat`'s byte recompute over the INTERPOSED element
/// sequence.
///
/// FIDELITY: `utils:join/3` = `interpolate_string(intersperse(Sep, List))`. Since every list
/// element and the separator are xsd:strings, `interpolate_string` concatenates them VERBATIM
/// (its `string(H)` branch — no `term_to_atom`/`atom_string` rendering), so joining is precisely
/// concatenating `intersperse(Sep, List)`. We therefore build the interposed sequence
/// ([`interpose_join_elements`]) and delegate to [`expected_concat_objects`] — the SAME
/// root-authenticating byte recompute the concat circuit uses. SOUNDNESS is inherited verbatim:
/// operand bytes are pinned to the trusted root, a non-string operand row FAILS CLOSED (never
/// stringified), and byte concatenation is deterministic.
fn expected_join_objects(
    input: &ProvedBgp,
    input_plan: &CompiledRelation,
    elements: &[ConcatElement],
    separator: &ConcatElement,
    layer: &impl VerifierLayer,
    expected_root: ProofHash,
) -> std::io::Result<Vec<CanonicalObject>> {
    let interposed = interpose_join_elements(elements, separator);
    expected_concat_objects(input, input_plan, &interposed, layer, expected_root)
}

/// VERIFIER RECOMPUTE for WOQL `sub_string` — the per-row, CHARACTER-based substring.
///
/// FIDELITY (SWI builtin `sub_string/5`, character-based over code points): decode the
/// root-authenticated string column bytes as UTF-8 (a non-UTF-8 lexical FAILS CLOSED — xsd:string
/// is UTF-8, so this never occurs for a faithful store, and if it did we refuse rather than
/// mis-slice), then take the `length` Unicode scalar values starting at code-point offset
/// `before`. Rust `char` == code point for valid UTF-8, so this equals SWI's substring
/// byte-for-byte. When `before + length` exceeds the row's CHARACTER length, SWI `sub_string`
/// FAILS (drops the row) — we FAIL CLOSED here, so a proof is only produced when NO row is
/// dropped, in which case this per-row map equals the reference result exactly (never a
/// fabricated out-of-range substring). `before`/`length` are trusted compile-time constants.
///
/// SOUNDNESS: the operand bytes are pinned to the trusted root via
/// [`resolve_authenticated_child_values`]/[`authenticate_string_column`]; `before`/`length` are
/// bound into the query digest; char-slicing is deterministic — the caller's exact-match check
/// leaves no biasable witness.
fn expected_substring_objects(
    input: &ProvedBgp,
    input_plan: &CompiledRelation,
    string_column: usize,
    before: usize,
    length: usize,
    layer: &impl VerifierLayer,
    expected_root: ProofHash,
) -> std::io::Result<Vec<CanonicalObject>> {
    let values = resolve_authenticated_child_values(input, input_plan, layer, expected_root)?;
    let column_bytes = authenticate_string_column(&values, string_column, input.result_len)?;
    let mut out = Vec::with_capacity(input.result_len);
    for row_bytes in &column_bytes {
        let text = std::str::from_utf8(row_bytes).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "WOQL Substring operand row is not valid UTF-8",
            )
        })?;
        let total_chars = text.chars().count();
        // Non-negative already (validated at compile); `before + length` cannot overflow for any
        // realistic string but guard anyway. Out of range => reference `sub_string` FAILS =>
        // FAIL CLOSED (never fabricate a substring for a row the reference would drop).
        let end = before.checked_add(length).ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "WOQL Substring before+length overflow",
            )
        })?;
        if end > total_chars {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "WOQL Substring before+length exceeds operand character length (out of range: reference sub_string fails closed)",
            ));
        }
        let sub: String = text.chars().skip(before).take(length).collect();
        out.push(CanonicalObject::Value {
            datatype: CanonicalDatatype::String,
            lexical: sub.into_bytes(),
        });
    }
    Ok(out)
}

/// VERIFIER RECOMPUTE for WOQL `pad` — `utils:pad/4` (utils.pl:320) over a root-authenticated
/// xsd:string column, LEFT-padding each row to `target_len` CODE POINTS with `pad_char`.
///
/// FIDELITY: `pad_char` is a single code point (validated at compile), stored as its exact UTF-8
/// bytes; `L2 = target_len - char_count(text)`; prepending `pad_char`'s bytes `L2` times then the
/// operand bytes reproduces `repeat_term`/`append` byte-for-byte. When `char_count(text) >
/// target_len` (`L2 < 0`) the reference `utils:pad` THROWS `bad_pad_length` (the row has no
/// solution) — we FAIL CLOSED at recompute rather than fabricate a truncated result, so a proof is
/// only ever produced when NO row is dropped, in which case the per-row map equals the reference
/// exactly. A non-UTF-8 operand row also fails closed.
fn expected_pad_objects(
    input: &ProvedBgp,
    input_plan: &CompiledRelation,
    string_column: usize,
    pad_char: &[u8],
    target_len: usize,
    layer: &impl VerifierLayer,
    expected_root: ProofHash,
) -> std::io::Result<Vec<CanonicalObject>> {
    let values = resolve_authenticated_child_values(input, input_plan, layer, expected_root)?;
    let column_bytes = authenticate_string_column(&values, string_column, input.result_len)?;
    let mut out = Vec::with_capacity(input.result_len);
    for row_bytes in &column_bytes {
        let text = std::str::from_utf8(row_bytes).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "WOQL Pad operand row is not valid UTF-8",
            )
        })?;
        // `string_length` counts CODE POINTS; `chars()` yields Unicode scalar values (= code
        // points for valid UTF-8), so this equals SWI's char length.
        let char_count = text.chars().count();
        if char_count > target_len {
            // `L2 < 0` => reference `utils:pad` throws `bad_pad_length` => FAIL CLOSED.
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "WOQL Pad operand is longer than the pad target length (bad_pad_length: reference pad throws)",
            ));
        }
        let pad_count = target_len - char_count; // >= 0, and pad_char is exactly one code point
        let mut lexical: Vec<u8> = Vec::with_capacity(pad_char.len() * pad_count + row_bytes.len());
        for _ in 0..pad_count {
            lexical.extend_from_slice(pad_char); // PREPEND the padding (append(Padding, A, Total))
        }
        lexical.extend_from_slice(row_bytes);
        out.push(CanonicalObject::Value {
            datatype: CanonicalDatatype::String,
            lexical,
        });
    }
    Ok(out)
}

/// `true` iff byte `b` is passed through UNENCODED by the reference `encode_id_fragment`
/// (`src/core/document/json.pl:191` = SWI `uri_encoded(segment, ...)`), EXCLUDING `+` (which
/// `encode_id_fragment` subsequently rewrites to `%2B`). The exact set was captured from the
/// reference's own SWI `uri_encoded/3` builtin (all 95 printable ASCII probed): unreserved plus a
/// subset of `sub-delims`/`pchar` (`! $ & ' ( ) * , - . ; = @ _ ~`) plus alphanumerics. Every
/// passthrough byte is single-byte ASCII, so operating BYTE-WISE is exactly equivalent to SWI's
/// per-code-point encoding: any non-ASCII byte (>= 0x80, i.e. every byte of a multi-byte UTF-8
/// sequence) is >= 0x80, never collides with a passthrough byte, and is percent-encoded — matching
/// SWI's "encode to UTF-8 then percent-encode each byte" (verified: `é` -> `%C3%A9`).
fn is_idgen_passthrough_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric()
        || matches!(
            b,
            b'!' | b'$'
                | b'&'
                | b'\''
                | b'('
                | b')'
                | b'*'
                | b','
                | b'-'
                | b'.'
                | b';'
                | b'='
                | b'@'
                | b'_'
                | b'~'
        )
}

/// Reproduce the reference `encode_id_fragment/2` (`json.pl:191`) for a plain string operand:
/// `format(atom(F),'~w',[Elt])` (identity on an xsd:string's content) then `uri_encoded(segment,
/// F, P)` then `re_replace('+' -> '%2B', P)`. Passthrough bytes are emitted verbatim; `+` becomes
/// `%2B`; every other byte becomes `%XX` with UPPERCASE hex (matching SWI). The output is ALWAYS
/// pure ASCII (passthrough bytes are ASCII; encoded bytes are `%` + ASCII hex).
fn encode_id_fragment(bytes: &[u8]) -> Vec<u8> {
    const HEX_UPPER: &[u8; 16] = b"0123456789ABCDEF";
    let mut out = Vec::with_capacity(bytes.len());
    for &b in bytes {
        if is_idgen_passthrough_byte(b) {
            out.push(b);
        } else if b == b'+' {
            // uri_encoded(segment) leaves '+' as-is; encode_id_fragment then rewrites it to %2B.
            out.extend_from_slice(b"%2B");
        } else {
            out.push(b'%');
            out.push(HEX_UPPER[(b >> 4) as usize]);
            out.push(HEX_UPPER[(b & 0x0f) as usize]);
        }
    }
    out
}

/// Resolve one `ConcatElement`'s bytes for a given row: a trusted literal's bytes, or the
/// root-authenticated string column's row content (pre-authenticated into `column_bytes`).
fn concat_element_row_bytes<'a>(
    element: &'a ConcatElement,
    column_bytes: &'a std::collections::BTreeMap<usize, Vec<Vec<u8>>>,
    row: usize,
) -> &'a [u8] {
    match element {
        ConcatElement::Literal(bytes) => bytes,
        ConcatElement::Column { index } => &column_bytes[index][row],
    }
}

/// VERIFIER RECOMPUTE for WOQL `hash`/`HashKey` — the exact `idgen_hash/3` recipe (json.pl:467)
/// over root-authenticated `base` + `args`:
///   suffix = args.map(encode_id_fragment).join("+");  hash = lowercase_hex(sha256(suffix));
///   id = base ++ hash.
///
/// FIDELITY: `base` is RAW-prepended (never `idgen`-encoded — `format("~w~w",[Base,Hash])`); each
/// `arg` is `encode_id_fragment`'d (NO `raw/2` transform, since `literally` already reduced every
/// operand to a plain string — `raw/2` is identity on non-dicts, so no set-sorting applies);
/// fragments are joined by a literal `'+'` (`merge_separator_split(_, '+', _)`); the hash is the
/// lowercase 64-char hex SHA-256 of the (pure-ASCII) suffix bytes; and the digest is appended to
/// base. Every byte is authenticated against the trusted root (columns) or comes from the trusted
/// compiled query (literals); the id is revealed as a `CanonicalObject::Node` (the reference binds
/// a `NodeValue`/URI).
fn expected_hashkey_objects(
    input: &ProvedBgp,
    input_plan: &CompiledRelation,
    base: &ConcatElement,
    args: &[ConcatElement],
    layer: &impl VerifierLayer,
    expected_root: ProofHash,
) -> std::io::Result<Vec<CanonicalObject>> {
    let values = resolve_authenticated_child_values(input, input_plan, layer, expected_root)?;
    // Authenticate each referenced string column (base's and args') ONCE, keyed by column index.
    let mut column_bytes: std::collections::BTreeMap<usize, Vec<Vec<u8>>> =
        std::collections::BTreeMap::new();
    let mut authenticate = |element: &ConcatElement| -> std::io::Result<()> {
        if let ConcatElement::Column { index } = element {
            if !column_bytes.contains_key(index) {
                column_bytes.insert(
                    *index,
                    authenticate_string_column(&values, *index, input.result_len)?,
                );
            }
        }
        Ok(())
    };
    authenticate(base)?;
    for arg in args {
        authenticate(arg)?;
    }

    let mut out = Vec::with_capacity(input.result_len);
    for row in 0..input.result_len {
        // Build the idgen suffix: encode each arg fragment, join by '+'. Pure ASCII by construction.
        let mut suffix: Vec<u8> = Vec::new();
        for (position, arg) in args.iter().enumerate() {
            if position > 0 {
                suffix.push(b'+');
            }
            let arg_bytes = concat_element_row_bytes(arg, &column_bytes, row);
            suffix.extend_from_slice(&encode_id_fragment(arg_bytes));
        }
        // sha256 of the suffix bytes, lowercase hex (== SWI crypto_data_hash(..,[algorithm(sha256)])).
        let digest_hex = hex::encode(Sha256::digest(&suffix));
        // id = raw base ++ hex digest.
        let base_bytes = concat_element_row_bytes(base, &column_bytes, row);
        let mut id: Vec<u8> = Vec::with_capacity(base_bytes.len() + digest_hex.len());
        id.extend_from_slice(base_bytes);
        id.extend_from_slice(digest_hex.as_bytes());
        out.push(CanonicalObject::Node(id));
    }
    Ok(out)
}

fn expected_cardinality_column(
    result_len: usize,
    emit_zero_for_empty: bool,
) -> std::io::Result<Vec<Vec<ArkFr>>> {
    if result_len == 0 && !emit_zero_for_empty {
        return Ok(vec![Vec::new()]);
    }
    Ok(vec![vec![ArkFr::from_u64(u64::try_from(result_len).map_err(
        |_| std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "collection cardinality exceeds u64",
        ),
    )?)]])
}

fn expected_collection_sum_column(
    input: &ProvedBgp,
    input_plan: &CompiledRelation,
    template: CollectionSumTemplate,
    datatype: CanonicalDatatype,
    layer: &impl VerifierLayer,
    expected_root: ProofHash,
) -> std::io::Result<Vec<Vec<ArkFr>>> {
    let mut sum = GroupSumValue::zero(datatype)?;
    match template {
        CollectionSumTemplate::Column(column) => {
            let descriptors = relation_result_descriptors(input_plan)?;
            let values = resolve_result_values(
                layer,
                expected_root,
                &input.result_columns,
                &descriptors,
            )?;
            let values = values.get(column).ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "collection-sum value column out of range",
                )
            })?;
            if values.len() != input.result_len {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "collection-sum input column length mismatch",
                ));
            }
            for value in values {
                sum = sum.checked_add(integral_group_sum_value(value, datatype)?)?;
            }
        }
        CollectionSumTemplate::UnsignedConstant(value) => {
            for _ in 0..input.result_len {
                sum = sum.checked_add(GroupSumValue::Unsigned(value))?;
            }
        }
        CollectionSumTemplate::SignedConstant(value) => {
            for _ in 0..input.result_len {
                sum = sum.checked_add(GroupSumValue::Signed(i128::from(value)))?;
            }
        }
    }
    Ok(vec![vec![sum.field()?]])
}

fn expected_collection_extrema_column(
    input: &ProvedBgp,
    input_plan: &CompiledRelation,
    value_column: usize,
    datatype: CanonicalDatatype,
    kind: GroupExtremaKind,
    layer: &impl VerifierLayer,
    expected_root: ProofHash,
) -> std::io::Result<Vec<Vec<ArkFr>>> {
    let descriptors = relation_result_descriptors(input_plan)?;
    let values = resolve_result_values(
        layer,
        expected_root,
        &input.result_columns,
        &descriptors,
    )?;
    let values = values.get(value_column).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "collection-extrema value column out of range",
        )
    })?;
    let fields = input.result_columns.get(value_column).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "collection-extrema field column out of range",
        )
    })?;
    if values.len() != input.result_len || fields.len() != input.result_len {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "collection-extrema input column length mismatch",
        ));
    }
    let mut selected: Option<(ArkFr, GroupSumValue)> = None;
    for (field, value) in fields.iter().zip(values) {
        let numeric = integral_group_sum_value(value, datatype)?;
        let replace = match selected {
            None => true,
            Some((_, current)) => match kind {
                GroupExtremaKind::Min => numeric.cmp_same_domain(current)?.is_lt(),
                GroupExtremaKind::Max => numeric.cmp_same_domain(current)?.is_gt(),
            },
        };
        if replace {
            selected = Some((*field, numeric));
        }
    }
    Ok(vec![selected.map(|(field, _)| vec![field]).unwrap_or_default()])
}

impl ProvedBgp {
    pub fn as_single(&self) -> Option<(&LayerBgpProof, &[u64])> {
        match &self.statement {
            ProvedQueryStatement::Single {
                proof,
                predicate_ids,
            } => Some((proof, predicate_ids)),
            _ => None,
        }
    }

    pub fn as_bag_union(&self) -> Option<(&[ProvedBgp], &BagUnionProof)> {
        match &self.statement {
            ProvedQueryStatement::BagUnion { branches, proof } => Some((branches, proof)),
            _ => None,
        }
    }

    /// The per-row synthesized strings of a top-level string op (currently `concat`), each a
    /// TERMINAL revealed `xsd:string` the verifier RE-DERIVES from root-authenticated operands.
    /// `None` for a non-string-op proof. Reading these AFTER a successful `verify` is sound:
    /// `verify` re-derives and requires them to equal the verifier recomputation, so a value
    /// returned here is exactly the store-faithful concatenation (never prover-chosen).
    pub fn concat_result_objects(&self) -> Option<&[CanonicalObject]> {
        match &self.statement {
            ProvedQueryStatement::Concat { result_objects, .. } => Some(result_objects),
            _ => None,
        }
    }

    /// The per-row synthesized strings of a top-level `Join`, each a TERMINAL revealed
    /// `xsd:string` the verifier RE-DERIVES (separator interposed) from root-authenticated
    /// operands. `None` for a non-join proof. Reading these AFTER a successful `verify` is sound
    /// for the same reason as [`Self::concat_result_objects`].
    pub fn join_result_objects(&self) -> Option<&[CanonicalObject]> {
        match &self.statement {
            ProvedQueryStatement::Join { result_objects, .. } => Some(result_objects),
            _ => None,
        }
    }

    /// The per-row synthesized substrings of a top-level `Substring`, each a TERMINAL revealed
    /// `xsd:string` the verifier RE-DERIVES by char-slicing the root-authenticated string column.
    /// `None` for a non-substring proof. Reading these AFTER a successful `verify` is sound for
    /// the same reason as [`Self::concat_result_objects`].
    pub fn substring_result_objects(&self) -> Option<&[CanonicalObject]> {
        match &self.statement {
            ProvedQueryStatement::Substring { result_objects, .. } => Some(result_objects),
            _ => None,
        }
    }

    /// The per-row synthesized strings of a top-level `Pad`, each a TERMINAL revealed `xsd:string`
    /// the verifier RE-DERIVES by left-padding the root-authenticated string column to the trusted
    /// target length. `None` for a non-pad proof. Reading these AFTER a successful `verify` is
    /// sound for the same reason as [`Self::concat_result_objects`].
    pub fn pad_result_objects(&self) -> Option<&[CanonicalObject]> {
        match &self.statement {
            ProvedQueryStatement::Pad { result_objects, .. } => Some(result_objects),
            _ => None,
        }
    }

    /// The per-row synthesized document-ID URIs of a top-level `HashKey`/`hash`, each a TERMINAL
    /// revealed `CanonicalObject::Node` the verifier RE-DERIVES via the exact `idgen_hash` recipe
    /// over the root-authenticated base + args. `None` for a non-hash proof. Reading these AFTER a
    /// successful `verify` is sound for the same reason as [`Self::concat_result_objects`].
    pub fn hashkey_result_objects(&self) -> Option<&[CanonicalObject]> {
        match &self.statement {
            ProvedQueryStatement::HashKey { result_objects, .. } => Some(result_objects),
            _ => None,
        }
    }

    /// The per-row synthesized `xsd:decimal` results of a top-level exact-rational arithmetic
    /// (`Divide`/decimal-operand `Plus`/`Minus`/`Times`/`Exp`), each a TERMINAL revealed
    /// `CanonicalObject::Value` (datatype `xsd:decimal`) the verifier RE-DERIVES by re-evaluating the
    /// exact rational and re-rendering the canonical 20-fractional-digit lexical. `None` for a
    /// non-rational-arithmetic proof. Reading these AFTER a successful `verify` is sound for the same
    /// reason as [`Self::concat_result_objects`].
    pub fn rational_arith_result_objects(&self) -> Option<&[CanonicalObject]> {
        match &self.statement {
            ProvedQueryStatement::RationalArith { result_objects, .. } => Some(result_objects),
            _ => None,
        }
    }

    /// The per-row synthesized `xsd:date` results of a top-level DATE-result calendar op
    /// (`DayAfter`/`DayBefore`/`MonthStartDate`/`MonthEndDate`), each a TERMINAL revealed
    /// `CanonicalObject::Value` (datatype `xsd:date`, client-facing `YYYY-MM-DD` lexical) the verifier
    /// RE-DERIVES from the root-authenticated operand. `None` for any other proof. Reading these AFTER
    /// a successful `verify` is sound for the same reason as [`Self::concat_result_objects`].
    pub fn calendar_date_result_objects(&self) -> Option<&[CanonicalObject]> {
        match &self.statement {
            ProvedQueryStatement::CalendarDate { result_objects, .. } => Some(result_objects),
            _ => None,
        }
    }

    /// The per-row synthesized `xsd:string` results of a top-level NUMBER→STRING `Typecast`, each a
    /// TERMINAL revealed `CanonicalObject::Value` (datatype `xsd:string`) the verifier RE-DERIVES by
    /// rendering the root-authenticated numeric source. `None` for any other proof. Reading these
    /// AFTER a successful `verify` is sound for the same reason as [`Self::concat_result_objects`].
    pub fn typecast_to_string_result_objects(&self) -> Option<&[CanonicalObject]> {
        match &self.statement {
            ProvedQueryStatement::TypecastNumberToString { result_objects, .. } => {
                Some(result_objects)
            }
            _ => None,
        }
    }

    /// The per-row synthesized numeric results of a top-level STRING→NUMBER `Typecast`, each a
    /// TERMINAL revealed `CanonicalObject::Value` (datatype `xsd:integer`=`BigInt` or `xsd:decimal`)
    /// the verifier RE-DERIVES by re-parsing the root-authenticated `xsd:string` source. `None` for
    /// any other proof. Reading these AFTER a successful `verify` is sound for the same reason as
    /// [`Self::concat_result_objects`].
    pub fn typecast_from_string_result_objects(&self) -> Option<&[CanonicalObject]> {
        match &self.statement {
            ProvedQueryStatement::TypecastStringToNumber { result_objects, .. } => {
                Some(result_objects)
            }
            _ => None,
        }
    }

    /// Verify against a caller-selected, independently trusted root.
    ///
    /// LOW-LEVEL primitive, INSTANCE-ONLY: verifies the plan against the SINGLE `layer` +
    /// `expected_root` the caller supplies (byte-identical to before the schema-graph foundation for
    /// instance queries). It is the caller's responsibility to pass the correct per-graph view + root.
    ///
    /// A query that reads the SCHEMA graph (`Subsumption`) REJECTS closed here — the mirror of the
    /// [`CompiledBgp::prove_on_layer`] prove-side guard. A single caller-supplied `expected_root` is
    /// NOT tied to the trusted combined `database_root`, so a prover could hand a verifier a FABRICATED
    /// schema view + its matching root and have this low-level entry accept a forged superclass. Schema
    /// queries MUST go through the SAFE two-graph entry [`ProvedBgp::verify_over_graphs`], which
    /// reconstructs the read-graph from the trusted query and anchors to the matching commitment via
    /// the combined `database_root`.
    pub fn verify(
        &self,
        compiled: &CompiledBgp,
        layer: &impl VerifierLayer,
        expected_root: ProofHash,
    ) -> std::io::Result<()> {
        // SCHEMA-GRAPH GUARD (mirror of `prove_on_layer`): reject a schema-reading query on this
        // single-root path — it must be verified through `verify_over_graphs` (combined database_root),
        // which binds the schema commitment to the trusted anchor. Instance queries are unaffected.
        if relation_graph(&compiled.relation)? != ProofGraph::Instance {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "query reads the schema graph; verify with `verify_over_graphs` (instance + schema commitments under the trusted database_root)",
            ));
        }
        self.verify_relation(
            &compiled.relation,
            layer,
            expected_root,
            compiled_query_digest(compiled)?,
        )
    }

    /// SCHEMA-GRAPH FOUNDATION — verify a proof over a TWO-graph database.
    ///
    /// Trust flows from the single externally-supplied `database_root`:
    ///   1. Validate BOTH per-graph verifier commitments independently AND confirm their roots combine
    ///      to `database_root` ([`terminus_store::proof::commitment::validate_database_root`]). Neither
    ///      per-graph root can be forged, swapped, or mismatched against the trusted anchor.
    ///   2. Reconstruct — from the TRUSTED compiled query — which single graph this query reads
    ///      ([`relation_graph`]); the graph selector is bound into the query digest, so a prover cannot
    ///      make a schema query verify against the instance graph (or vice versa).
    ///   3. Anchor the audited recursion to the matching commitment + its (now-trusted) root.
    ///
    /// An instance-only query verifies identically to [`verify`], with the added guarantee that its
    /// instance root belongs to the trusted database.
    ///
    /// TRUST ANCHOR — the `database_root` argument MUST be obtained from a trusted OUT-OF-BAND pin
    /// (the commit log / a signature over the head), NEVER recomputed from the prover-supplied proof
    /// bundle or from a prover-supplied schema view. If a verifier lets the prover choose both the
    /// schema view AND the `database_root`, the prover can publish a FABRICATED schema (real ∪ a forged
    /// `sys:inherits` edge), commit it, combine its root into a matching `database_root`, and this entry
    /// will validate the pair — the forgery is only excluded because the trusted `database_root` is
    /// fixed independently of the proof.
    pub fn verify_over_graphs(
        &self,
        compiled: &CompiledBgp,
        instance: &impl VerifierLayer,
        schema: &impl VerifierLayer,
        database_root: ProofHash,
    ) -> std::io::Result<()> {
        // The cross-root hash join (IsA) engages BOTH roots: route to the two-layer store verify, which
        // validates both per-graph commitments against the trusted `database_root` itself.
        if matches!(compiled.relation.statement, CompiledRelationStatement::CrossRootHashJoin { .. }) {
            let digest = compiled_query_digest(compiled)?;
            let plan = relation_proof_plan(&compiled.relation, digest)?;
            return match &self.statement {
                ProvedQueryStatement::CrossRootHashJoin { envelope } => {
                    envelope.verify_cross_root(instance, schema, database_root, &plan)
                }
                _ => Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "WOQL cross-root proof/plan mismatch",
                )),
            };
        }
        // (1) Both per-graph commitments validated + combined into the trusted database anchor.
        terminus_store::proof::commitment::validate_database_root(instance, schema, &database_root)?;
        let digest = compiled_query_digest(compiled)?;
        // (2)+(3) Reconstruct the read-graph from the trusted query and anchor to that commitment.
        match relation_graph(&compiled.relation)? {
            ProofGraph::Instance => {
                let root = instance.verifier_view().state.commitment_root;
                self.verify_relation(&compiled.relation, instance, root, digest)
            }
            ProofGraph::Schema => {
                let root = schema.verifier_view().state.commitment_root;
                self.verify_relation(&compiled.relation, schema, root, digest)
            }
        }
    }

    /// [`verify_over_graphs`] that also RESOLVES the delivered result objects (rendered against the
    /// correct per-graph dictionary — e.g. Subsumption class IRIs come out of the SCHEMA node/value
    /// dictionary). Same trust flow: validate both commitments against the trusted `database_root`,
    /// reconstruct the read-graph, verify + resolve against that graph's view.
    pub fn verify_over_graphs_and_resolve(
        &self,
        compiled: &CompiledBgp,
        instance: &impl VerifierLayer,
        schema: &impl VerifierLayer,
        database_root: ProofHash,
    ) -> std::io::Result<Vec<Vec<CanonicalObject>>> {
        // The cross-root hash join (IsA): verify through the two-layer store entry (which binds both
        // roots to the trusted `database_root`), then resolve the delivered `?x` ids against the
        // INSTANCE dictionary (the output lives in the instance namespace).
        if matches!(compiled.relation.statement, CompiledRelationStatement::CrossRootHashJoin { .. }) {
            let digest = compiled_query_digest(compiled)?;
            let plan = relation_proof_plan(&compiled.relation, digest)?;
            match &self.statement {
                ProvedQueryStatement::CrossRootHashJoin { envelope } => {
                    envelope.verify_cross_root(instance, schema, database_root, &plan)?;
                }
                _ => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "WOQL cross-root proof/plan mismatch",
                    ))
                }
            }
            if compiled.relation.schema.is_empty() {
                return Ok(Vec::new());
            }
            let instance_root = instance.verifier_view().state.commitment_root;
            let namespaces = compiled_result_descriptors(compiled)?
                .into_iter()
                .map(|descriptor| descriptor.namespace)
                .collect::<Vec<_>>();
            return resolve_typed_result_objects(
                instance, instance_root, &self.result_columns, &namespaces,
            );
        }
        terminus_store::proof::commitment::validate_database_root(instance, schema, &database_root)?;
        // Both commitments are now anchored to the trusted `database_root`; select the correct
        // per-graph view and resolve through the UNGUARDED core (the read-graph guard on
        // `verify_and_resolve` would otherwise reject the legitimate schema branch on this SAFE path).
        match relation_graph(&compiled.relation)? {
            ProofGraph::Instance => {
                let root = instance.verifier_view().state.commitment_root;
                self.verify_and_resolve_inner(compiled, instance, root)
            }
            ProofGraph::Schema => {
                let root = schema.verifier_view().state.commitment_root;
                self.verify_and_resolve_inner(compiled, schema, root)
            }
        }
    }

    fn relation_commit(&self, mapping: Option<&[usize]>) -> std::io::Result<RelationCommit> {
        let commitments = match mapping {
            Some(mapping) => mapping
                .iter()
                .map(|index| {
                    self.result_commitments.get(*index).copied().ok_or_else(|| {
                        std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "WOQL recursive relation mapping out of range",
                        )
                    })
                })
                .collect::<std::io::Result<Vec<_>>>()?,
            None => self.result_commitments.clone(),
        };
        Ok(RelationCommit {
            commitments,
            log_domain: self.result_log_domain,
            n: self.result_len,
        })
    }

    fn verify_relation(
        &self,
        compiled: &CompiledRelation,
        layer: &impl VerifierLayer,
        expected_root: ProofHash,
        query_digest: ProofHash,
    ) -> std::io::Result<()> {
        if let CompiledRelationStatement::When { expanded } = &compiled.statement {
            return self.verify_relation(expanded, layer, expected_root, query_digest);
        }
        match (&self.statement, &compiled.statement) {
            (
                ProvedQueryStatement::LayerTripleCount,
                CompiledRelationStatement::LayerTripleCount,
            ) => {
                let count = layer.verifier_view().state.materialized_count;
                let mut column = vec![ArkFr::zero(); 2];
                column[0] = ArkFr::from_u64(count);
                if self.result_len != 1
                    || self.result_log_domain != 1
                    || self.result_columns != vec![vec![column[0]]]
                    || self.result_commitments != vec![commit_column(&column)?]
                {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "WOQL layer triple-count result does not match trusted root",
                    ));
                }
            }
            (
                ProvedQueryStatement::GeneratedU64Filter { input },
                CompiledRelationStatement::GeneratedU64Filter {
                    input: input_plan,
                    column,
                    op,
                    constant,
                    constant_on_left,
                },
            ) => {
                input.verify_relation(
                    input_plan,
                    layer,
                    expected_root,
                    relation_query_digest(input_plan)?,
                )?;
                let expected_columns = expected_generated_u64_filter_columns(
                    input,
                    *column,
                    *op,
                    *constant,
                    *constant_on_left,
                )?;
                let expected_len = expected_columns.first().map_or(0, Vec::len);
                let expected_log = expected_len
                    .max(1)
                    .next_power_of_two()
                    .max(2)
                    .trailing_zeros() as usize;
                if self.result_len != expected_len
                    || self.result_log_domain != expected_log
                    || self.result_columns != expected_columns
                {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "WOQL generated-u64 comparison is not the exact derived selection",
                    ));
                }
            }
            (
                ProvedQueryStatement::GeneratedI64Filter { input },
                CompiledRelationStatement::GeneratedI64Filter {
                    input: input_plan,
                    column,
                    op,
                    constant,
                    constant_on_left,
                },
            ) => {
                input.verify_relation(
                    input_plan,
                    layer,
                    expected_root,
                    relation_query_digest(input_plan)?,
                )?;
                let expected_columns = expected_generated_i64_filter_columns(
                    input,
                    *column,
                    *op,
                    *constant,
                    *constant_on_left,
                )?;
                let expected_len = expected_columns.first().map_or(0, Vec::len);
                let expected_log = expected_len
                    .max(1)
                    .next_power_of_two()
                    .max(2)
                    .trailing_zeros() as usize;
                if self.result_len != expected_len
                    || self.result_log_domain != expected_log
                    || self.result_columns != expected_columns
                {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "WOQL generated-i64 comparison is not the exact derived selection",
                    ));
                }
            }
            (
                ProvedQueryStatement::Single {
                    proof,
                    predicate_ids,
                },
                CompiledRelationStatement::Bgp(plan),
            ) => {
                verify_layer_bgp(
                    layer,
                    expected_root,
                    plan,
                    predicate_ids,
                    &self.result_commitments,
                    self.result_len,
                    proof,
                )?;
                verify_layer_bgp_result_log_domain(layer, plan, proof, self.result_log_domain)?;
            }
            (
                ProvedQueryStatement::BagUnion { branches, proof },
                CompiledRelationStatement::BagUnion {
                    branches: branch_plans,
                    mappings,
                },
            ) => {
                if branches.len() < 2
                    || branches.len() != branch_plans.len()
                    || branches.len() != mappings.len()
                {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "WOQL recursive union branch mismatch",
                    ));
                }
                let mut inputs = Vec::with_capacity(branches.len());
                for ((branch, branch_plan), mapping) in
                    branches.iter().zip(branch_plans).zip(mappings)
                {
                    branch.verify_relation(
                        branch_plan,
                        layer,
                        expected_root,
                        relation_query_digest(branch_plan)?,
                    )?;
                    inputs.push(if compiled.schema.is_empty() {
                        branch.relation_commit(None)?
                    } else {
                        branch.relation_commit(Some(mapping))?
                    });
                }
                let mut transcript = Transcript254::new(b"parture-union-envelope");
                transcript.append_bytes(b"pow/union-envelope/root", &expected_root);
                transcript.append_bytes(b"pow/union-envelope/query", &query_digest);
                verify_bag_union(
                    &inputs,
                    &self.result_commitments,
                    self.result_len,
                    self.result_log_domain,
                    proof,
                    &mut transcript,
                )?;
            }
            (
                ProvedQueryStatement::BagProjection { input },
                CompiledRelationStatement::BagProjection {
                    input: input_plan,
                    indices,
                },
            ) => {
                input.verify_relation(
                    input_plan,
                    layer,
                    expected_root,
                    relation_query_digest(input_plan)?,
                )?;
                let (expected_commitments, expected_columns) = if indices.is_empty() {
                    let mut unit = vec![ArkFr::zero(); 1usize << input.result_log_domain];
                    unit[..input.result_len].fill(ArkFr::one());
                    (
                        vec![terminus_store::proof::argument::commit_column(&unit)?],
                        vec![unit[..input.result_len].to_vec()],
                    )
                } else {
                    let commitments = indices
                        .iter()
                        .map(|index| {
                            input.result_commitments.get(*index).copied().ok_or_else(|| {
                                std::io::Error::new(
                                    std::io::ErrorKind::InvalidData,
                                    "WOQL bag projection index out of range",
                                )
                            })
                        })
                        .collect::<std::io::Result<Vec<_>>>()?;
                    let columns = indices
                        .iter()
                        .map(|index| {
                            input.result_columns.get(*index).cloned().ok_or_else(|| {
                                std::io::Error::new(
                                    std::io::ErrorKind::InvalidData,
                                    "WOQL bag projection index out of range",
                                )
                            })
                        })
                        .collect::<std::io::Result<Vec<_>>>()?;
                    (commitments, columns)
                };
                if self.result_commitments != expected_commitments
                    || self.result_columns != expected_columns
                    || self.result_len != input.result_len
                    || self.result_log_domain != input.result_log_domain
                {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "WOQL bag projection changed rows or multiplicity",
                    ));
                }
            }
            (
                ProvedQueryStatement::Distinct { input, proof },
                CompiledRelationStatement::Distinct {
                    input: input_plan,
                    key_columns,
                },
            ) => {
                input.verify_relation(
                    input_plan,
                    layer,
                    expected_root,
                    relation_query_digest(input_plan)?,
                )?;
                let rows = distinct_output_rows(
                    &self.result_columns,
                    self.result_len,
                    compiled,
                    layer,
                )?;
                let mut transcript = Transcript254::new(b"parture-distinct-envelope");
                transcript.append_bytes(b"pow/distinct-envelope/root", &expected_root);
                transcript.append_bytes(b"pow/distinct-envelope/query", &query_digest);
                verify_relation_distinct(
                    &input.relation_commit(None)?,
                    key_columns,
                    &rows,
                    proof,
                    &mut transcript,
                )?;
            }
            (
                ProvedQueryStatement::LeftAnti { left, right, proof },
                CompiledRelationStatement::LeftAnti {
                    left: left_plan,
                    right: right_plan,
                    key_pairs,
                },
            ) => {
                left.verify_relation(
                    left_plan,
                    layer,
                    expected_root,
                    relation_query_digest(left_plan)?,
                )?;
                right.verify_relation(
                    right_plan,
                    layer,
                    expected_root,
                    relation_query_digest(right_plan)?,
                )?;
                let left_commit = left.relation_commit(None)?;
                let right_commit = right.relation_commit(None)?;
                let mut transcript = Transcript254::new(b"parture-left-anti-envelope");
                transcript.append_bytes(b"pow/anti-envelope/root", &expected_root);
                transcript.append_bytes(b"pow/anti-envelope/query", &query_digest);
                match (key_pairs.is_empty(), proof.as_ref()) {
                    (true, None) => verify_global_not(
                        &left_commit,
                        &right_commit,
                        &self.result_commitments,
                        self.result_len,
                        self.result_log_domain,
                    )?,
                    (false, Some(proof)) => verify_left_anti_join_with_keys(
                        &left_commit,
                        &right_commit,
                        key_pairs,
                        &self.result_commitments,
                        self.result_len,
                        self.result_log_domain,
                        proof,
                        &mut transcript,
                    )?,
                    _ => {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "WOQL recursive anti proof variant mismatch",
                        ))
                    }
                }
            }
            (
                ProvedQueryStatement::LeftOuter { left, right, proof },
                CompiledRelationStatement::LeftOuter {
                    left: left_plan,
                    right: right_plan,
                    key_pairs,
                },
            ) => {
                left.verify_relation(
                    left_plan,
                    layer,
                    expected_root,
                    relation_query_digest(left_plan)?,
                )?;
                right.verify_relation(
                    right_plan,
                    layer,
                    expected_root,
                    relation_query_digest(right_plan)?,
                )?;
                let left_commit = left.relation_commit(None)?;
                let right_commit = right.relation_commit(None)?;
                let mut transcript = Transcript254::new(b"parture-left-outer-envelope");
                transcript.append_bytes(b"pow/outer-envelope/root", &expected_root);
                transcript.append_bytes(b"pow/outer-envelope/query", &query_digest);
                if key_pairs.is_empty() {
                    verify_global_left_outer_join(
                        &left_commit,
                        &right_commit,
                        &self.result_commitments,
                        self.result_len,
                        self.result_log_domain,
                        proof,
                        &mut transcript,
                    )?;
                } else {
                    verify_left_outer_join_with_keys(
                        &left_commit,
                        &right_commit,
                        key_pairs,
                        &self.result_commitments,
                        self.result_len,
                        self.result_log_domain,
                        proof,
                        &mut transcript,
                    )?;
                }
            }
            (
                ProvedQueryStatement::InnerJoin { left, right, proof },
                CompiledRelationStatement::InnerJoin { left: left_plan, right: right_plan, key_pairs },
            ) => {
                left.verify_relation(left_plan, layer, expected_root, relation_query_digest(left_plan)?)?;
                right.verify_relation(right_plan, layer, expected_root, relation_query_digest(right_plan)?)?;
                let mut transcript = Transcript254::new(b"parture-inner-join-envelope");
                transcript.append_bytes(b"pow/inner-envelope/root", &expected_root);
                transcript.append_bytes(b"pow/inner-envelope/query", &query_digest);
                verify_inner_join_with_keys(
                    &left.relation_commit(None)?, &right.relation_commit(None)?, key_pairs,
                    &self.result_commitments, self.result_len, self.result_log_domain,
                    proof, &mut transcript,
                )?;
            }
            (
                ProvedQueryStatement::Equality { input, proof },
                CompiledRelationStatement::Equality {
                    input: input_plan,
                    constraint,
                },
            ) => {
                input.verify_relation(
                    input_plan,
                    layer,
                    expected_root,
                    relation_query_digest(input_plan)?,
                )?;
                let resolved = resolve_relation_equality_constraint(layer, constraint)?;
                let mut transcript = Transcript254::new(b"parture-equality-envelope");
                transcript.append_bytes(b"pow/equality-envelope/root", &expected_root);
                transcript.append_bytes(b"pow/equality-envelope/query", &query_digest);
                verify_relation_equality(
                    &input.relation_commit(None)?,
                    &resolved,
                    &self.result_commitments,
                    self.result_len,
                    self.result_log_domain,
                    proof,
                    &mut transcript,
                )?;
            }
            (
                ProvedQueryStatement::TupleDisequality { input, proof },
                CompiledRelationStatement::TupleDisequality {
                    input: input_plan,
                    pairs,
                },
            ) => {
                input.verify_relation(
                    input_plan,
                    layer,
                    expected_root,
                    relation_query_digest(input_plan)?,
                )?;
                let resolved = resolve_tuple_disequality_pairs(layer, pairs)?;
                let mut transcript = Transcript254::new(b"parture-tuple-disequality-envelope");
                transcript.append_bytes(b"pow/tuple-disequality-envelope/root", &expected_root);
                transcript.append_bytes(b"pow/tuple-disequality-envelope/query", &query_digest);
                verify_tuple_disequality(
                    &input.relation_commit(None)?,
                    &resolved,
                    &self.result_commitments,
                    self.result_len,
                    self.result_log_domain,
                    proof,
                    &mut transcript,
                )?;
            }
            (
                ProvedQueryStatement::GroupExtrema { input },
                CompiledRelationStatement::GroupExtrema {
                    input: input_plan,
                    key_columns,
                    value_column,
                    datatype,
                    kind,
                },
            ) => {
                input.verify_relation(
                    input_plan,
                    layer,
                    expected_root,
                    relation_query_digest(input_plan)?,
                )?;
                let expected_columns = expected_group_extrema_columns(
                    input,
                    input_plan,
                    key_columns,
                    *value_column,
                    *datatype,
                    *kind,
                    layer,
                    expected_root,
                )?;
                let expected_len = expected_columns.first().map_or(0, Vec::len);
                let expected_log = expected_len.max(1).next_power_of_two().max(2)
                    .trailing_zeros() as usize;
                if self.result_len != expected_len
                    || self.result_log_domain != expected_log
                    || self.result_columns != expected_columns
                {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "WOQL group-extrema result is not the exact canonical aggregate",
                    ));
                }
            }
            (
                ProvedQueryStatement::GroupSum { input },
                CompiledRelationStatement::GroupSum {
                    input: input_plan,
                    key_columns,
                    value_column,
                    datatype,
                },
            ) => {
                input.verify_relation(
                    input_plan,
                    layer,
                    expected_root,
                    relation_query_digest(input_plan)?,
                )?;
                let expected_columns = expected_group_sum_columns(
                    input,
                    input_plan,
                    key_columns,
                    *value_column,
                    *datatype,
                    layer,
                    expected_root,
                )?;
                let expected_len = expected_columns.first().map_or(0, Vec::len);
                let expected_log = expected_len
                    .max(1)
                    .next_power_of_two()
                    .max(2)
                    .trailing_zeros() as usize;
                if self.result_len != expected_len
                    || self.result_log_domain != expected_log
                    || self.result_columns != expected_columns
                {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "WOQL group-sum result is not the exact derived aggregate",
                    ));
                }
            }
            (
                ProvedQueryStatement::GroupCount { input },
                CompiledRelationStatement::GroupCount {
                    input: input_plan,
                    key_columns,
                },
            ) => {
                input.verify_relation(
                    input_plan,
                    layer,
                    expected_root,
                    relation_query_digest(input_plan)?,
                )?;
                let expected_columns = expected_group_count_columns(
                    input,
                    input_plan,
                    key_columns,
                    layer,
                    expected_root,
                )?;
                let expected_len = expected_columns.first().map_or(0, Vec::len);
                let expected_log = expected_len
                    .max(1)
                    .next_power_of_two()
                    .max(2)
                    .trailing_zeros() as usize;
                if self.result_len != expected_len
                    || self.result_log_domain != expected_log
                    || self.result_columns != expected_columns
                {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "WOQL group-count result is not the exact derived aggregate",
                    ));
                }
            }
            (
                ProvedQueryStatement::CollectionCount { input },
                CompiledRelationStatement::CollectionCount { input: input_plan },
            ) => {
                input.verify_relation(
                    input_plan,
                    layer,
                    expected_root,
                    relation_query_digest(input_plan)?,
                )?;
                let expected_columns = expected_cardinality_column(input.result_len, true)?;
                if self.result_len != 1
                    || self.result_log_domain != 1
                    || self.result_columns != expected_columns
                {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "WOQL collection-count result is not the exact derived cardinality",
                    ));
                }
            }
            (
                ProvedQueryStatement::CollectionSum { input },
                CompiledRelationStatement::CollectionSum {
                    input: input_plan,
                    template,
                    datatype,
                },
            ) => {
                input.verify_relation(
                    input_plan,
                    layer,
                    expected_root,
                    relation_query_digest(input_plan)?,
                )?;
                let expected_columns = expected_collection_sum_column(
                    input,
                    input_plan,
                    *template,
                    *datatype,
                    layer,
                    expected_root,
                )?;
                if self.result_len != 1
                    || self.result_log_domain != 1
                    || self.result_columns != expected_columns
                {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "WOQL collection-sum result is not the exact derived aggregate",
                    ));
                }
            }
            (
                ProvedQueryStatement::CollectionExtrema { input },
                CompiledRelationStatement::CollectionExtrema {
                    input: input_plan,
                    value_column,
                    datatype,
                    kind,
                },
            ) => {
                input.verify_relation(
                    input_plan,
                    layer,
                    expected_root,
                    relation_query_digest(input_plan)?,
                )?;
                let expected_columns = expected_collection_extrema_column(
                    input,
                    input_plan,
                    *value_column,
                    *datatype,
                    *kind,
                    layer,
                    expected_root,
                )?;
                let expected_len = expected_columns[0].len();
                if self.result_len != expected_len
                    || self.result_log_domain != 1
                    || self.result_columns != expected_columns
                {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "WOQL collection-extrema result is not the exact derived extremum",
                    ));
                }
            }
            (
                ProvedQueryStatement::Eval { input },
                CompiledRelationStatement::Eval {
                    input: input_plan,
                    expr,
                },
            ) => {
                // (1) Authenticate the operand-binding child against the trusted root. The
                //     child's own `verify_relation` ends with `verify_columns`, binding every
                //     operand column to the root — so operand VALUES are the store's values,
                //     with no prover latitude.
                input.verify_relation(
                    input_plan,
                    layer,
                    expected_root,
                    relation_query_digest(input_plan)?,
                )?;
                // (2) VERIFIER RE-EVALUATION: recompute the whole tree elementwise over the
                //     root-authenticated operand columns (+ query-local literals) in checked
                //     i128, narrowing to the declared i64 generated domain. The derived column
                //     is appended to the (also authenticated) child columns; the result must
                //     match exactly. Row count and domain are inherited from the child (Eval
                //     is a per-row map, not a filter or aggregate).
                let expected_columns =
                    expected_eval_columns(input, input_plan, expr, layer, expected_root)?;
                if self.result_len != input.result_len
                    || self.result_log_domain != input.result_log_domain
                    || self.result_columns != expected_columns
                {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "WOQL Eval result is not the exact verifier-recomputed arithmetic column",
                    ));
                }
            }
            (
                ProvedQueryStatement::Typecast { input },
                CompiledRelationStatement::Typecast {
                    input: input_plan,
                    source_column,
                    source_datatype,
                    target_datatype,
                },
            ) => {
                // (1) Authenticate the source-binding child against the trusted root — its own
                //     `verify_relation` ends with `verify_columns`, pinning the source column to
                //     the root, so the source VALUE is the store's value with no prover latitude.
                input.verify_relation(
                    input_plan,
                    layer,
                    expected_root,
                    relation_query_digest(input_plan)?,
                )?;
                // (2) VERIFIER RE-DERIVATION: recompute the cast column from the root-authenticated
                //     source (target-range check included — an out-of-range value fails closed,
                //     matching Prolog's casting_error throw) and require an exact match. Row count
                //     and domain are inherited from the child (Typecast is a per-row map).
                let expected_columns = expected_typecast_column(
                    input,
                    input_plan,
                    *source_column,
                    *source_datatype,
                    *target_datatype,
                    layer,
                    expected_root,
                )?;
                if self.result_len != input.result_len
                    || self.result_log_domain != input.result_log_domain
                    || self.result_columns != expected_columns
                {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "WOQL Typecast result is not the exact verifier-recomputed cast column",
                    ));
                }
            }
            (
                ProvedQueryStatement::Concat {
                    input,
                    result_objects,
                },
                CompiledRelationStatement::Concat {
                    input: input_plan,
                    elements,
                    ..
                },
            ) => {
                // (1) Authenticate the operand-binding child against the trusted root. The
                //     child's own `verify_relation` ends with `verify_columns`, binding every
                //     operand column to the root — so operand string bytes are the store's
                //     values, with no prover latitude.
                input.verify_relation(
                    input_plan,
                    layer,
                    expected_root,
                    relation_query_digest(input_plan)?,
                )?;
                // (2) TERMINAL, REVEALED result: the committed columns are the child's operand
                //     columns VERBATIM (no committed result column — a synthesized string does
                //     not encode into a field element and feeds nothing downstream). Pin the
                //     carried-through columns and the inherited row count/domain (concat is a
                //     per-row map, not a filter/aggregate).
                if self.result_len != input.result_len
                    || self.result_log_domain != input.result_log_domain
                    || self.result_columns != input.result_columns
                {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "WOQL Concat must carry the child columns through unchanged",
                    ));
                }
                // (3) VERIFIER RE-DERIVATION: recompute the per-row concatenation over the
                //     root-authenticated operands (+ query literals) and require the DELIVERED
                //     result strings to match EXACTLY. A tampered/wrong result-string column, a
                //     tampered operand (which shifts the recomputation), or a non-string operand
                //     row all fail closed. There is no biasable witness — the inputs are pinned
                //     to the trusted root and byte concatenation is deterministic.
                let expected_objects =
                    expected_concat_objects(input, input_plan, elements, layer, expected_root)?;
                if *result_objects != expected_objects {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "WOQL Concat result is not the exact verifier-recomputed concatenation",
                    ));
                }
            }
            (
                ProvedQueryStatement::Join {
                    input,
                    result_objects,
                },
                CompiledRelationStatement::Join {
                    input: input_plan,
                    elements,
                    separator,
                    ..
                },
            ) => {
                // (1) Authenticate the operand-binding child against the trusted root (its own
                //     `verify_relation` ends with `verify_columns`, pinning operand columns).
                input.verify_relation(
                    input_plan,
                    layer,
                    expected_root,
                    relation_query_digest(input_plan)?,
                )?;
                // (2) TERMINAL, REVEALED result: the committed columns are the child's VERBATIM
                //     (no committed result column). Pin the carried-through columns + row
                //     count/domain (join is a per-row map, not a filter/aggregate).
                if self.result_len != input.result_len
                    || self.result_log_domain != input.result_log_domain
                    || self.result_columns != input.result_columns
                {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "WOQL Join must carry the child columns through unchanged",
                    ));
                }
                // (3) VERIFIER RE-DERIVATION: recompute the per-row separator-interposed join over
                //     the root-authenticated operands (+ query literals) and require the DELIVERED
                //     result strings to match EXACTLY. A tampered/wrong result column, a tampered
                //     operand, or a non-string operand row all fail closed.
                let expected_objects = expected_join_objects(
                    input,
                    input_plan,
                    elements,
                    separator,
                    layer,
                    expected_root,
                )?;
                if *result_objects != expected_objects {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "WOQL Join result is not the exact verifier-recomputed join",
                    ));
                }
            }
            (
                ProvedQueryStatement::Substring {
                    input,
                    result_objects,
                },
                CompiledRelationStatement::Substring {
                    input: input_plan,
                    string_column,
                    before,
                    length,
                    ..
                },
            ) => {
                // (1) Authenticate the operand-binding child against the trusted root.
                input.verify_relation(
                    input_plan,
                    layer,
                    expected_root,
                    relation_query_digest(input_plan)?,
                )?;
                // (2) TERMINAL, REVEALED result: committed columns are the child's VERBATIM; pin
                //     count/domain (substring is a per-row map — with a fail-closed drop guard).
                if self.result_len != input.result_len
                    || self.result_log_domain != input.result_log_domain
                    || self.result_columns != input.result_columns
                {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "WOQL Substring must carry the child columns through unchanged",
                    ));
                }
                // (3) VERIFIER RE-DERIVATION: char-slice the root-authenticated string column at
                //     the trusted constant offsets and require an EXACT match. A tampered result
                //     column, a tampered operand, an out-of-range row, or a non-UTF-8 row all fail
                //     closed.
                let expected_objects = expected_substring_objects(
                    input,
                    input_plan,
                    *string_column,
                    *before,
                    *length,
                    layer,
                    expected_root,
                )?;
                if *result_objects != expected_objects {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "WOQL Substring result is not the exact verifier-recomputed substring",
                    ));
                }
            }
            (
                ProvedQueryStatement::Pad {
                    input,
                    result_objects,
                },
                CompiledRelationStatement::Pad {
                    input: input_plan,
                    string_column,
                    pad_char,
                    target_len,
                    ..
                },
            ) => {
                // (1) Authenticate the operand-binding child against the trusted root.
                input.verify_relation(
                    input_plan,
                    layer,
                    expected_root,
                    relation_query_digest(input_plan)?,
                )?;
                // (2) TERMINAL, REVEALED result: committed columns are the child's VERBATIM; pin
                //     count/domain (pad is a per-row map — with a fail-closed overflow guard).
                if self.result_len != input.result_len
                    || self.result_log_domain != input.result_log_domain
                    || self.result_columns != input.result_columns
                {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "WOQL Pad must carry the child columns through unchanged",
                    ));
                }
                // (3) VERIFIER RE-DERIVATION: left-pad the root-authenticated string column to the
                //     trusted target length with the trusted pad char and require an EXACT match. A
                //     tampered result, tampered operand, operand-longer-than-target, or non-UTF-8
                //     row all fail closed.
                let expected_objects = expected_pad_objects(
                    input,
                    input_plan,
                    *string_column,
                    pad_char,
                    *target_len,
                    layer,
                    expected_root,
                )?;
                if *result_objects != expected_objects {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "WOQL Pad result is not the exact verifier-recomputed padding",
                    ));
                }
            }
            (
                ProvedQueryStatement::HashKey {
                    input,
                    result_objects,
                },
                CompiledRelationStatement::HashKey {
                    input: input_plan,
                    base,
                    args,
                    ..
                },
            ) => {
                // (1) Authenticate the operand-binding child against the trusted root.
                input.verify_relation(
                    input_plan,
                    layer,
                    expected_root,
                    relation_query_digest(input_plan)?,
                )?;
                // (2) TERMINAL, REVEALED result: committed columns are the child's VERBATIM; pin
                //     count/domain (hash is a per-row map, no drops).
                if self.result_len != input.result_len
                    || self.result_log_domain != input.result_log_domain
                    || self.result_columns != input.result_columns
                {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "WOQL HashKey must carry the child columns through unchanged",
                    ));
                }
                // (3) VERIFIER RE-DERIVATION: run the exact `idgen_hash` recipe over the
                //     root-authenticated base + args and require an EXACT match. A tampered result,
                //     tampered operand, or tampered base/args all fail closed.
                let expected_objects = expected_hashkey_objects(
                    input,
                    input_plan,
                    base,
                    args,
                    layer,
                    expected_root,
                )?;
                if *result_objects != expected_objects {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "WOQL HashKey result is not the exact verifier-recomputed idgen_hash",
                    ));
                }
            }
            (
                ProvedQueryStatement::ExactScalarOrder { input, proof },
                CompiledRelationStatement::ExactScalarOrder {
                    input: input_plan,
                    datatype,
                    direction,
                },
            ) => {
                input.verify_relation(
                    input_plan,
                    layer,
                    expected_root,
                    relation_query_digest(input_plan)?,
                )?;
                let mut transcript = Transcript254::new(b"parture-order-envelope");
                transcript.append_bytes(b"pow/order-envelope/root", &expected_root);
                transcript.append_bytes(b"pow/order-envelope/query", &query_digest);
                verify_exact_scalar_order(
                    layer,
                    &input.relation_commit(None)?,
                    *datatype,
                    *direction,
                    &self.result_commitments,
                    self.result_len,
                    self.result_log_domain,
                    proof,
                    &mut transcript,
                )?;
            }
            (
                ProvedQueryStatement::ExactMultiOrder { input, proof },
                CompiledRelationStatement::ExactMultiOrder { input: input_plan, key_indices, datatypes, directions },
            ) => {
                input.verify_relation(input_plan, layer, expected_root, relation_query_digest(input_plan)?)?;
                let mut transcript = Transcript254::new(b"parture-multi-order-envelope");
                transcript.append_bytes(b"pow/multi-order-envelope/root", &expected_root);
                transcript.append_bytes(b"pow/multi-order-envelope/query", &query_digest);
                verify_exact_multi_order(layer, &input.relation_commit(None)?, &self.result_columns, datatypes, key_indices, directions, &self.result_commitments, self.result_len, self.result_log_domain, proof, &mut transcript)?;
            }
            (
                ProvedQueryStatement::ExactOrderedSlice { input },
                CompiledRelationStatement::ExactOrderedSlice {
                    input: input_plan,
                    offset,
                    limit,
                    ..
                },
            ) => {
                input.verify_relation(
                    input_plan,
                    layer,
                    expected_root,
                    relation_query_digest(input_plan)?,
                )?;
                let start = usize::try_from(*offset)
                    .unwrap_or(usize::MAX)
                    .min(input.result_len);
                let available = input.result_len - start;
                let take = limit
                    .map(|count| usize::try_from(count).unwrap_or(usize::MAX).min(available))
                    .unwrap_or(available);
                let end = start + take;
                let expected_log = take.max(1).next_power_of_two().max(2).trailing_zeros() as usize;
                let expected_columns = input
                    .result_columns
                    .iter()
                    .map(|column| column[start..end].to_vec())
                    .collect::<Vec<_>>();
                if self.result_len != take
                    || self.result_log_domain != expected_log
                    || self.result_columns != expected_columns
                {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "WOQL ordered slice is not the exact contiguous child slice",
                    ));
                }
            }
            (
                ProvedQueryStatement::FloorEval { input },
                CompiledRelationStatement::FloorEval {
                    input: input_plan,
                    expr,
                },
            ) => {
                // (1) Authenticate the operand-binding child against the trusted root (its own
                //     `verify_relation` ends with `verify_columns`, pinning every operand column).
                input.verify_relation(
                    input_plan,
                    layer,
                    expected_root,
                    relation_query_digest(input_plan)?,
                )?;
                // (2) VERIFIER RE-DERIVATION: recompute the floored column in exact rationals over
                //     the root-authenticated operands, narrowed to the i64 generated domain, and
                //     require an EXACT match. Row count/domain are inherited (floor is a per-row
                //     map). A result outside i64 or a non-numeric operand row fails closed.
                let expected_columns =
                    expected_floor_eval_columns(input, input_plan, expr, layer, expected_root)?;
                if self.result_len != input.result_len
                    || self.result_log_domain != input.result_log_domain
                    || self.result_columns != expected_columns
                {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "WOQL floor result is not the exact verifier-recomputed floored column",
                    ));
                }
            }
            (
                ProvedQueryStatement::RationalArith {
                    input,
                    result_objects,
                },
                CompiledRelationStatement::RationalArith {
                    input: input_plan,
                    expr,
                    ..
                },
            ) => {
                // (1) Authenticate the operand-binding child against the trusted root.
                input.verify_relation(
                    input_plan,
                    layer,
                    expected_root,
                    relation_query_digest(input_plan)?,
                )?;
                // (2) TERMINAL, REVEALED result: committed columns are the child's VERBATIM (the
                //     xsd:decimal result is not a committed column); pin count/domain (per-row map).
                if self.result_len != input.result_len
                    || self.result_log_domain != input.result_log_domain
                    || self.result_columns != input.result_columns
                {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "WOQL rational arithmetic must carry the child columns through unchanged",
                    ));
                }
                // (3) VERIFIER RE-DERIVATION: recompute the exact rational per row and re-render the
                //     canonical 20-fractional-digit xsd:decimal lexical, requiring an EXACT match. A
                //     tampered result, tampered operand, zero divisor, non-numeric operand, or carry
                //     overflow all fail closed — no biasable witness (inputs pinned to the root).
                let expected_objects =
                    expected_rational_arith_objects(input, input_plan, expr, layer, expected_root)?;
                if *result_objects != expected_objects {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "WOQL rational-arithmetic result is not the exact verifier-recomputed xsd:decimal",
                    ));
                }
            }
            (
                ProvedQueryStatement::TypecastNumberToString {
                    input,
                    result_objects,
                },
                CompiledRelationStatement::TypecastNumberToString {
                    input: input_plan,
                    source_column,
                    ..
                },
            ) => {
                // (1) Authenticate the source-binding child against the trusted root — its own
                //     `verify_relation` ends with `verify_columns`, pinning the numeric source column.
                input.verify_relation(
                    input_plan,
                    layer,
                    expected_root,
                    relation_query_digest(input_plan)?,
                )?;
                // (2) TERMINAL, REVEALED result: committed columns are the child's VERBATIM (the
                //     xsd:string cast is not a committed column); pin count/domain (per-row map).
                if self.result_len != input.result_len
                    || self.result_log_domain != input.result_log_domain
                    || self.result_columns != input.result_columns
                {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "WOQL number→string Typecast must carry the child columns through unchanged",
                    ));
                }
                // (3) VERIFIER RE-DERIVATION: render the client-facing xsd:string per row from the
                //     root-authenticated numeric source and require an EXACT match. A float/double or
                //     non-numeric operand row fails closed — no biasable witness.
                let expected_objects = expected_cast_to_string_objects(
                    input,
                    input_plan,
                    *source_column,
                    layer,
                    expected_root,
                )?;
                if *result_objects != expected_objects {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "WOQL number→string Typecast result is not the exact verifier-recomputed xsd:string",
                    ));
                }
            }
            (
                ProvedQueryStatement::TypecastStringToNumber {
                    input,
                    result_objects,
                },
                CompiledRelationStatement::TypecastStringToNumber {
                    input: input_plan,
                    source_column,
                    target,
                    ..
                },
            ) => {
                // (1) Authenticate the source-binding child against the trusted root.
                input.verify_relation(
                    input_plan,
                    layer,
                    expected_root,
                    relation_query_digest(input_plan)?,
                )?;
                // (2) TERMINAL, REVEALED result: committed columns are the child's VERBATIM (the
                //     parsed numeric value is not a committed column); pin count/domain (per-row map).
                if self.result_len != input.result_len
                    || self.result_log_domain != input.result_log_domain
                    || self.result_columns != input.result_columns
                {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "WOQL string→number Typecast must carry the child columns through unchanged",
                    ));
                }
                // (3) VERIFIER RE-DERIVATION: re-parse the root-authenticated xsd:string per row with
                //     the exact reference grammar for `target` and require an EXACT match. A
                //     malformed/out-of-grammar string, or a non-string operand row, fails closed.
                let expected_objects = expected_cast_from_string_objects(
                    input,
                    input_plan,
                    *source_column,
                    *target,
                    layer,
                    expected_root,
                )?;
                if *result_objects != expected_objects {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "WOQL string→number Typecast result is not the exact verifier-recomputed numeric value",
                    ));
                }
            }
            (
                ProvedQueryStatement::CalendarInt { input },
                CompiledRelationStatement::CalendarInt {
                    input: input_plan,
                    date_column,
                    outputs,
                },
            ) => {
                // (1) Authenticate the operand-binding child against the trusted root (its own
                //     `verify_relation` ends with `verify_columns`, pinning the xsd:date column).
                input.verify_relation(
                    input_plan,
                    layer,
                    expected_root,
                    relation_query_digest(input_plan)?,
                )?;
                // (2) VERIFIER RE-DERIVATION: recompute the derived integer column(s) over the
                //     root-authenticated date column and require the WHOLE column vector (child
                //     columns ++ derived) to match EXACTLY. Row count/domain are inherited from the
                //     child (a per-row map). A non-date operand row or an out-of-range year fails
                //     closed — no biasable witness.
                let expected_columns = expected_calendar_int_columns(
                    input,
                    input_plan,
                    *date_column,
                    outputs,
                    layer,
                    expected_root,
                )?;
                if self.result_len != input.result_len
                    || self.result_log_domain != input.result_log_domain
                    || self.result_columns != expected_columns
                {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "WOQL calendar-integer result is not the exact verifier-recomputed column(s)",
                    ));
                }
            }
            (
                ProvedQueryStatement::CalendarDate {
                    input,
                    result_objects,
                },
                CompiledRelationStatement::CalendarDate {
                    input: input_plan,
                    op,
                    input_column,
                    ..
                },
            ) => {
                // (1) Authenticate the operand-binding child against the trusted root.
                input.verify_relation(
                    input_plan,
                    layer,
                    expected_root,
                    relation_query_digest(input_plan)?,
                )?;
                // (2) TERMINAL, REVEALED result: committed columns are the child's VERBATIM (the
                //     xsd:date result is not a committed column); pin count/domain (per-row map).
                if self.result_len != input.result_len
                    || self.result_log_domain != input.result_log_domain
                    || self.result_columns != input.result_columns
                {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "WOQL calendar-date op must carry the child columns through unchanged",
                    ));
                }
                // (3) VERIFIER RE-DERIVATION: recompute the per-row xsd:date and require the DELIVERED
                //     result to match EXACTLY. A tampered result, tampered operand, non-zero offset,
                //     out-of-range year, or wrong-typed operand all fail closed.
                let expected_objects = expected_calendar_date_objects(
                    input,
                    input_plan,
                    *op,
                    *input_column,
                    layer,
                    expected_root,
                )?;
                if *result_objects != expected_objects {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "WOQL calendar-date result is not the exact verifier-recomputed xsd:date",
                    ));
                }
            }
            (
                ProvedQueryStatement::TransitiveClosure {
                    edge,
                    seed,
                    rounds,
                    fixpoint,
                },
                CompiledRelationStatement::TransitiveClosure {
                    edge: edge_plan, ..
                },
            ) => {
                // DELEGATE to the audited store verify: reconstruct the store closure envelope and run
                // the store's own recursive verify (the edge BGP, every accumulate-and-dedupe round,
                // and the empty-delta fixpoint). The woql2 layer re-implements NONE of the closure
                // orchestration, so there is no second, drift-prone copy of it to keep sound.
                let envelope = self.build_transitive_closure_envelope(
                    layer,
                    expected_root,
                    query_digest,
                    edge_plan,
                    edge,
                    seed,
                    rounds,
                    fixpoint,
                )?;
                let plan = relation_proof_plan(compiled, query_digest)?;
                envelope.verify(layer, expected_root, &plan)?;
            }
            _ => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "WOQL recursive proof/plan variant mismatch",
                ))
            }
        }
        self.verify_columns(&self.result_columns)
    }

    /// Verify arbitrary caller-delivered Store ID columns against the result commitments
    /// authenticated by this proof. A changed, reordered, inserted, or omitted row fails.
    pub fn verify_columns(&self, columns: &[Vec<ArkFr>]) -> std::io::Result<()> {
        verify_result_columns(
            &self.result_commitments,
            columns,
            self.result_len,
            self.result_log_domain,
        )
    }

    /// Verify the query proof and resolve every proven Store ID to the exact canonical
    /// node IRI or typed literal in the root-authenticated node/value dictionary.
    ///
    /// LOW-LEVEL, INSTANCE-ONLY (mirror of [`CompiledBgp::prove_on_layer`]): a schema-reading query
    /// rejects closed here — resolve it through [`ProvedBgp::verify_over_graphs_and_resolve`], which
    /// ties the schema commitment to the trusted combined `database_root`.
    pub fn verify_and_resolve(
        &self,
        compiled: &CompiledBgp,
        layer: &impl VerifierLayer,
        expected_root: ProofHash,
    ) -> std::io::Result<Vec<Vec<CanonicalObject>>> {
        // SCHEMA-GRAPH GUARD (mirror of `prove_on_layer`): a schema-reading query must NOT resolve
        // against a single caller-supplied root — route it through `verify_over_graphs_and_resolve`.
        if relation_graph(&compiled.relation)? != ProofGraph::Instance {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "query reads the schema graph; verify with `verify_over_graphs` (instance + schema commitments under the trusted database_root)",
            ));
        }
        self.verify_and_resolve_inner(compiled, layer, expected_root)
    }

    /// UNGUARDED core of [`verify_and_resolve`]: verify + resolve against the caller-supplied per-graph
    /// view/root with NO read-graph guard. Reachable only from the guarded public entry (instance
    /// queries) and from [`verify_over_graphs_and_resolve`], which has already validated BOTH
    /// commitments into the trusted combined `database_root` and selected the correct per-graph view —
    /// so the schema commitment is already anchored to the trusted database when this runs.
    fn verify_and_resolve_inner(
        &self,
        compiled: &CompiledBgp,
        layer: &impl VerifierLayer,
        expected_root: ProofHash,
    ) -> std::io::Result<Vec<Vec<CanonicalObject>>> {
        let descriptors = compiled_result_descriptors(compiled)?;
        if descriptors.iter().any(|descriptor| descriptor.nullable) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "nullable Optional results require verify_and_resolve_values",
            ));
        }
        // Verify against the per-graph root directly (the read-graph guard already ran on the entry).
        self.verify_relation(
            &compiled.relation,
            layer,
            expected_root,
            compiled_query_digest(compiled)?,
        )?;
        if compiled.relation.schema.is_empty() {
            Ok(Vec::new())
        } else {
            let namespaces = descriptors
                .into_iter()
                .map(|descriptor| descriptor.namespace)
                .collect::<Vec<_>>();
            resolve_typed_result_objects(layer, expected_root, &self.result_columns, &namespaces)
        }
    }

    /// LOW-LEVEL, INSTANCE-ONLY (mirror of [`CompiledBgp::prove_on_layer`]): a schema-reading query
    /// rejects closed here — no combined-root values entry routes through it, so this is a hard reject.
    pub fn verify_and_resolve_values(
        &self,
        compiled: &CompiledBgp,
        layer: &impl VerifierLayer,
        expected_root: ProofHash,
    ) -> std::io::Result<Vec<Vec<CanonicalResultValue>>> {
        // SCHEMA-GRAPH GUARD (mirror of `prove_on_layer`): schema-reading queries must not resolve
        // against a single caller-supplied root. Instance queries are unaffected (byte-identical).
        if relation_graph(&compiled.relation)? != ProofGraph::Instance {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "query reads the schema graph; verify with `verify_over_graphs` (instance + schema commitments under the trusted database_root)",
            ));
        }
        self.verify(compiled, layer, expected_root)?;
        if compiled.relation.schema.is_empty() {
            Ok(Vec::new())
        } else {
            resolve_result_values(
                layer,
                expected_root,
                &self.result_columns,
                &compiled_result_descriptors(compiled)?,
            )
        }
    }

    /// Verify caller-delivered canonical node IRIs/typed literals against the query
    /// proof, its committed result IDs, and the root-authenticated Store dictionary.
    pub fn verify_objects(
        &self,
        compiled: &CompiledBgp,
        layer: &impl VerifierLayer,
        expected_root: ProofHash,
        objects: &[Vec<CanonicalObject>],
    ) -> std::io::Result<()> {
        self.verify(compiled, layer, expected_root)?;
        if compiled.relation.schema.is_empty() {
            if objects.is_empty() {
                Ok(())
            } else {
                Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "ground result exposes no dictionary-object columns",
                ))
            }
        } else {
            let resolved = resolve_typed_result_objects(
                layer,
                expected_root,
                &self.result_columns,
                &compiled_result_descriptors(compiled)?
                    .into_iter()
                    .map(|descriptor| descriptor.namespace)
                    .collect::<Vec<_>>(),
            )?;
            if resolved == objects {
                Ok(())
            } else {
                Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "delivered result objects do not match typed authenticated Store IDs",
                ))
            }
        }
    }

    /// Verify rows produced by the ordinary WOQL executor against this proof. The
    /// executor supplies Store dictionary IDs in its public variable order. WOQL BGP
    /// results are unordered, so comparison is an exact row-multiset comparison after
    /// both sides resolve through the authenticated dictionary.
    pub fn verify_executed_rows(
        &self,
        compiled: &CompiledBgp,
        layer: &impl VerifierLayer,
        expected_root: ProofHash,
        variables: &[String],
        rows: &[Vec<u64>],
    ) -> std::io::Result<()> {
        if let Some(count_variable) = compiled.count_variable.as_deref() {
            self.verify(compiled, layer, expected_root)?;
            if variables != [count_variable] {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "executed Count variable does not match the compiled count binding",
                ));
            }
            let count = u64::try_from(self.result_len).map_err(|_| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "proven Count result does not fit the execution boundary",
                )
            })?;
            if rows != [vec![count]] {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "ordinary WOQL Count binding does not equal the proven result cardinality",
                ));
            }
            return Ok(());
        }
        if compiled_result_descriptors(compiled)?
            .iter()
            .any(|descriptor| descriptor.nullable)
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "nullable Optional rows require verify_executed_values",
            ));
        }
        if compiled.relation.schema.is_empty() {
            self.verify(compiled, layer, expected_root)?;
            if !variables.is_empty()
                || rows.len() != self.result_len
                || rows.iter().any(|row| !row.is_empty())
            {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "ordinary ground-query rows do not match the proven existence relation",
                ));
            }
            return Ok(());
        }
        let projection = compiled
            .projection
            .clone()
            .unwrap_or_else(|| (0..compiled.relation.schema.len()).collect());
        let expected_variables: Vec<&str> = projection
            .iter()
            .map(|index| compiled.relation.schema[*index].as_str())
            .collect();
        if variables.iter().map(String::as_str).collect::<Vec<_>>() != expected_variables {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "executed result variables do not match the compiled projection",
            ));
        }
        if rows.iter().any(|row| row.len() != projection.len()) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "executed result row width does not match the compiled projection",
            ));
        }

        let proven_objects = self.verify_and_resolve(compiled, layer, expected_root)?;
        let mut expected_rows: Vec<Vec<CanonicalObject>> = (0..self.result_len)
            .map(|row| {
                projection
                    .iter()
                    .map(|column| proven_objects[*column][row].clone())
                    .collect()
            })
            .collect();

        let mut delivered_columns = vec![Vec::with_capacity(rows.len()); projection.len()];
        for row in rows {
            for (column, id) in row.iter().enumerate() {
                delivered_columns[column].push(ArkFr::from_u64(*id));
            }
        }
        let namespaces: Vec<_> = compiled_result_descriptors(compiled)?
            .into_iter()
            .map(|descriptor| descriptor.namespace)
            .collect();
        let projected_namespaces: Vec<_> =
            projection.iter().map(|index| namespaces[*index]).collect();
        let delivered_objects = resolve_typed_result_objects(
            layer,
            expected_root,
            &delivered_columns,
            &projected_namespaces,
        )?;
        let mut delivered_rows: Vec<Vec<CanonicalObject>> = (0..rows.len())
            .map(|row| {
                (0..projection.len())
                    .map(|column| delivered_objects[column][row].clone())
                    .collect()
            })
            .collect();

        expected_rows.sort();
        delivered_rows.sort();
        if expected_rows != delivered_rows {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "ordinary WOQL result rows do not match the proven result multiset",
            ));
        }
        Ok(())
    }

    pub fn verify_executed_values(
        &self,
        compiled: &CompiledBgp,
        layer: &impl VerifierLayer,
        expected_root: ProofHash,
        variables: &[String],
        rows: &[Vec<ExecutedResultValue>],
    ) -> std::io::Result<()> {
        if compiled.count_variable.is_some() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Count bindings use the scalar execution boundary, not Store result values",
            ));
        }
        let projection = compiled
            .projection
            .clone()
            .unwrap_or_else(|| (0..compiled.relation.schema.len()).collect());
        let expected_variables: Vec<&str> = projection
            .iter()
            .map(|index| compiled.relation.schema[*index].as_str())
            .collect();
        if variables.iter().map(String::as_str).collect::<Vec<_>>() != expected_variables
            || rows.iter().any(|row| row.len() != projection.len())
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "executed nullable result shape does not match compiled projection",
            ));
        }
        let proven = self.verify_and_resolve_values(compiled, layer, expected_root)?;
        let mut expected_rows: Vec<Vec<CanonicalResultValue>> = (0..self.result_len)
            .map(|row| {
                projection
                    .iter()
                    .map(|column| proven[*column][row].clone())
                    .collect()
            })
            .collect();
        let descriptors = compiled_result_descriptors(compiled)?;
        let projected_descriptors: Vec<_> =
            projection.iter().map(|index| descriptors[*index]).collect();
        let mut delivered_columns = vec![Vec::with_capacity(rows.len()); projection.len()];
        for row in rows {
            for (column, value) in row.iter().enumerate() {
                let descriptor = projected_descriptors[column];
                delivered_columns[column].push(match (value, descriptor.namespace) {
                    (
                        ExecutedResultValue::GeneratedDecimalU64(value),
                        ResultIdNamespace::GeneratedDecimalU64,
                    ) => ArkFr::from_u64(*value),
                    (
                        ExecutedResultValue::GeneratedDecimalI64(value),
                        ResultIdNamespace::GeneratedDecimalI64,
                    ) => generated_decimal_i64_field(*value),
                    (
                        ExecutedResultValue::Id(id),
                        ResultIdNamespace::NodeValue | ResultIdNamespace::Predicate,
                    ) if *id != 0 => {
                        ArkFr::from_u64(*id)
                    }
                    (
                        ExecutedResultValue::Null,
                        ResultIdNamespace::GeneratedDecimalU64
                        | ResultIdNamespace::GeneratedDecimalI64,
                    ) => {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "generated decimal zero is not a nullable result",
                        ))
                    }
                    (ExecutedResultValue::Null, _) => ArkFr::zero(),
                    (ExecutedResultValue::Id(_), _)
                    | (ExecutedResultValue::GeneratedDecimalU64(_), _)
                    | (ExecutedResultValue::GeneratedDecimalI64(_), _) => {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "executed generated scalar/Store-ID kind does not match compiled descriptor",
                        ))
                    }
                });
            }
        }
        let delivered = resolve_result_values(
            layer,
            expected_root,
            &delivered_columns,
            &projected_descriptors,
        )?;
        let mut delivered_rows: Vec<Vec<CanonicalResultValue>> = (0..rows.len())
            .map(|row| {
                (0..projection.len())
                    .map(|column| delivered[column][row].clone())
                    .collect()
            })
            .collect();
        expected_rows.sort();
        delivered_rows.sort();
        if expected_rows != delivered_rows {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "ordinary WOQL nullable rows do not match the proven result multiset",
            ));
        }
        Ok(())
    }

    /// Encode a self-contained result envelope only after checking the proof, result IDs,
    /// and canonical lexical values against the caller-selected root.
    pub fn encode_envelope(
        &self,
        compiled: &CompiledBgp,
        layer: &LayerCommitment,
        expected_root: ProofHash,
    ) -> std::io::Result<Vec<u8>> {
        let digest = compiled_query_digest(compiled)?;
        // `verify_relation` recursively checks the complete proof tree.  Keep that single
        // public trust boundary, then only materialize the already-checked tree below;
        // verifying every child again (and the finished envelope once more) made checked
        // encoding quadratic in the depth/number of recursive operators.
        self.verify_relation(&compiled.relation, layer, expected_root, digest)?;
        let envelope = self.recursive_envelope_after_verification(
            &compiled.relation,
            layer,
            expected_root,
            digest,
        )?;
        envelope.encode()
    }

    /// Materialize an envelope after the caller has verified the complete root relation.
    /// This must remain private: decoded/external envelopes still go through the full
    /// recursive `RecursiveRelationEnvelope::verify` path.
    /// Reconstruct the store [`RecursiveRelationEnvelope`] for a bounded transitive closure from the
    /// carried proof pieces, so the woql2 layer can DELEGATE to the audited store verify (it adds no
    /// closure-specific verification of its own). The edge sub-relation reuses the closure's own query
    /// digest as its identity tag (see `closure_edge_plan`), so no closure-internal information is
    /// needed to rebuild it. Generic over the trusted layer so both the verify path and the
    /// encode-after-verify path can call it.
    #[allow(clippy::too_many_arguments)]
    fn build_transitive_closure_envelope(
        &self,
        layer: &impl VerifierLayer,
        expected_root: ProofHash,
        query_digest: ProofHash,
        edge_plan: &LayerBgpPlan,
        edge: &ProvedBgp,
        seed: &TransitiveClosureSeed,
        rounds: &[TransitiveClosureRound],
        fixpoint: &TransitiveClosureFixpoint,
    ) -> std::io::Result<RecursiveRelationEnvelope> {
        let invalid = |message: &str| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, message.to_string())
        };
        let (edge_proof, edge_predicate_ids) = match &edge.statement {
            ProvedQueryStatement::Single {
                proof,
                predicate_ids,
            } => (proof, predicate_ids),
            _ => return Err(invalid("transitive closure edge is not a single BGP relation")),
        };
        // Rebuild the edge Bgp leaf envelope exactly as the `Single` reconstruction does.
        let edge_descriptors =
            terminus_store::proof::query::result_column_descriptors(edge_plan)?;
        let edge_result_objects = resolve_typed_result_objects(
            layer,
            expected_root,
            &edge.result_columns,
            &result_id_namespaces(edge_plan)?,
        )?;
        let edge_result_values = resolve_result_values(
            layer,
            expected_root,
            &edge.result_columns,
            &edge_descriptors,
        )?;
        let edge_envelope = RecursiveRelationEnvelope {
            layer_root: expected_root,
            query_digest,
            proof: RecursiveRelationProof::Bgp(Box::new(LayerBgpEnvelope {
                layer_root: expected_root,
                plan_digest: plan_digest(edge_plan)?,
                query_digest,
                proof: *edge_proof.clone(),
                result_commitments: edge.result_commitments.clone(),
                result_columns: edge.result_columns.clone(),
                result_log_domain: edge.result_log_domain,
                result_len: edge.result_len,
                predicate_ids: edge_predicate_ids.clone(),
                result_objects: edge_result_objects,
            })),
            result_commitments: edge.result_commitments.clone(),
            result_columns: edge.result_columns.clone(),
            result_log_domain: edge.result_log_domain,
            result_len: edge.result_len,
            ground_result: false,
            result_descriptors: edge_descriptors,
            result_values: edge_result_values,
        };
        // The delivered reachable set is one nonnullable node column.
        let descriptors = vec![ResultColumnDescriptor {
            namespace: ResultIdNamespace::NodeValue,
            nullable: false,
        }];
        let result_values = resolve_result_values(
            layer,
            expected_root,
            &self.result_columns,
            &descriptors,
        )?;
        Ok(RecursiveRelationEnvelope {
            layer_root: expected_root,
            query_digest,
            proof: RecursiveRelationProof::TransitiveClosure {
                edge: Box::new(edge_envelope),
                seed: seed.clone(),
                rounds: rounds.to_vec(),
                fixpoint: fixpoint.clone(),
            },
            result_commitments: self.result_commitments.clone(),
            result_columns: self.result_columns.clone(),
            result_log_domain: self.result_log_domain,
            result_len: self.result_len,
            ground_result: false,
            result_descriptors: descriptors,
            result_values,
        })
    }

    fn recursive_envelope_after_verification(
        &self,
        compiled: &CompiledRelation,
        layer: &LayerCommitment,
        expected_root: ProofHash,
        query_digest: ProofHash,
    ) -> std::io::Result<RecursiveRelationEnvelope> {
        if let CompiledRelationStatement::When { expanded } = &compiled.statement {
            return self.recursive_envelope_after_verification(
                expanded, layer, expected_root, query_digest,
            );
        }
        // The transitive closure reconstructs its whole store envelope in one shot (it carries the
        // store proof pieces directly), so it early-returns rather than threading a `proof` through the
        // common wrapper below.
        if let (
            ProvedQueryStatement::TransitiveClosure {
                edge,
                seed,
                rounds,
                fixpoint,
            },
            CompiledRelationStatement::TransitiveClosure {
                edge: edge_plan, ..
            },
        ) = (&self.statement, &compiled.statement)
        {
            return self.build_transitive_closure_envelope(
                layer,
                expected_root,
                query_digest,
                edge_plan,
                edge,
                seed,
                rounds,
                fixpoint,
            );
        }
        let proof = match (&self.statement, &compiled.statement) {
            (
                ProvedQueryStatement::LayerTripleCount,
                CompiledRelationStatement::LayerTripleCount,
            ) => RecursiveRelationProof::LayerTripleCount,
            (
                ProvedQueryStatement::GeneratedU64Filter { input },
                CompiledRelationStatement::GeneratedU64Filter {
                    input: input_plan, ..
                },
            ) => RecursiveRelationProof::GeneratedU64Filter {
                input: Box::new(input.recursive_envelope_after_verification(
                    input_plan,
                    layer,
                    expected_root,
                    relation_query_digest(input_plan)?,
                )?),
            },
            (
                ProvedQueryStatement::GeneratedI64Filter { input },
                CompiledRelationStatement::GeneratedI64Filter {
                    input: input_plan, ..
                },
            ) => RecursiveRelationProof::GeneratedI64Filter {
                input: Box::new(input.recursive_envelope_after_verification(
                    input_plan,
                    layer,
                    expected_root,
                    relation_query_digest(input_plan)?,
                )?),
            },
            (
                ProvedQueryStatement::Single {
                    proof,
                    predicate_ids,
                },
                CompiledRelationStatement::Bgp(plan),
            ) => {
                let result_objects = if compiled.schema.is_empty() {
                    Vec::new()
                } else {
                    resolve_typed_result_objects(
                        layer,
                        expected_root,
                        &self.result_columns,
                        &result_id_namespaces(plan)?,
                    )?
                };
                RecursiveRelationProof::Bgp(Box::new(LayerBgpEnvelope {
                    layer_root: expected_root,
                    plan_digest: plan_digest(plan)?,
                    query_digest,
                    proof: *proof.clone(),
                    result_commitments: self.result_commitments.clone(),
                    result_columns: self.result_columns.clone(),
                    result_log_domain: self.result_log_domain,
                    result_len: self.result_len,
                    predicate_ids: predicate_ids.clone(),
                    result_objects,
                }))
            }
            (
                ProvedQueryStatement::BagUnion { branches, proof },
                CompiledRelationStatement::BagUnion {
                    branches: branch_plans,
                    ..
                },
            ) => RecursiveRelationProof::BagUnion {
                branches: branches
                    .iter()
                    .zip(branch_plans)
                    .map(|(branch, plan)| {
                        branch.recursive_envelope_after_verification(
                            plan,
                            layer,
                            expected_root,
                            relation_query_digest(plan)?,
                        )
                    })
                    .collect::<std::io::Result<_>>()?,
                proof: proof.clone(),
            },
            (
                ProvedQueryStatement::BagProjection { input },
                CompiledRelationStatement::BagProjection {
                    input: input_plan, ..
                },
            ) => RecursiveRelationProof::BagProjection {
                input: Box::new(input.recursive_envelope_after_verification(
                    input_plan,
                    layer,
                    expected_root,
                    relation_query_digest(input_plan)?,
                )?),
            },
            (
                ProvedQueryStatement::Distinct { input, proof },
                CompiledRelationStatement::Distinct {
                    input: input_plan, ..
                },
            ) => RecursiveRelationProof::Distinct {
                input: Box::new(input.recursive_envelope_after_verification(
                    input_plan,
                    layer,
                    expected_root,
                    relation_query_digest(input_plan)?,
                )?),
                proof: proof.clone(),
            },
            (
                ProvedQueryStatement::Equality { input, proof },
                CompiledRelationStatement::Equality {
                    input: input_plan, ..
                },
            ) => RecursiveRelationProof::Equality {
                input: Box::new(input.recursive_envelope_after_verification(
                    input_plan,
                    layer,
                    expected_root,
                    relation_query_digest(input_plan)?,
                )?),
                proof: proof.clone(),
            },
            (
                ProvedQueryStatement::TupleDisequality { input, proof },
                CompiledRelationStatement::TupleDisequality {
                    input: input_plan, ..
                },
            ) => RecursiveRelationProof::TupleDisequality {
                input: Box::new(input.recursive_envelope_after_verification(
                    input_plan,
                    layer,
                    expected_root,
                    relation_query_digest(input_plan)?,
                )?),
                proof: proof.clone(),
            },
            (
                ProvedQueryStatement::GroupExtrema { input },
                CompiledRelationStatement::GroupExtrema {
                    input: input_plan, ..
                },
            ) => RecursiveRelationProof::GroupExtrema {
                input: Box::new(input.recursive_envelope_after_verification(
                    input_plan,
                    layer,
                    expected_root,
                    relation_query_digest(input_plan)?,
                )?),
            },
            (
                ProvedQueryStatement::GroupSum { input },
                CompiledRelationStatement::GroupSum {
                    input: input_plan, ..
                },
            ) => RecursiveRelationProof::GroupSum {
                input: Box::new(input.recursive_envelope_after_verification(
                    input_plan,
                    layer,
                    expected_root,
                    relation_query_digest(input_plan)?,
                )?),
            },
            (
                ProvedQueryStatement::GroupCount { input },
                CompiledRelationStatement::GroupCount {
                    input: input_plan, ..
                },
            ) => RecursiveRelationProof::GroupCount {
                input: Box::new(input.recursive_envelope_after_verification(
                    input_plan,
                    layer,
                    expected_root,
                    relation_query_digest(input_plan)?,
                )?),
            },
            (
                ProvedQueryStatement::CollectionCount { input },
                CompiledRelationStatement::CollectionCount { input: input_plan },
            ) => RecursiveRelationProof::CollectionCount {
                input: Box::new(input.recursive_envelope_after_verification(
                    input_plan,
                    layer,
                    expected_root,
                    relation_query_digest(input_plan)?,
                )?),
            },
            (
                ProvedQueryStatement::CollectionSum { input },
                CompiledRelationStatement::CollectionSum {
                    input: input_plan, ..
                },
            ) => RecursiveRelationProof::CollectionSum {
                input: Box::new(input.recursive_envelope_after_verification(
                    input_plan,
                    layer,
                    expected_root,
                    relation_query_digest(input_plan)?,
                )?),
            },
            (
                ProvedQueryStatement::CollectionExtrema { input },
                CompiledRelationStatement::CollectionExtrema {
                    input: input_plan, ..
                },
            ) => RecursiveRelationProof::CollectionExtrema {
                input: Box::new(input.recursive_envelope_after_verification(
                    input_plan,
                    layer,
                    expected_root,
                    relation_query_digest(input_plan)?,
                )?),
            },
            (
                ProvedQueryStatement::Eval { .. },
                CompiledRelationStatement::Eval { .. },
            ) => {
                // V1: Eval proofs are verify-only. There is no Store `RecursiveRelationProof`
                // for the appended derived column yet, so re-encoding an Eval proof into a
                // portable Store envelope is deferred (a follow-up would add a Store plan
                // kind). Reject closed instead of emitting a truncated envelope.
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "WOQL Eval proofs are verify-only in v1 and cannot be re-encoded into a Store recursive envelope",
                ));
            }
            (
                ProvedQueryStatement::Typecast { .. },
                CompiledRelationStatement::Typecast { .. },
            ) => {
                // V1: like Eval, Typecast proofs are verify-only — no Store `RecursiveRelationProof`
                // exists for the appended cast column. Reject closed rather than emit a truncated
                // envelope.
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "WOQL Typecast proofs are verify-only in v1 and cannot be re-encoded into a Store recursive envelope",
                ));
            }
            (
                ProvedQueryStatement::Concat { .. },
                CompiledRelationStatement::Concat { .. },
            ) => {
                // V1: like Eval/Typecast, Concat proofs are verify-only. The terminal result is
                // revealed and re-derived at verify, not carried as a Store `RecursiveRelationProof`
                // column, so a portable Store envelope cannot represent it. Reject closed.
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "WOQL Concat proofs are verify-only in v1 and cannot be re-encoded into a Store recursive envelope",
                ));
            }
            (
                ProvedQueryStatement::Join { .. },
                CompiledRelationStatement::Join { .. },
            ) => {
                // Verify-only in v1, exactly like Concat (terminal revealed result). Reject closed.
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "WOQL Join proofs are verify-only in v1 and cannot be re-encoded into a Store recursive envelope",
                ));
            }
            (
                ProvedQueryStatement::Substring { .. },
                CompiledRelationStatement::Substring { .. },
            ) => {
                // Verify-only in v1, exactly like Concat (terminal revealed result). Reject closed.
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "WOQL Substring proofs are verify-only in v1 and cannot be re-encoded into a Store recursive envelope",
                ));
            }
            (
                ProvedQueryStatement::Pad { .. },
                CompiledRelationStatement::Pad { .. },
            ) => {
                // Verify-only in v1, exactly like Concat (terminal revealed result). Reject closed.
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "WOQL Pad proofs are verify-only in v1 and cannot be re-encoded into a Store recursive envelope",
                ));
            }
            (
                ProvedQueryStatement::HashKey { .. },
                CompiledRelationStatement::HashKey { .. },
            ) => {
                // Verify-only in v1, exactly like Concat (terminal revealed result). Reject closed.
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "WOQL HashKey proofs are verify-only in v1 and cannot be re-encoded into a Store recursive envelope",
                ));
            }
            (
                ProvedQueryStatement::FloorEval { .. },
                CompiledRelationStatement::FloorEval { .. },
            ) => {
                // Verify-only in v1, exactly like Eval (appended derived column, no Store plan kind).
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "WOQL floor proofs are verify-only in v1 and cannot be re-encoded into a Store recursive envelope",
                ));
            }
            (
                ProvedQueryStatement::RationalArith { .. },
                CompiledRelationStatement::RationalArith { .. },
            ) => {
                // Verify-only in v1, exactly like Concat (terminal revealed result). Reject closed.
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "WOQL rational-arithmetic proofs are verify-only in v1 and cannot be re-encoded into a Store recursive envelope",
                ));
            }
            (
                ProvedQueryStatement::CalendarInt { .. },
                CompiledRelationStatement::CalendarInt { .. },
            ) => {
                // Verify-only in v1, exactly like Eval (appended derived column(s), no Store plan kind).
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "WOQL calendar-integer proofs are verify-only in v1 and cannot be re-encoded into a Store recursive envelope",
                ));
            }
            (
                ProvedQueryStatement::CalendarDate { .. },
                CompiledRelationStatement::CalendarDate { .. },
            ) => {
                // Verify-only in v1, exactly like Concat (terminal revealed result). Reject closed.
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "WOQL calendar-date proofs are verify-only in v1 and cannot be re-encoded into a Store recursive envelope",
                ));
            }
            (
                ProvedQueryStatement::TypecastNumberToString { .. },
                CompiledRelationStatement::TypecastNumberToString { .. },
            ) => {
                // Verify-only in v1, exactly like Concat (terminal revealed result). Reject closed.
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "WOQL number→string Typecast proofs are verify-only in v1 and cannot be re-encoded into a Store recursive envelope",
                ));
            }
            (
                ProvedQueryStatement::TypecastStringToNumber { .. },
                CompiledRelationStatement::TypecastStringToNumber { .. },
            ) => {
                // Verify-only in v1, exactly like Concat (terminal revealed result). Reject closed.
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "WOQL string→number Typecast proofs are verify-only in v1 and cannot be re-encoded into a Store recursive envelope",
                ));
            }
            (
                ProvedQueryStatement::ExactScalarOrder { input, proof },
                CompiledRelationStatement::ExactScalarOrder {
                    input: input_plan, ..
                },
            ) => RecursiveRelationProof::ExactScalarOrder {
                input: Box::new(input.recursive_envelope_after_verification(
                    input_plan,
                    layer,
                    expected_root,
                    relation_query_digest(input_plan)?,
                )?),
                proof: proof.clone(),
            },
            (
                ProvedQueryStatement::ExactMultiOrder { input, proof },
                CompiledRelationStatement::ExactMultiOrder { input: input_plan, .. },
            ) => RecursiveRelationProof::ExactMultiOrder {
                input: Box::new(input.recursive_envelope_after_verification(input_plan, layer, expected_root, relation_query_digest(input_plan)?)?),
                proof: proof.clone(),
            },
            (
                ProvedQueryStatement::ExactOrderedSlice { input },
                CompiledRelationStatement::ExactOrderedSlice {
                    input: input_plan, ..
                },
            ) => RecursiveRelationProof::ExactOrderedSlice {
                input: Box::new(input.recursive_envelope_after_verification(
                    input_plan,
                    layer,
                    expected_root,
                    relation_query_digest(input_plan)?,
                )?),
            },
            (
                ProvedQueryStatement::LeftAnti { left, right, proof },
                CompiledRelationStatement::LeftAnti {
                    left: left_plan,
                    right: right_plan,
                    ..
                },
            ) => RecursiveRelationProof::LeftAnti {
                left: Box::new(left.recursive_envelope_after_verification(
                    left_plan,
                    layer,
                    expected_root,
                    relation_query_digest(left_plan)?,
                )?),
                right: Box::new(right.recursive_envelope_after_verification(
                    right_plan,
                    layer,
                    expected_root,
                    relation_query_digest(right_plan)?,
                )?),
                proof: proof.clone(),
            },
            (
                ProvedQueryStatement::InnerJoin { left, right, proof },
                CompiledRelationStatement::InnerJoin { left: left_plan, right: right_plan, .. },
            ) => RecursiveRelationProof::InnerJoin {
                left: Box::new(left.recursive_envelope_after_verification(
                    left_plan, layer, expected_root, relation_query_digest(left_plan)?,
                )?),
                right: Box::new(right.recursive_envelope_after_verification(
                    right_plan, layer, expected_root, relation_query_digest(right_plan)?,
                )?),
                proof: proof.clone(),
            },
            (
                ProvedQueryStatement::LeftOuter { left, right, proof },
                CompiledRelationStatement::LeftOuter {
                    left: left_plan,
                    right: right_plan,
                    ..
                },
            ) => RecursiveRelationProof::LeftOuter {
                left: Box::new(left.recursive_envelope_after_verification(
                    left_plan,
                    layer,
                    expected_root,
                    relation_query_digest(left_plan)?,
                )?),
                right: Box::new(right.recursive_envelope_after_verification(
                    right_plan,
                    layer,
                    expected_root,
                    relation_query_digest(right_plan)?,
                )?),
                proof: proof.clone(),
            },
            _ => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "WOQL recursive envelope proof/plan mismatch",
                ))
            }
        };
        let result_descriptors = relation_result_descriptors(compiled)?;
        let ground_result = compiled.schema.is_empty();
        let result_values = if ground_result {
            Vec::new()
        } else {
            resolve_result_values(
                layer,
                expected_root,
                &self.result_columns,
                &result_descriptors,
            )?
        };
        Ok(RecursiveRelationEnvelope {
            layer_root: expected_root,
            query_digest,
            proof,
            result_commitments: self.result_commitments.clone(),
            result_columns: self.result_columns.clone(),
            result_log_domain: self.result_log_domain,
            result_len: self.result_len,
            ground_result,
            result_descriptors,
            result_values,
        })
    }

    /// Commitments selected by an outer WOQL `Select`, or every result commitment when
    /// the query has no projection.
    pub fn projected_commitments<'a>(
        &'a self,
        compiled: &'a CompiledBgp,
    ) -> impl Iterator<Item = &'a ArkGT> + 'a {
        let projection = compiled
            .projection
            .clone()
            .unwrap_or_else(|| (0..self.result_commitments.len()).collect());
        projection
            .into_iter()
            .map(|index| &self.result_commitments[index])
    }

    /// Result ID columns selected by an outer WOQL `Select`, or every column when there
    /// is no projection. `Count` exposes no underlying columns.
    pub fn projected_columns<'a>(
        &'a self,
        compiled: &'a CompiledBgp,
    ) -> impl Iterator<Item = &'a [ArkFr]> + 'a {
        let projection = compiled
            .projection
            .clone()
            .unwrap_or_else(|| (0..self.result_columns.len()).collect());
        projection
            .into_iter()
            .map(|index| self.result_columns[index].as_slice())
    }

    /// Return the variable/value pair bound by an outer WOQL `Count`, if present.
    pub fn count_binding<'a>(&self, compiled: &'a CompiledBgp) -> Option<(&'a str, usize)> {
        compiled
            .count_variable
            .as_deref()
            .map(|variable| (variable, self.result_len))
    }
}

/// Decode and verify a portable envelope against an independently compiled query and
/// caller-selected trusted root. Parsing alone never confers verification status.
pub fn decode_and_verify_envelope(
    bytes: &[u8],
    compiled: &CompiledBgp,
    layer: &impl VerifierLayer,
    expected_root: ProofHash,
) -> std::io::Result<VerifiedBgpEnvelope> {
    let envelope = RecursiveRelationEnvelope::decode(bytes)?;
    let digest = compiled_query_digest(compiled)?;
    let plan = relation_proof_plan(&compiled.relation, digest)?;
    envelope.verify(layer, expected_root, &plan)?;
    let nullable = envelope
        .result_descriptors
        .iter()
        .any(|descriptor| descriptor.nullable);
    let generated = envelope.result_descriptors.iter().any(|descriptor| {
        matches!(
            descriptor.namespace,
            ResultIdNamespace::GeneratedDecimalU64
                | ResultIdNamespace::GeneratedDecimalI64
        )
    });
    let result_objects = if nullable || generated {
        Vec::new()
    } else {
        envelope
            .result_values
            .iter()
            .map(|column| {
                column
                    .iter()
                    .map(|value| match value {
                        CanonicalResultValue::Object(object) => Ok(object.clone()),
                        CanonicalResultValue::Null => Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "null value in a nonnullable recursive result",
                        )),
                        CanonicalResultValue::GeneratedDecimalU64(_) => Err(
                            std::io::Error::new(
                                std::io::ErrorKind::InvalidData,
                                "generated scalar is not a Store dictionary object",
                            ),
                        ),
                        CanonicalResultValue::GeneratedDecimalI64(_) => Err(
                            std::io::Error::new(
                                std::io::ErrorKind::InvalidData,
                                "generated scalar is not a Store dictionary object",
                            ),
                        ),
                    })
                    .collect()
            })
            .collect::<std::io::Result<_>>()?
    };
    let result_values = (nullable || generated).then(|| envelope.result_values.clone());
    Ok(VerifiedBgpEnvelope {
        proved: proved_from_recursive(envelope),
        result_objects,
        result_values,
    })
}

/// Decode both portable inputs and verify without loading native Store witness tables.
/// The trusted root remains a separate caller-selected input; the artifact cannot choose
/// its own trust anchor merely by decoding successfully.
pub fn decode_verifier_commitment_and_verify_envelope(
    envelope_bytes: &[u8],
    compiled: &CompiledBgp,
    verifier_commitment_bytes: &[u8],
    expected_root: ProofHash,
) -> std::io::Result<VerifiedBgpEnvelope> {
    let commitment = VerifierCommitment::decode(verifier_commitment_bytes)?;
    decode_and_verify_envelope(envelope_bytes, compiled, &commitment, expected_root)
}

/// Prove a top-level WOQL `Eval`: prove the operand-binding child through the Store, then
/// APPEND the derived integer column computed by the SAME recompute the verifier re-runs.
///
/// The child's own columns and commitments are reused verbatim (they are already
/// root-authenticated by the Store proof); only the one derived column is committed here,
/// with the child's context/domain, so nothing about the existing committed columns changes.
/// Because the appended column is produced by `expected_eval_columns` — the exact function
/// `verify_relation` re-runs — a prover cannot deliver a different column and still verify.
fn prove_eval_relation(
    context: &StoreProverContext<'_>,
    layer: &LayerCommitment,
    input: &CompiledRelation,
    expr: &EvalExpr,
) -> std::io::Result<ProvedBgp> {
    let child_digest = relation_query_digest(input)?;
    let child_plan = relation_proof_plan(input, child_digest)?;
    let child_envelope = RecursiveRelationEnvelope::prove_with_context(context, layer, &child_plan)?;
    let child = proved_from_recursive(child_envelope);

    // Recompute (child columns ++ derived column) against THIS layer's own root; identical to
    // the verifier's re-derivation. Take the last (derived) column and commit it.
    let expected =
        expected_eval_columns(&child, input, expr, layer, layer.state.commitment_root)?;
    let derived = expected
        .last()
        .cloned()
        .expect("expected_eval_columns always appends the derived column");

    let result_len = child.result_len;
    let result_log_domain = child.result_log_domain;
    let domain = 1usize << result_log_domain;
    let mut padded = derived.clone();
    padded.resize(domain, ArkFr::zero());
    let derived_commitment = context.commit_column(&padded)?;

    let mut result_columns = child.result_columns.clone();
    result_columns.push(derived);
    let mut result_commitments = child.result_commitments.clone();
    result_commitments.push(derived_commitment);

    Ok(ProvedBgp {
        statement: ProvedQueryStatement::Eval {
            input: Box::new(child),
        },
        result_commitments,
        result_columns,
        result_log_domain,
        result_len,
    })
}

/// Prove a top-level WOQL `floor(x)`: prove the operand-binding child through the Store, then APPEND
/// the derived integer column computed by the SAME [`expected_floor_eval_columns`] the verifier
/// re-runs. Structurally identical to [`prove_eval_relation`] (only the recompute differs — exact
/// rationals then a floor), so a prover cannot deliver a different column and still verify, and
/// cannot prove a floor whose i64 narrowing overflows or whose operand row is non-numeric.
fn prove_floor_eval_relation(
    context: &StoreProverContext<'_>,
    layer: &LayerCommitment,
    input: &CompiledRelation,
    expr: &DecimalExpr,
) -> std::io::Result<ProvedBgp> {
    let child_digest = relation_query_digest(input)?;
    let child_plan = relation_proof_plan(input, child_digest)?;
    let child_envelope = RecursiveRelationEnvelope::prove_with_context(context, layer, &child_plan)?;
    let child = proved_from_recursive(child_envelope);

    let expected =
        expected_floor_eval_columns(&child, input, expr, layer, layer.state.commitment_root)?;
    let derived = expected
        .last()
        .cloned()
        .expect("expected_floor_eval_columns always appends the derived column");

    let result_len = child.result_len;
    let result_log_domain = child.result_log_domain;
    let domain = 1usize << result_log_domain;
    let mut padded = derived.clone();
    padded.resize(domain, ArkFr::zero());
    let derived_commitment = context.commit_column(&padded)?;

    let mut result_columns = child.result_columns.clone();
    result_columns.push(derived);
    let mut result_commitments = child.result_commitments.clone();
    result_commitments.push(derived_commitment);

    Ok(ProvedBgp {
        statement: ProvedQueryStatement::FloorEval {
            input: Box::new(child),
        },
        result_commitments,
        result_columns,
        result_log_domain,
        result_len,
    })
}

/// Prove a top-level WOQL exact-rational arithmetic (`Divide`/decimal-operand arithmetic): prove the
/// operand-binding child through the Store, then DELIVER the per-row xsd:decimal result computed by
/// the SAME [`expected_rational_arith_objects`] the verifier re-runs. Like `Concat`, NO derived
/// column is committed — the result is TERMINAL/revealed; the child's columns/commitments are reused
/// verbatim. A zero divisor, a non-numeric operand row, or a carry-overflow render FAILS CLOSED here
/// (exactly as verify does), so a prover cannot produce a proof for a row the reference would reject.
fn prove_rational_arith_relation(
    context: &StoreProverContext<'_>,
    layer: &LayerCommitment,
    input: &CompiledRelation,
    expr: &DecimalExpr,
) -> std::io::Result<ProvedBgp> {
    let child_digest = relation_query_digest(input)?;
    let child_plan = relation_proof_plan(input, child_digest)?;
    let child_envelope = RecursiveRelationEnvelope::prove_with_context(context, layer, &child_plan)?;
    let child = proved_from_recursive(child_envelope);

    let result_objects =
        expected_rational_arith_objects(&child, input, expr, layer, layer.state.commitment_root)?;

    let result_len = child.result_len;
    let result_log_domain = child.result_log_domain;
    let result_columns = child.result_columns.clone();
    let result_commitments = child.result_commitments.clone();

    Ok(ProvedBgp {
        statement: ProvedQueryStatement::RationalArith {
            input: Box::new(child),
            result_objects,
        },
        result_commitments,
        result_columns,
        result_log_domain,
        result_len,
    })
}

/// Prove a top-level WOQL NUMBER→STRING `Typecast`: prove the source-binding child through the Store,
/// then DELIVER the per-row `xsd:string` cast produced by the SAME [`expected_cast_to_string_objects`]
/// the verifier re-runs. Like `Concat`/`RationalArith`, NO derived column is committed — the child's
/// columns/commitments are reused verbatim and the synthesized strings are re-checked at verify. A
/// float/double or non-numeric operand row FAILS CLOSED here (exactly as verify does).
fn prove_typecast_to_string_relation(
    context: &StoreProverContext<'_>,
    layer: &LayerCommitment,
    input: &CompiledRelation,
    source_column: usize,
) -> std::io::Result<ProvedBgp> {
    let child_digest = relation_query_digest(input)?;
    let child_plan = relation_proof_plan(input, child_digest)?;
    let child_envelope = RecursiveRelationEnvelope::prove_with_context(context, layer, &child_plan)?;
    let child = proved_from_recursive(child_envelope);

    let result_objects = expected_cast_to_string_objects(
        &child,
        input,
        source_column,
        layer,
        layer.state.commitment_root,
    )?;

    let result_len = child.result_len;
    let result_log_domain = child.result_log_domain;
    let result_columns = child.result_columns.clone();
    let result_commitments = child.result_commitments.clone();

    Ok(ProvedBgp {
        statement: ProvedQueryStatement::TypecastNumberToString {
            input: Box::new(child),
            result_objects,
        },
        result_commitments,
        result_columns,
        result_log_domain,
        result_len,
    })
}

/// Prove a top-level WOQL STRING→NUMBER `Typecast`: prove the source-binding child through the Store,
/// then DELIVER the per-row numeric cast produced by the SAME [`expected_cast_from_string_objects`]
/// the verifier re-runs. Like `Concat`/`RationalArith`, NO derived column is committed. A
/// malformed/out-of-grammar string, or a non-string operand row, FAILS CLOSED here (exactly as
/// verify does), so a prover cannot produce a proof for a row the reference would reject.
fn prove_typecast_from_string_relation(
    context: &StoreProverContext<'_>,
    layer: &LayerCommitment,
    input: &CompiledRelation,
    source_column: usize,
    target: StringNumericTarget,
) -> std::io::Result<ProvedBgp> {
    let child_digest = relation_query_digest(input)?;
    let child_plan = relation_proof_plan(input, child_digest)?;
    let child_envelope = RecursiveRelationEnvelope::prove_with_context(context, layer, &child_plan)?;
    let child = proved_from_recursive(child_envelope);

    let result_objects = expected_cast_from_string_objects(
        &child,
        input,
        source_column,
        target,
        layer,
        layer.state.commitment_root,
    )?;

    let result_len = child.result_len;
    let result_log_domain = child.result_log_domain;
    let result_columns = child.result_columns.clone();
    let result_commitments = child.result_commitments.clone();

    Ok(ProvedBgp {
        statement: ProvedQueryStatement::TypecastStringToNumber {
            input: Box::new(child),
            result_objects,
        },
        result_commitments,
        result_columns,
        result_log_domain,
        result_len,
    })
}

/// Prove a top-level WOQL INTEGER-result calendar op (`Weekday`/`WeekdaySundayStart`/`IsoWeek`):
/// prove the operand-binding child through the Store, then APPEND the derived integer column(s)
/// computed by the SAME [`expected_calendar_int_columns`] the verifier re-runs. Structurally like
/// [`prove_eval_relation`], but — because `IsoWeek` binds TWO integers — it appends ALL `outputs.len()`
/// derived columns (each committed), NOT just the last: taking only `.last()` would silently drop a
/// column and mis-shape the proof. A non-date operand row / out-of-range year FAILS CLOSED here
/// (exactly as verify does), so a prover cannot produce a proof over a row the reference would reject.
fn prove_calendar_int_relation(
    context: &StoreProverContext<'_>,
    layer: &LayerCommitment,
    input: &CompiledRelation,
    date_column: usize,
    outputs: &[CalendarIntFn],
) -> std::io::Result<ProvedBgp> {
    let child_digest = relation_query_digest(input)?;
    let child_plan = relation_proof_plan(input, child_digest)?;
    let child_envelope = RecursiveRelationEnvelope::prove_with_context(context, layer, &child_plan)?;
    let child = proved_from_recursive(child_envelope);

    // Recompute (child columns ++ one derived column PER output) against THIS layer's own root;
    // identical to the verifier's re-derivation. The trailing `outputs.len()` columns are the derived
    // ones — commit and append EACH (never just the last).
    let expected =
        expected_calendar_int_columns(&child, input, date_column, outputs, layer, layer.state.commitment_root)?;
    let derived_start = expected.len().checked_sub(outputs.len()).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "WOQL calendar-integer recompute produced fewer columns than outputs",
        )
    })?;

    let result_len = child.result_len;
    let result_log_domain = child.result_log_domain;
    let domain = 1usize << result_log_domain;
    let mut result_columns = child.result_columns.clone();
    let mut result_commitments = child.result_commitments.clone();
    for derived in &expected[derived_start..] {
        let mut padded = derived.clone();
        padded.resize(domain, ArkFr::zero());
        result_commitments.push(context.commit_column(&padded)?);
        result_columns.push(derived.clone());
    }

    Ok(ProvedBgp {
        statement: ProvedQueryStatement::CalendarInt {
            input: Box::new(child),
        },
        result_commitments,
        result_columns,
        result_log_domain,
        result_len,
    })
}

/// Prove a top-level WOQL DATE-result calendar op (`DayAfter`/`DayBefore`/`MonthStartDate`/
/// `MonthEndDate`): prove the operand-binding child through the Store, then DELIVER the per-row
/// `xsd:date` result computed by the SAME [`expected_calendar_date_objects`] the verifier re-runs.
/// Like `Concat`, NO derived column is committed — the child's columns/commitments are reused verbatim
/// and the synthesized date is re-checked at verify. A non-zero offset / out-of-range year / wrong-typed
/// operand FAILS CLOSED here (exactly as verify does).
fn prove_calendar_date_relation(
    context: &StoreProverContext<'_>,
    layer: &LayerCommitment,
    input: &CompiledRelation,
    op: CalendarDateFn,
    input_column: usize,
) -> std::io::Result<ProvedBgp> {
    let child_digest = relation_query_digest(input)?;
    let child_plan = relation_proof_plan(input, child_digest)?;
    let child_envelope = RecursiveRelationEnvelope::prove_with_context(context, layer, &child_plan)?;
    let child = proved_from_recursive(child_envelope);

    let result_objects =
        expected_calendar_date_objects(&child, input, op, input_column, layer, layer.state.commitment_root)?;

    let result_len = child.result_len;
    let result_log_domain = child.result_log_domain;
    let result_columns = child.result_columns.clone();
    let result_commitments = child.result_commitments.clone();

    Ok(ProvedBgp {
        statement: ProvedQueryStatement::CalendarDate {
            input: Box::new(child),
            result_objects,
        },
        result_commitments,
        result_columns,
        result_log_domain,
        result_len,
    })
}

/// Prove a top-level WOQL `Typecast`: prove the source-binding child through the Store, then
/// APPEND the derived cast column computed by the SAME recompute the verifier re-runs.
///
/// Structurally identical to [`prove_eval_relation`]: the child's columns/commitments are reused
/// verbatim (already root-authenticated by the Store proof); only the one cast column is committed
/// here. Because it is produced by `expected_typecast_column` — the exact function
/// `verify_relation` re-runs (including the target-range fail-closed check) — a prover cannot
/// deliver a different column and still verify, and cannot prove an out-of-range narrowing at all.
fn prove_typecast_relation(
    context: &StoreProverContext<'_>,
    layer: &LayerCommitment,
    input: &CompiledRelation,
    source_column: usize,
    source_datatype: CanonicalDatatype,
    target_datatype: CanonicalDatatype,
) -> std::io::Result<ProvedBgp> {
    let child_digest = relation_query_digest(input)?;
    let child_plan = relation_proof_plan(input, child_digest)?;
    let child_envelope = RecursiveRelationEnvelope::prove_with_context(context, layer, &child_plan)?;
    let child = proved_from_recursive(child_envelope);

    let expected = expected_typecast_column(
        &child,
        input,
        source_column,
        source_datatype,
        target_datatype,
        layer,
        layer.state.commitment_root,
    )?;
    let derived = expected
        .last()
        .cloned()
        .expect("expected_typecast_column always appends the derived column");

    let result_len = child.result_len;
    let result_log_domain = child.result_log_domain;
    let domain = 1usize << result_log_domain;
    let mut padded = derived.clone();
    padded.resize(domain, ArkFr::zero());
    let derived_commitment = context.commit_column(&padded)?;

    let mut result_columns = child.result_columns.clone();
    result_columns.push(derived);
    let mut result_commitments = child.result_commitments.clone();
    result_commitments.push(derived_commitment);

    Ok(ProvedBgp {
        statement: ProvedQueryStatement::Typecast {
            input: Box::new(child),
        },
        result_commitments,
        result_columns,
        result_log_domain,
        result_len,
    })
}

/// Prove a top-level WOQL `Concatenate`: prove the operand-binding child through the Store, then
/// DELIVER the per-row concatenation computed by the SAME recompute the verifier re-runs.
///
/// Unlike `Eval`/`Typecast`, NO derived column is committed: the concat result is TERMINAL and
/// revealed. The child's columns/commitments are reused verbatim (already root-authenticated by
/// the Store proof); the synthesized strings are carried in the proved statement and re-derived
/// (hence re-checked) at verify. Because they are produced by `expected_concat_objects` — the
/// exact function `verify_relation` re-runs over the root-authenticated operands — a prover
/// cannot deliver a different concatenation and still verify, and cannot prove a concat over a
/// non-string operand row at all (it fails closed here, matching the verifier).
fn prove_concat_relation(
    context: &StoreProverContext<'_>,
    layer: &LayerCommitment,
    input: &CompiledRelation,
    elements: &[ConcatElement],
) -> std::io::Result<ProvedBgp> {
    let child_digest = relation_query_digest(input)?;
    let child_plan = relation_proof_plan(input, child_digest)?;
    let child_envelope = RecursiveRelationEnvelope::prove_with_context(context, layer, &child_plan)?;
    let child = proved_from_recursive(child_envelope);

    // Recompute the per-row concatenation against THIS layer's own root; identical to the
    // verifier's re-derivation (fails closed on a non-string operand row, exactly as verify does).
    let result_objects =
        expected_concat_objects(&child, input, elements, layer, layer.state.commitment_root)?;

    // The committed columns/commitments are the child's, unchanged (terminal result is revealed).
    let result_len = child.result_len;
    let result_log_domain = child.result_log_domain;
    let result_columns = child.result_columns.clone();
    let result_commitments = child.result_commitments.clone();

    Ok(ProvedBgp {
        statement: ProvedQueryStatement::Concat {
            input: Box::new(child),
            result_objects,
        },
        result_commitments,
        result_columns,
        result_log_domain,
        result_len,
    })
}

/// Prove a top-level WOQL `Join`: prove the operand-binding child through the Store, then DELIVER
/// the per-row separator-interposed join produced by the SAME recompute the verifier re-runs
/// ([`expected_join_objects`]). Like `Concat`, NO derived column is committed — the child's
/// columns/commitments are reused verbatim and the synthesized strings are re-checked at verify.
fn prove_join_relation(
    context: &StoreProverContext<'_>,
    layer: &LayerCommitment,
    input: &CompiledRelation,
    elements: &[ConcatElement],
    separator: &ConcatElement,
) -> std::io::Result<ProvedBgp> {
    let child_digest = relation_query_digest(input)?;
    let child_plan = relation_proof_plan(input, child_digest)?;
    let child_envelope = RecursiveRelationEnvelope::prove_with_context(context, layer, &child_plan)?;
    let child = proved_from_recursive(child_envelope);

    let result_objects = expected_join_objects(
        &child,
        input,
        elements,
        separator,
        layer,
        layer.state.commitment_root,
    )?;

    let result_len = child.result_len;
    let result_log_domain = child.result_log_domain;
    let result_columns = child.result_columns.clone();
    let result_commitments = child.result_commitments.clone();

    Ok(ProvedBgp {
        statement: ProvedQueryStatement::Join {
            input: Box::new(child),
            result_objects,
        },
        result_commitments,
        result_columns,
        result_log_domain,
        result_len,
    })
}

/// Prove a top-level WOQL `Substring`: prove the operand-binding child through the Store, then
/// DELIVER the per-row char-substring produced by the SAME recompute the verifier re-runs
/// ([`expected_substring_objects`]). Fails closed here (exactly as verify does) on an out-of-range
/// or non-UTF-8 operand row, so a prover cannot produce a proof over a row the reference drops.
fn prove_substring_relation(
    context: &StoreProverContext<'_>,
    layer: &LayerCommitment,
    input: &CompiledRelation,
    string_column: usize,
    before: usize,
    length: usize,
) -> std::io::Result<ProvedBgp> {
    let child_digest = relation_query_digest(input)?;
    let child_plan = relation_proof_plan(input, child_digest)?;
    let child_envelope = RecursiveRelationEnvelope::prove_with_context(context, layer, &child_plan)?;
    let child = proved_from_recursive(child_envelope);

    let result_objects = expected_substring_objects(
        &child,
        input,
        string_column,
        before,
        length,
        layer,
        layer.state.commitment_root,
    )?;

    let result_len = child.result_len;
    let result_log_domain = child.result_log_domain;
    let result_columns = child.result_columns.clone();
    let result_commitments = child.result_commitments.clone();

    Ok(ProvedBgp {
        statement: ProvedQueryStatement::Substring {
            input: Box::new(child),
            result_objects,
        },
        result_commitments,
        result_columns,
        result_log_domain,
        result_len,
    })
}

/// Prove a WOQL `Pad`: prove the operand-binding child through the Store, then DELIVER the per-row
/// left-padded strings recomputed by the SAME [`expected_pad_objects`] the verifier re-runs — no
/// new Store gadget, no committed result column, the committed columns are the child's verbatim.
fn prove_pad_relation(
    context: &StoreProverContext<'_>,
    layer: &LayerCommitment,
    input: &CompiledRelation,
    string_column: usize,
    pad_char: &[u8],
    target_len: usize,
) -> std::io::Result<ProvedBgp> {
    let child_digest = relation_query_digest(input)?;
    let child_plan = relation_proof_plan(input, child_digest)?;
    let child_envelope = RecursiveRelationEnvelope::prove_with_context(context, layer, &child_plan)?;
    let child = proved_from_recursive(child_envelope);

    let result_objects = expected_pad_objects(
        &child,
        input,
        string_column,
        pad_char,
        target_len,
        layer,
        layer.state.commitment_root,
    )?;

    let result_len = child.result_len;
    let result_log_domain = child.result_log_domain;
    let result_columns = child.result_columns.clone();
    let result_commitments = child.result_commitments.clone();

    Ok(ProvedBgp {
        statement: ProvedQueryStatement::Pad {
            input: Box::new(child),
            result_objects,
        },
        result_commitments,
        result_columns,
        result_log_domain,
        result_len,
    })
}

/// Prove a WOQL `HashKey`/`hash`: prove the operand-binding child through the Store, then DELIVER
/// the per-row `idgen_hash` document-IDs recomputed by the SAME [`expected_hashkey_objects`] the
/// verifier re-runs — no new Store gadget, no committed result column.
fn prove_hashkey_relation(
    context: &StoreProverContext<'_>,
    layer: &LayerCommitment,
    input: &CompiledRelation,
    base: &ConcatElement,
    args: &[ConcatElement],
) -> std::io::Result<ProvedBgp> {
    let child_digest = relation_query_digest(input)?;
    let child_plan = relation_proof_plan(input, child_digest)?;
    let child_envelope = RecursiveRelationEnvelope::prove_with_context(context, layer, &child_plan)?;
    let child = proved_from_recursive(child_envelope);

    let result_objects = expected_hashkey_objects(
        &child,
        input,
        base,
        args,
        layer,
        layer.state.commitment_root,
    )?;

    let result_len = child.result_len;
    let result_log_domain = child.result_log_domain;
    let result_columns = child.result_columns.clone();
    let result_commitments = child.result_commitments.clone();

    Ok(ProvedBgp {
        statement: ProvedQueryStatement::HashKey {
            input: Box::new(child),
            result_objects,
        },
        result_commitments,
        result_columns,
        result_log_domain,
        result_len,
    })
}

fn proved_from_recursive(envelope: RecursiveRelationEnvelope) -> ProvedBgp {
    // The two-root cross-dictionary hash join is carried WHOLESALE (it engages two commitment roots
    // and is verified by `verify_cross_root`, not the single-graph mirror), so it is wrapped directly
    // rather than decomposed into the `ProvedQueryStatement` tree.
    if matches!(envelope.proof, RecursiveRelationProof::CrossRootHashJoin { .. }) {
        return ProvedBgp {
            result_commitments: envelope.result_commitments.clone(),
            result_columns: envelope.result_columns.clone(),
            result_log_domain: envelope.result_log_domain,
            result_len: envelope.result_len,
            statement: ProvedQueryStatement::CrossRootHashJoin { envelope: Box::new(envelope) },
        };
    }
    let statement = match envelope.proof {
        RecursiveRelationProof::LayerTripleCount => ProvedQueryStatement::LayerTripleCount,
        RecursiveRelationProof::GeneratedU64Filter { input } => {
            ProvedQueryStatement::GeneratedU64Filter {
                input: Box::new(proved_from_recursive(*input)),
            }
        }
        RecursiveRelationProof::GeneratedI64Filter { input } => {
            ProvedQueryStatement::GeneratedI64Filter {
                input: Box::new(proved_from_recursive(*input)),
            }
        }
        RecursiveRelationProof::Bgp(leaf) => ProvedQueryStatement::Single {
            proof: Box::new(leaf.proof),
            predicate_ids: leaf.predicate_ids,
        },
        RecursiveRelationProof::BagUnion { branches, proof } => ProvedQueryStatement::BagUnion {
            branches: branches.into_iter().map(proved_from_recursive).collect(),
            proof,
        },
        RecursiveRelationProof::BagProjection { input } => {
            ProvedQueryStatement::BagProjection {
                input: Box::new(proved_from_recursive(*input)),
            }
        }
        RecursiveRelationProof::Distinct { input, proof } => ProvedQueryStatement::Distinct {
            input: Box::new(proved_from_recursive(*input)),
            proof,
        },
        RecursiveRelationProof::LeftAnti { left, right, proof } => ProvedQueryStatement::LeftAnti {
            left: Box::new(proved_from_recursive(*left)),
            right: Box::new(proved_from_recursive(*right)),
            proof,
        },
        RecursiveRelationProof::InnerJoin { left, right, proof } => ProvedQueryStatement::InnerJoin {
            left: Box::new(proved_from_recursive(*left)),
            right: Box::new(proved_from_recursive(*right)),
            proof,
        },
        RecursiveRelationProof::LeftOuter { left, right, proof } => {
            ProvedQueryStatement::LeftOuter {
                left: Box::new(proved_from_recursive(*left)),
                right: Box::new(proved_from_recursive(*right)),
                proof,
            }
        }
        RecursiveRelationProof::Equality { input, proof } => ProvedQueryStatement::Equality {
            input: Box::new(proved_from_recursive(*input)),
            proof,
        },
        RecursiveRelationProof::TupleDisequality { input, proof } => {
            ProvedQueryStatement::TupleDisequality {
                input: Box::new(proved_from_recursive(*input)),
                proof,
            }
        }
        RecursiveRelationProof::ExactScalarOrder { input, proof } => {
            ProvedQueryStatement::ExactScalarOrder {
                input: Box::new(proved_from_recursive(*input)),
                proof,
            }
        }
        RecursiveRelationProof::ExactMultiOrder { input, proof } => ProvedQueryStatement::ExactMultiOrder {
            input: Box::new(proved_from_recursive(*input)), proof,
        },
        RecursiveRelationProof::ExactOrderedSlice { input } => {
            ProvedQueryStatement::ExactOrderedSlice {
                input: Box::new(proved_from_recursive(*input)),
            }
        }
        RecursiveRelationProof::GroupExtrema { input } => {
            ProvedQueryStatement::GroupExtrema {
                input: Box::new(proved_from_recursive(*input)),
            }
        }
        RecursiveRelationProof::GroupSum { input } => ProvedQueryStatement::GroupSum {
            input: Box::new(proved_from_recursive(*input)),
        },
        RecursiveRelationProof::TransitiveClosure {
            edge,
            seed,
            rounds,
            fixpoint,
        } => ProvedQueryStatement::TransitiveClosure {
            edge: Box::new(proved_from_recursive(*edge)),
            seed,
            rounds,
            fixpoint: Box::new(fixpoint),
        },
        RecursiveRelationProof::GroupCount { input } => ProvedQueryStatement::GroupCount {
            input: Box::new(proved_from_recursive(*input)),
        },
        RecursiveRelationProof::CollectionCount { input } => {
            ProvedQueryStatement::CollectionCount {
                input: Box::new(proved_from_recursive(*input)),
            }
        }
        RecursiveRelationProof::CollectionSum { input } => ProvedQueryStatement::CollectionSum {
            input: Box::new(proved_from_recursive(*input)),
        },
        RecursiveRelationProof::CollectionExtrema { input } => {
            ProvedQueryStatement::CollectionExtrema {
                input: Box::new(proved_from_recursive(*input)),
            }
        }
        // The two-root cross-dictionary hash join is carried WHOLESALE by the early return at the top of
        // this function (it engages two roots and is verified by `verify_cross_root`, not the
        // single-graph mirror), so it never reaches this decomposition match.
        RecursiveRelationProof::CrossRootHashJoin { .. } => unreachable!(
            "cross-root envelopes are wrapped wholesale by the early return, not decomposed here"
        ),
    };
    ProvedBgp {
        statement,
        result_commitments: envelope.result_commitments,
        result_columns: envelope.result_columns,
        result_log_domain: envelope.result_log_domain,
        result_len: envelope.result_len,
    }
}

/// Errors at the explicit compile/load/prove boundary.
#[derive(Debug, thiserror::Error)]
pub enum QueryProofError {
    #[error(transparent)]
    Plan(#[from] BgpPlanError),
    #[error(transparent)]
    Store(#[from] std::io::Error),
}


fn store_id_for_field(
    field: ArkFr,
    namespace: ResultIdNamespace,
    layer: &impl VerifierLayer,
) -> std::io::Result<u64> {
    let view = layer.verifier_view();
    match namespace {
        ResultIdNamespace::Predicate => view
            .predicate_catalog
            .iter()
            .find(|r| ArkFr::from_u64(r.predicate_id) == field)
            .map(|r| r.predicate_id),
        ResultIdNamespace::NodeValue => view
            .node_value_catalog
            .iter()
            .find(|r| ArkFr::from_u64(r.object_id) == field)
            .map(|r| r.object_id),
        ResultIdNamespace::GeneratedDecimalU64
        | ResultIdNamespace::GeneratedDecimalI64 => None,
    }
    .ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "group-extrema Store id absent from planned namespace",
        )
    })
}

fn group_output_rows(
    columns: &[Vec<ArkFr>],
    n: usize,
    relation: &CompiledRelation,
    layer: &impl VerifierLayer,
) -> std::io::Result<Vec<(u64, u64)>> {
    if columns.len() != 2 || columns.iter().any(|c| c.len() < n) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "group-extrema output shape mismatch",
        ));
    }
    let namespace = relation_result_descriptors(relation)?[0].namespace;
    (0..n)
        .map(|i| {
            Ok((
                store_id_for_field(columns[0][i], namespace, layer)?,
                store_id_for_field(columns[1][i], ResultIdNamespace::NodeValue, layer)?,
            ))
        })
        .collect()
}

fn distinct_output_rows(
    columns: &[Vec<ArkFr>],
    n: usize,
    relation: &CompiledRelation,
    layer: &impl VerifierLayer,
) -> std::io::Result<Vec<Vec<u64>>> {
    let descriptors = relation_result_descriptors(relation)?;
    if columns.len() != descriptors.len() || columns.iter().any(|column| column.len() < n) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "distinct output shape mismatch",
        ));
    }
    (0..n)
        .map(|row| {
            columns
                .iter()
                .zip(&descriptors)
                .map(|(column, descriptor)| {
                    store_id_for_field(column[row], descriptor.namespace, layer)
                })
                .collect()
        })
        .collect()
}


impl CompiledBgp {
    /// Generate a proof against an already loaded authenticated commitment.
    pub fn prove(&self, layer: &LayerCommitment) -> std::io::Result<ProvedBgp> {
        self.prove_with_context(&StoreProverContext::cpu(), layer)
    }

    /// Generate the complete recursive relation proof using caller-selected arithmetic.
    pub fn prove_with_context(
        &self,
        context: &StoreProverContext<'_>,
        layer: &LayerCommitment,
    ) -> std::io::Result<ProvedBgp> {
        // WOQL `Eval` is a woql2-side verifier-recompute layered over a Store-proven child.
        // Prove the operand-binding child through the Store, then APPEND the derived integer
        // column using the same recompute the verifier will re-run — no new Store gadget and
        // no change to the existing committed columns.
        if let CompiledRelationStatement::Eval { input, expr } = &self.relation.statement {
            return prove_eval_relation(context, layer, input, expr);
        }
        // WOQL `Typecast` is likewise a woql2-side verifier-recompute over a Store-proven child.
        if let CompiledRelationStatement::Typecast {
            input,
            source_column,
            source_datatype,
            target_datatype,
        } = &self.relation.statement
        {
            return prove_typecast_relation(
                context,
                layer,
                input,
                *source_column,
                *source_datatype,
                *target_datatype,
            );
        }
        // WOQL `Concatenate` is likewise a woql2-side verifier-recompute over a Store-proven
        // child: prove the operand-binding child through the Store, then DELIVER the per-row
        // concatenation recomputed by the same function the verifier re-runs — no new Store
        // gadget, no committed result column, no change to the existing committed columns.
        if let CompiledRelationStatement::Concat { input, elements, .. } = &self.relation.statement {
            return prove_concat_relation(context, layer, input, elements);
        }
        // WOQL `Join` — Concat with the separator interposed; same woql2-side recompute discipline.
        if let CompiledRelationStatement::Join {
            input,
            elements,
            separator,
            ..
        } = &self.relation.statement
        {
            return prove_join_relation(context, layer, input, elements, separator);
        }
        // WOQL `Substring` — a per-row char-slice of a Store-proven authenticated string column.
        if let CompiledRelationStatement::Substring {
            input,
            string_column,
            before,
            length,
            ..
        } = &self.relation.statement
        {
            return prove_substring_relation(context, layer, input, *string_column, *before, *length);
        }
        // WOQL `Pad` — a per-row left-pad of a Store-proven authenticated string column.
        if let CompiledRelationStatement::Pad {
            input,
            string_column,
            pad_char,
            target_len,
            ..
        } = &self.relation.statement
        {
            return prove_pad_relation(context, layer, input, *string_column, pad_char, *target_len);
        }
        // WOQL `HashKey`/`hash` — a per-row `idgen_hash` over Store-proven authenticated operands.
        if let CompiledRelationStatement::HashKey {
            input, base, args, ..
        } = &self.relation.statement
        {
            return prove_hashkey_relation(context, layer, input, base, args);
        }
        // WOQL `floor(x)` — a per-row exact-rational floor over a Store-proven authenticated child.
        if let CompiledRelationStatement::FloorEval { input, expr } = &self.relation.statement {
            return prove_floor_eval_relation(context, layer, input, expr);
        }
        // WOQL exact-rational arithmetic (`Divide`/decimal operands) — a per-row terminal
        // xsd:decimal delivery over a Store-proven authenticated child.
        if let CompiledRelationStatement::RationalArith { input, expr, .. } =
            &self.relation.statement
        {
            return prove_rational_arith_relation(context, layer, input, expr);
        }
        // WOQL NUMBER→STRING `Typecast` — a per-row terminal xsd:string delivery over a Store-proven
        // authenticated numeric child.
        if let CompiledRelationStatement::TypecastNumberToString {
            input,
            source_column,
            ..
        } = &self.relation.statement
        {
            return prove_typecast_to_string_relation(context, layer, input, *source_column);
        }
        // WOQL STRING→NUMBER `Typecast` — a per-row terminal numeric delivery over a Store-proven
        // authenticated xsd:string child.
        if let CompiledRelationStatement::TypecastStringToNumber {
            input,
            source_column,
            target,
            ..
        } = &self.relation.statement
        {
            return prove_typecast_from_string_relation(
                context,
                layer,
                input,
                *source_column,
                *target,
            );
        }
        // WOQL INTEGER-result calendar ops — a per-row map over a Store-proven authenticated date
        // column, appending the derived integer column(s) via the same recompute verify re-runs.
        if let CompiledRelationStatement::CalendarInt {
            input,
            date_column,
            outputs,
        } = &self.relation.statement
        {
            return prove_calendar_int_relation(context, layer, input, *date_column, outputs);
        }
        // WOQL DATE-result calendar ops — a per-row terminal xsd:date delivery over a Store-proven
        // authenticated operand column (like Concat/RationalArith).
        if let CompiledRelationStatement::CalendarDate {
            input,
            op,
            input_column,
            ..
        } = &self.relation.statement
        {
            return prove_calendar_date_relation(context, layer, input, *op, *input_column);
        }
        let digest = compiled_query_digest(self)?;
        let plan = relation_proof_plan(&self.relation, digest)?;
        let envelope = RecursiveRelationEnvelope::prove_with_context(context, layer, &plan)?;
        Ok(proved_from_recursive(envelope))
    }

    /// Explicitly load the commitment from a synchronous Store layer and prove. Calling
    /// this method is the lifecycle decision; no worker or background job is started.
    ///
    /// INSTANCE-ONLY: this entry loads only the instance-graph commitment (byte-identical to before
    /// the schema-graph foundation). A query that reads the SCHEMA graph (Subsumption) rejects closed
    /// here — use [`CompiledBgp::prove_on_layers`], which also loads the schema commitment.
    pub fn prove_on_layer(&self, layer: &SyncStoreLayer) -> Result<ProvedBgp, QueryProofError> {
        self.prove_on_layer_with_context(&StoreProverContext::cpu(), layer)
    }

    /// Load the commitment from a synchronous Store layer and prove with the caller's
    /// context. No runtime initialization or scheduling occurs here.
    pub fn prove_on_layer_with_context(
        &self,
        context: &StoreProverContext<'_>,
        layer: &SyncStoreLayer,
    ) -> Result<ProvedBgp, QueryProofError> {
        // A schema-reading query cannot be proven with only the instance layer — reject closed rather
        // than silently anchor a schema proof to the instance root.
        if relation_graph(&self.relation)? != ProofGraph::Instance {
            return Err(QueryProofError::Store(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "query reads the schema graph; prove with `prove_on_layers` (instance + schema layers)",
            )));
        }
        let commitment = layer.proof_commitment()?;
        Ok(self.prove_with_context(context, &commitment)?)
    }

    /// SCHEMA-GRAPH FOUNDATION — prove over BOTH committed graphs (instance + schema).
    ///
    /// Selects, from the compiled plan's single [`relation_graph`], which layer's `LayerCommitment`
    /// the whole proof is built over: an instance query proves over the instance commitment exactly as
    /// [`prove_on_layer`] (byte-identical), a Subsumption query proves over the schema commitment. The
    /// audited recursion is handed that ONE commitment and stays graph-blind. The trust anchor a
    /// verifier reconstructs is the combined [`terminus_store::proof::commitment::database_root`] of
    /// the two per-graph roots (see [`CompiledBgp::prove_on_layers_root`]).
    pub fn prove_on_layers(
        &self,
        instance: &SyncStoreLayer,
        schema: &SyncStoreLayer,
    ) -> Result<ProvedBgp, QueryProofError> {
        self.prove_on_layers_with_context(&StoreProverContext::cpu(), instance, schema)
    }

    /// [`prove_on_layers`] with a caller-owned prover context.
    pub fn prove_on_layers_with_context(
        &self,
        context: &StoreProverContext<'_>,
        instance: &SyncStoreLayer,
        schema: &SyncStoreLayer,
    ) -> Result<ProvedBgp, QueryProofError> {
        // The cross-root hash join (IsA) reads BOTH graphs: prove it through the two-layer store entry
        // (left=schema, right=instance), binding both roots to the trusted `database_root`.
        if matches!(self.relation.statement, CompiledRelationStatement::CrossRootHashJoin { .. }) {
            let instance_commit = instance.proof_commitment()?;
            let schema_commit = schema.proof_commitment()?;
            let digest = compiled_query_digest(self)?;
            let plan = relation_proof_plan(&self.relation, digest)?;
            let envelope = RecursiveRelationEnvelope::prove_cross_root_with_context(
                context, &instance_commit, &schema_commit, &plan,
            )?;
            return Ok(proved_from_recursive(envelope));
        }
        let commitment = match relation_graph(&self.relation)? {
            ProofGraph::Instance => instance.proof_commitment()?,
            ProofGraph::Schema => schema.proof_commitment()?,
        };
        Ok(self.prove_with_context(context, &commitment)?)
    }

    /// Prove a top-level cross-root (IsA) query directly over two already-committed layers (the
    /// instance and schema `LayerCommitment`s). Mirrors [`prove_on_layers`] for callers that hold
    /// commitments rather than `SyncStoreLayer`s.
    pub fn prove_cross_root_layers(
        &self,
        instance: &LayerCommitment,
        schema: &LayerCommitment,
    ) -> Result<ProvedBgp, QueryProofError> {
        self.prove_cross_root_layers_with_context(&StoreProverContext::cpu(), instance, schema)
    }

    pub fn prove_cross_root_layers_with_context(
        &self,
        context: &StoreProverContext<'_>,
        instance: &LayerCommitment,
        schema: &LayerCommitment,
    ) -> Result<ProvedBgp, QueryProofError> {
        if !matches!(self.relation.statement, CompiledRelationStatement::CrossRootHashJoin { .. }) {
            return Err(QueryProofError::Store(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "prove_cross_root_layers requires a cross-root (IsA) query",
            )));
        }
        let digest = compiled_query_digest(self)?;
        let plan = relation_proof_plan(&self.relation, digest)?;
        let envelope = RecursiveRelationEnvelope::prove_cross_root_with_context(
            context, instance, schema, &plan,
        )?;
        Ok(proved_from_recursive(envelope))
    }

    /// The externally-trusted combined `database_root` for a two-graph database: binds the instance
    /// and schema per-graph commitment roots. A caller publishes this alongside a proof so a verifier
    /// can validate both commitments against it (see
    /// [`terminus_store::proof::commitment::database_root`]).
    ///
    /// TRUST ANCHOR: a verifier must obtain this `database_root` from a trusted OUT-OF-BAND pin (commit
    /// log / signature over the head) — NEVER by recomputing it from a prover-supplied proof bundle or
    /// schema view. A prover controls the schema layer it commits, so a `database_root` recomputed from
    /// prover input would happily bind a FABRICATED schema (with an injected `sys:inherits` superclass)
    /// and let a forged subsumption verify. See [`ProvedBgp::verify_over_graphs`].
    pub fn prove_on_layers_root(
        instance: &SyncStoreLayer,
        schema: &SyncStoreLayer,
    ) -> Result<ProofHash, QueryProofError> {
        let instance_root = instance.proof_commitment()?.state.commitment_root;
        let schema_root = schema.proof_commitment()?.state.commitment_root;
        Ok(terminus_store::proof::commitment::database_root(
            &instance_root,
            &schema_root,
        ))
    }
}

/// Convenience for app/domain callers that want one explicit compile-and-prove call.
pub fn compile_and_prove(
    query: &Query,
    layer: &SyncStoreLayer,
) -> Result<(CompiledBgp, ProvedBgp), QueryProofError> {
    compile_and_prove_with_context(&StoreProverContext::cpu(), query, layer)
}

/// Compile and prove with an explicitly caller-owned Store prover context.
pub fn compile_and_prove_with_context(
    context: &StoreProverContext<'_>,
    query: &Query,
    layer: &SyncStoreLayer,
) -> Result<(CompiledBgp, ProvedBgp), QueryProofError> {
    let compiled = plan_bgp(query)?;
    let proof = compiled.prove_on_layer_with_context(context, layer)?;
    Ok((compiled, proof))
}

/// Every reason [`plan_bgp`] refuses a query. One variant per rejection reason.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BgpPlanError {
    /// A construct that is not a supported proof wrapper, triple pattern, conjunction,
    /// or comparison appeared in the query body.
    #[error(
        "unsupported construct `{construct}`: only a conjunction (And) of triples is supported"
    )]
    UnsupportedConstruct { construct: &'static str },

    /// A conjunct inside the `And` was neither a `Triple` (or nested `And`) nor a numeric
    /// comparison (`Greater`/`Less`/`Gte`/`Lte`, routed to object-value range filters).
    #[error("unsupported conjunct `{construct}`: every conjunct must be a Triple or a numeric comparison")]
    ConjunctNotTriple { construct: &'static str },

    /// A comparison conjunct had a VARIABLE on both sides (`?a op ?b`): a range filter
    /// needs exactly one variable and one numeric constant.
    #[error("range comparison has a variable on both sides: exactly one side must be a numeric constant")]
    RangeBothVariables,

    /// A comparison conjunct had a CONSTANT on both sides (no variable to range over).
    #[error("range comparison has no variable: exactly one side must be the compared variable")]
    RangeBothConstants,

    /// A comparison's constant is not a round-trip-aligned order-preserving literal.
    /// Supported: `xsd:boolean`, the explicit fixed-width XSD integer family, the bounded-i64
    /// subset currently representable by the AST's `xsd:integer`, `xsd:float`/`double`
    /// (`Float64`), and `xsd:date`/`xsd:dateTime` (via the store's decoded i64-nanoseconds
    /// key). Strings, decimals, arbitrary-precision integers outside i64, other temporal
    /// types (`dateTimeStamp`/`time`/`duration`/g*), other datatypes, and list operands
    /// reject closed.
    #[error(
        "unsupported range constant `{datatype}`: no round-trip-aligned order-preserving encoding"
    )]
    UnsupportedRangeDatatype { datatype: &'static str },

    /// A `date`/`dateTime` range constant lies at or outside the representable
    /// i64-nanoseconds window (≈ 1677-09-21T00:12:43.145224192Z ..
    /// 2262-04-11T23:47:16.854775807Z, endpoints EXCLUDED). The OP encoding SATURATES data
    /// dates outside that window to `i64::MIN`/`i64::MAX`; requiring the constant strictly
    /// inside it keeps every far data date strictly ordered against the constant, so the
    /// filter stays sound. A constant that far out is astronomically implausible, so we
    /// reject-close rather than special-case the saturated boundary.
    #[error("date/dateTime range constant is outside the exact i64-nanoseconds window")]
    DateRangeConstantOutOfExactRange,

    /// A ranged variable is not the object of any atom, so there is no object-binding atom
    /// to attach the range to.
    #[error("range variable `?{var}` is not the object of any atom")]
    RangeVariableNotObject { var: String },

    /// A ranged variable also occupies a SUBJECT position. v1 attaches a range only to a
    /// variable that is purely an atom's object (never a join key on the subject side).
    #[error(
        "range variable `?{var}` appears as a subject: only a pure object variable can be ranged"
    )]
    RangeVariableIsSubject { var: String },

    /// A ranged variable is the object of MORE THAN ONE atom. v1 requires a unique
    /// object-binding atom to attach the range to.
    #[error("range variable `?{var}` is the object of more than one atom")]
    RangeVariableObjectOfMultiple { var: String },

    /// More than two range bounds target the same variable.
    #[error("more than two range bounds on variable `?{var}`")]
    DuplicateRangeOnVariable { var: String },

    #[error("InRange value must be a variable")]
    InRangeValueNotVariable,

    #[error("InRange bounds must be scalar constants")]
    InRangeBoundNotConstant,

    #[error("range bounds for `?{var}` use different datatypes")]
    RangeDatatypeMismatch { var: String },

    #[error("range variable `?{var}` is constrained to a node object")]
    RangeOnNodeObject { var: String },

    /// No triple atoms remain after normalization.
    #[error("BGP needs at least one atom, found {count}")]
    TooFewAtoms { count: usize },

    #[error("variable `?{var}` occurs in both predicate and node/value positions")]
    VariableNamespaceConflict { var: String },

    /// The object position held a compound value the store's node/value dictionary does
    /// not model as a single constant (a `Dictionary` or `List`). Node IRIs and aligned
    /// typed literals are accepted; these are not.
    #[error(
        "unsupported object value `{kind}`: only node IRIs and aligned typed literals are allowed"
    )]
    UnsupportedObjectValue { kind: &'static str },

    /// The object was a typed literal whose xsd datatype is not one this compiler can
    /// round-trip-align with the store's value canonicalization (only the verified subset
    /// is emitted; every other datatype rejects closed rather than authenticate an
    /// unproven constant). See CP3.
    #[error("unsupported value datatype `{datatype}`: no round-trip-aligned encoding")]
    UnsupportedValueDatatype { datatype: &'static str },

    /// Reserved for malformed externally-built plans. The compiler now emits sound
    /// mixed-arity gating plans directly.
    #[error("invalid ground/relational atom composition")]
    GroundAtom,

    /// The triple targeted the schema graph.
    #[error("schema-graph triples unsupported: only the instance graph is queried")]
    SchemaGraphUnsupported,

    /// RESERVED (no longer produced): a BGP leaf that mixes column sources — a materialized
    /// `Triple` joined with a delta `AddedTriple`/`DeletedTriple`, or additions joined with
    /// removals. Cross-source joins are now SUPPORTED: each atom carries its own
    /// [`ColumnSource`], and the store joins over the shared dictionaries. Retained as a public
    /// variant for API stability.
    #[error("cross-source conjunction unsupported: a BGP may not mix materialized/added/deleted triples")]
    MixedColumnSource,

    /// Reserved for malformed externally-built plans. The compiler now composes
    /// disconnected components through authenticated Cartesian products.
    #[error("disconnected BGP")]
    DisconnectedBgp,

    #[error("Or needs at least two branches, found {count}")]
    OrTooFewBranches { count: usize },

    #[error("Or branches must expose the same variable schema")]
    OrIncompatibleSchema,

    #[error("Or branches expose incompatible Store ID namespaces")]
    OrIncompatibleNamespaces,

    #[error("Select, Count, or Distinct inside an Or branch is unsupported")]
    OrUnsupportedBranchWrapper,

    #[error("Or branches resolve to conflicting collections")]
    OrRoutingConflict,

    #[error("correlated Not needs a non-empty positive outer BGP")]
    NotMissingOuter,

    #[error("Not supports at most two shared variables, found {count}")]
    NotSharedVariableCount { count: usize },

    #[error("Not subplans may not contain result wrappers or correlate through nullable columns")]
    NotUnsupportedSubplan,

    #[error("Not subplans resolve to conflicting collections")]
    NotRoutingConflict,

    #[error("Optional needs a non-empty positive outer BGP")]
    OptionalMissingOuter,

    #[error("Optional supports at most two shared variables, found {count}")]
    OptionalSharedVariableCount { count: usize },

    #[error(
        "Optional subplans may not contain result wrappers or correlate through nullable columns"
    )]
    OptionalUnsupportedSubplan,

    #[error("recursive proof relation exceeds depth or node limits")]
    RecursiveStructureLimit,

    #[error("Equals needs an already-authenticated input relation")]
    EqualsMissingInput,

    #[error("Equals operand is a list or dictionary")]
    EqualsUnsupportedOperand,

    #[error("Equals variable `?{var}` is not already bound by the preceding relation")]
    EqualsUnboundVariable { var: String },

    #[error("Equals variable `?{var}` is nullable")]
    EqualsNullableVariable { var: String },

    #[error("Equals variable `?{var}` has no exact node/predicate/datatype domain proof")]
    EqualsUnknownDomain { var: String },

    #[error("Equals operands have incompatible exact value domains")]
    EqualsDomainMismatch,

    #[error("cross-datatype numeric Equals uses coercive executor semantics")]
    EqualsCoerciveNumeric,

    #[error("GroupBy extrema requires zero to two unique node keys and an immediately consuming RangeMin/RangeMax over the hidden grouped list")]
    GroupExtremaUnsupportedShape,

    #[error("GroupBy extrema template must be a nonnullable variable with an exact fixed-width integral datatype")]
    GroupExtremaUnsupportedValue,

    #[error("GroupBy followed by Sum requires zero to two unique node keys, one private grouped-list variable, and one distinct result variable")]
    GroupSumUnsupportedShape,

    #[error("GroupBy Sum template must be an exact fixed-width signed or unsigned integer variable")]
    GroupSumUnsupportedValue,

    #[error("GroupBy followed by Length requires zero to two unique node keys, one private grouped-list variable, and one distinct result variable")]
    GroupCountUnsupportedShape,

    #[error("GroupBy Length template must be a nonnullable bound Store-ID variable")]
    GroupCountUnsupportedValue,

    #[error("Collect followed by Length requires one private collected-list variable consumed immediately once and one distinct result variable")]
    CollectionCountUnsupportedShape,

    #[error("Collect followed by Sum requires one private collected-list variable consumed immediately once and one distinct result variable")]
    CollectionSumUnsupportedShape,

    #[error("Collect-Sum template must be an exact fixed-width integral constant or nonnullable child-bound variable")]
    CollectionSumUnsupportedValue,

    #[error("Collect followed by RangeMin/RangeMax requires one private list consumed immediately once and one distinct result variable")]
    CollectionExtremaUnsupportedShape,

    #[error("Collect extrema template must be one exact nonnullable fixed-width integral child variable")]
    CollectionExtremaUnsupportedValue,

    #[error("generated GroupBy-Sum comparisons require the sum source before comparisons of that result variable to exact integral xsd:decimal constants")]
    GroupSumComparisonUnsupportedShape,

    #[error("generated GroupBy-Sum comparison constants must be exact integral xsd:decimal values in the generated result domain")]
    GroupSumComparisonUnsupportedConstant,

    #[error("OrderBy proof coverage requires one ascending key naming the complete one-column result of a leaf BGP")]
    OrderByUnsupportedShape,

    #[error("OrderBy proof coverage currently supports ascending order only")]
    OrderByUnsupportedDirection,

    #[error("OrderBy key must have an exact fixed-width order-preserving numeric datatype")]
    OrderByUnsupportedValue,

    #[error("Start/Limit proof coverage requires an authenticated ordered child without result wrappers")]
    OrderedSliceUnsupportedShape,

    #[error("ordered triple-value proof coverage requires instance graph, constant subject/predicate, and exactly one object/cursor variable")]
    TripleValueOrderUnsupportedShape,

    #[error("ordered triple-value proof coverage requires fixed-width numeric literal bounds of one datatype")]
    TripleValueOrderUnsupportedValue,

    #[error("TripleCount proof coverage requires an explicit instance-graph resource")]
    TripleCountUnsupportedResource,

    #[error("TripleCount output must be a variable")]
    TripleCountOutputNotVariable,

    #[error("generated TripleCount comparisons require the count source before one or more comparisons of that same variable to exact integral xsd:decimal constants")]
    TripleCountComparisonUnsupportedShape,

    #[error("generated TripleCount comparison constants must be exact nonnegative integral xsd:decimal values in the u64 range")]
    TripleCountComparisonUnsupportedConstant,

    #[error("Optional subplans resolve to conflicting collections")]
    OptionalRoutingConflict,

    #[error("When needs a preceding non-empty authenticated outer relation")]
    WhenMissingOuter,

    #[error("When condition/consequent must share one or two nonnullable variables with their input relation")]
    WhenUnsupportedVariables,

    #[error("When branch-local variables would escape the result: {vars:?}")]
    WhenBranchLocalsEscape { vars: Vec<String> },

    #[error("When branch-local variable `?{var}` is referenced by a later operator")]
    WhenBranchLocalUsedLater { var: String },

    #[error("When condition/consequent may not contain result wrappers or nullable correlation keys")]
    WhenUnsupportedSubplan,

    #[error("When subplans resolve to conflicting collections")]
    WhenRoutingConflict,

    /// A `Select`ed variable does not occur in the BGP body.
    #[error("selected variable `?{var}` does not occur in the BGP")]
    SelectedVariableNotInBgp { var: String },

    /// Count output must be a variable; constant/list outputs need unification semantics
    /// that are not represented by the current result metadata.
    #[error("Count output must be a variable")]
    CountOutputNotVariable,

    #[error("Distinct with visible projection/ordering is unsupported; projected Distinct is currently exact only beneath Count")]
    DistinctWithProjectionOrCount,

    #[error("Distinct variables must name the complete BGP schema")]
    DistinctNotFullSchema,

    #[error("projected Distinct requires one or two witness variables, found {count}")]
    DistinctUnsupportedKeyCount { count: usize },

    #[error("Distinct witness `?{var}` must have an exact nonnullable Store-ID domain")]
    DistinctUnsupportedDomain { var: String },

    #[error("nested Distinct wrappers are unsupported")]
    NestedDistinctUnsupported,

    /// A full-schema `Distinct` whose child can carry duplicate full rows (an
    /// `Or`/`BagUnion`, a `Not`/`LeftAnti`, an `Optional`/`LeftOuter`, a projection, a
    /// `When`, ...). The transparent full-row path is only a sound no-op over a relation
    /// whose rows are already set-valued and unique (a pure natural-join relation); over a
    /// bag it would silently pass a NON-deduplicated result off as `Distinct`. Full-schema
    /// dedup over a bag is deferred to a future real-dedup gadget and rejected closed.
    #[error("full-schema Distinct over a duplicate-producing child requires a real dedup gadget; only Distinct over a pure natural-join relation is currently a sound no-op")]
    DistinctOverDuplicateProducingChild,

    /// A composite (two-key) correlated join whose packed key columns are not both Store-ID
    /// (node/value or predicate) namespaces. The two keys are packed as `col0 + 2^64·col1`,
    /// injective only when both keys are Store dictionary IDs (`< 2^64`); a generated-decimal
    /// namespace can reach ~2^254 and collide, so it is rejected closed.
    #[error("composite join key `?{var}` must be a Store-ID (node/value or predicate) namespace; packed two-key joins are injective only for keys below 2^64")]
    CompositeJoinKeyNotStoreId { var: String },

    // ---- Fixed-length property-path lowering (`Query::Path`) ----
    /// A `Path` whose `Star`, `Plus`, or unbounded `Times` pattern requires authenticated
    /// transitive closure / regular-path evaluation, which the fixed-length lowering does
    /// not model. These need a future bounded-iteration/closure layer and reject closed.
    #[error("unbounded property path (`Star`/`Plus`/open `Times`) requires transitive closure, which is out of scope for the fixed-length path lowering")]
    UnboundedPathUnsupported,

    /// A `Path` that captures the traversed edge list into its `path` field. The lowering
    /// produces only the endpoint relation and cannot authenticate the edge-list witness.
    #[error("Path edge-list capture (`path` field) is unsupported: the lowering proves only the endpoint relation")]
    PathWitnessCaptureUnsupported,

    /// A `Path` endpoint (`subject`/`object`) is neither a node IRI nor a variable.
    #[error("Path endpoint must be a node IRI or a variable")]
    PathEndpointUnsupported,

    /// A `PathPredicate`/`InversePathPredicate` with no predicate (the `.` any-predicate
    /// wildcard). The proof authenticates a concrete predicate IRI; a wildcard is rejected.
    #[error("wildcard path predicate (`.`) is unsupported: a concrete predicate IRI is required")]
    PathWildcardUnsupported,

    /// A multi-hop path reuses a predicate across two hops. Leading-Prolog path evaluation
    /// is edge-SIMPLE (`Open_Set` forbids re-traversing an identical edge within one path),
    /// which plain conjunctive bag-join does NOT model: on self-loop/edge-reuse data the
    /// join over-counts. Paths with pairwise-distinct hop predicates (no edge can repeat)
    /// lower straight to a bag-join; a SINGLE-predicate repeat is instead lowered with a
    /// `TupleDisequality` edge-simple filter (see [`plan_deferred_repeated_predicate_path`]).
    /// This error is the internal signal `ensure_edge_simple_exact` raises to route a
    /// repeated-predicate chain to that deferred handler; it never escapes for a supported
    /// single-predicate path.
    #[error("multi-hop path reuses predicate `{predicate}`; edge-simple path semantics require a TupleDisequality filter (deferred lowering)")]
    PathRepeatedPredicateUnsupported { predicate: String },

    /// A repeated-predicate path whose hops do NOT all use the SAME predicate (a
    /// MIXED-predicate repeat, e.g. `(p;q)^2` or `Sequence([p, q, p])`). Edge-simple
    /// exactness for a mixed repeat must fold the predicate id into the edge fingerprint,
    /// which is a later sub-effort; only single-predicate repeats are provable here, so a
    /// mixed-predicate repeat rejects closed.
    #[error("mixed-predicate repeated path is unsupported: only single-predicate repeats have an edge-simple TupleDisequality lowering")]
    PathMixedPredicateRepeatUnsupported,

    /// A conjunctive-chain hop is itself a union/range that cannot be a pure triple chain
    /// (an `Or`, or a bounded-range `Times`), so the enclosing `Sequence`/`Times` cannot be
    /// expressed as a left-deep triple chain.
    #[error("non-elementary path hop (`Or`/bounded-range `Times`) inside a sequence is unsupported")]
    PathNonElementaryHopUnsupported,

    /// A union alternative (a top-level `Or` branch or a bounded-range `Times` length) is a
    /// multi-hop chain. Multi-hop union branches introduce per-branch intermediate variables
    /// that would leak into (or make incompatible) the union schema, so they reject closed.
    #[error("multi-hop path alternative in a union (`Or`/range `Times`) is unsupported: union branches must be single hops")]
    PathUnionBranchMultiHop,

    /// A `Times` whose range includes length 0 (the zero-length identity `S == T`). The
    /// identity relation is not a triple pattern; per the leading-Prolog identity semantics
    /// this rejects closed.
    #[error("zero-length path (identity, `Times` from 0) is unsupported")]
    PathIdentityUnsupported,

    /// An empty path pattern (an empty `Sequence` or empty `Or`).
    #[error("empty path pattern")]
    PathEmptyPattern,

    /// A `Times` with `from > to` (an empty repetition range).
    #[error("Path `Times` range is empty (from > to)")]
    PathTimesInvalidRange,

    // ---- Transitive-closure (`Star`/`Plus`) property paths ----
    /// A `Star`/`Plus` path that is NOT under an endpoint `Distinct`. The reference emits an
    /// edge-simple BAG of endpoints; a set closure equals that bag only once a surrounding `Distinct`
    /// (or `Count`) collapses the multiplicity. A bare `star`/`plus` would ship bag semantics as a set,
    /// so it rejects closed until the collapsing gate is present.
    #[error("`Star`/`Plus` path is provable only under an endpoint `Distinct` (bare closure would ship a set for a bag); reject closed")]
    ClosureRequiresDistinct,

    /// A `Star`/`Plus` path with BOTH endpoints free (`path(?x, star(p), ?y)`). This is the identity
    /// over the active domain, needing a domain-authenticated diagonal that this pass does not model.
    #[error("`Star`/`Plus` path with two free endpoints requires a domain-authenticated diagonal (deferred); reject closed")]
    ClosureBothEndpointsFree,

    /// A `Star`/`Plus` path with BOTH endpoints bound (`path(a, star(p), b)`). This is a boolean
    /// reachability check, not a set query; it is a deferred follow-up.
    #[error("`Star`/`Plus` path with two bound endpoints is a boolean reachability check (deferred); reject closed")]
    ClosureBothEndpointsBound,

    /// A `Star`/`Plus` over a COMPOUND sub-pattern (a sequence, union, or nested repetition rather than
    /// a single predicate). Multi-atom closure bodies need the general BGP-in-the-loop lowering, a
    /// deferred follow-up (B2).
    #[error("`Star`/`Plus` over a compound sub-pattern (only a single predicate is provable) is deferred; reject closed")]
    ClosureCompoundSubpattern,

    /// A `Count`-gated `Star`/`Plus` closure. The reachable-set cardinality is provable, but the
    /// `Count` integration is a follow-up; do the `Distinct` form first.
    #[error("`Count`-gated `Star`/`Plus` closure is a follow-up (do the `Distinct` form first); reject closed")]
    ClosureCountGateDeferred,

    /// A `Subsumption ?child << ?parent` with BOTH endpoints FREE. Reference `class_subsumed`
    /// with two unbound classes enumerates the reflexive-transitive `sys:inherits` relation over the
    /// WHOLE schema — a two-column pairs enumeration that needs a domain-authenticated diagonal /
    /// pairs Distinct this pass does not model (mirrors [`ClosureBothEndpointsFree`]). Reject closed.
    #[error("`Subsumption` with two free class endpoints needs a domain-authenticated pairs enumeration (deferred); reject closed")]
    SubsumptionBothEndpointsFree,

    /// A `Subsumption` endpoint that is neither a class-IRI node nor a variable (e.g. it parsed to an
    /// unexpected `NodeValue`). Subsumption ranges over CLASS nodes only.
    #[error("`Subsumption` endpoints must each be a class IRI or a variable")]
    SubsumptionEndpointNotClass,

    // ---- Triple-SHAPED constructs that are NOT store BGP triples (`TypeOf`/`Dot`/`IsA`) ----
    // These three woql2 leaves resemble a triple but do NOT denote a store triple pattern in
    // TerminusDB's query semantics (verified against the Prolog reference — citations per
    // variant). Lowering any of them to a BGP `Triple` would authenticate a relation the
    // engine never computes, so each rejects closed rather than emit an unsound plan. Exact
    // class filtering by stored type remains available as a plain `Triple(?x, <rdf:type>,
    // Class)`; these operators are simply not that.

    /// A `Query::TypeOf`. WOQL `typeof/2` is a value-SHAPE classifier, not a stored-class
    /// lookup: a node/document IRI yields `sys#Top` UNCONDITIONALLY (it never reads the
    /// instance's `rdf:type`), a typed literal yields its datatype IRI, a dictionary yields
    /// `sys#Dictionary`, and a language-tagged literal yields its tag as `xsd:string`
    /// (terminusdb `src/core/query/woql_compile.pl`, runtime `typeof/2`; `IsA`/`TypeOf`
    /// both parsed in `src/core/query/json_woql.pl`). Lowering `TypeOf(v, t)` to
    /// `t(v, <rdf:type>, t)` would authenticate the `rdf:type` relation — a DIFFERENT
    /// relation than `typeof` computes (e.g. it would bind an instance's concrete class
    /// where `typeof` binds `sys#Top`). Rejected closed.
    #[error(
        "TypeOf is a value-shape classifier (node→sys#Top, typed literal→datatype IRI), not a stored-class lookup: lowering it to an rdf:type triple would authenticate a different relation"
    )]
    TypeOfNotStoredType,

    /// A `Query::Dot`. WOQL `dot/3` is IN-MEMORY dictionary field access on an ALREADY-BOUND
    /// document value — it reads NO store triples (terminusdb
    /// `src/core/query/woql_compile.pl`: `dot` compiles to `dot_field_unwrap` → `dot_field`,
    /// which is `get_dict` over a resolved dictionary, and `find_resources(dot(_,_,_),…)` is
    /// the empty set; the woql2 AST doc for `Dot` itself says "a bound document"). Lowering
    /// `Dot(doc, field, value)` to a store edge `t(doc, field, value)` would authenticate a
    /// graph-edge relation the operator never queries (different subject semantics, different
    /// `get_dict` field-name resolution). Rejected closed.
    #[error(
        "Dot is in-memory dictionary field access on a bound document (reads no store triples), not a graph edge: lowering it to a store triple would authenticate a different relation"
    )]
    DotNotStoreEdge,

    /// A `Query::IsA` with a BOUND class (the node regime). WOQL `isa/2` compiles to `is_instance`,
    /// whose node regime (`is_instance_class`, terminusdb `src/core/document/instance.pl:80-92`) is
    /// SUBSUMPTION-closed: `X isa C` holds iff `X` has a DIRECT stored `rdf:type Class` where
    /// `class_subsumed(Class, C)` — i.e. `Class` lies in the DOWNWARD `sys:inherits` closure of `C`
    /// (`C` together with its subclasses; the subclass edge is `sys:inherits`,
    /// `http://terminusdb.com/schema/sys#inherits`, not `rdfs:subClassOf`). A bare direct
    /// `t(element, <rdf:type>, C)` triple is INCOMPLETE — it misses every instance whose direct type is
    /// a proper subclass of `C`.
    ///
    /// The schema-graph foundation now EXISTS (`Subsumption` commits and closes the `sys:inherits`
    /// hierarchy over the SCHEMA graph), so the blocker is no longer "the schema graph is uncommitted".
    /// The blocker is a CROSS-DICTIONARY reconciliation. The direct `rdf:type Class` relation is read
    /// from the INSTANCE graph (its `Class` values are INSTANCE-dictionary ids), while the downward
    /// subsumption closure of `C` is computed over the SCHEMA graph (its class values are
    /// SCHEMA-dictionary ids). The two graphs are SEPARATE committed layers with SEPARATE dictionaries
    /// under SEPARATE `commitment_root`s (combined only by the two-hash `database_root`), so the SAME
    /// class IRI generally has DIFFERENT ids in the two dicts, and two unrelated IRIs can share an id.
    /// A raw dict-id join/equality between the instance `Class` column and the schema closure column is
    /// therefore BOTH unsound (false matches from colliding ids) AND incomplete (missed matches for the
    /// same IRI's differing ids). Every match must instead be authenticated as
    /// `instance_rdf_type_class_IRI == a_closure_class_IRI`, with the instance-side dict membership
    /// (`id → IRI`) anchored to the INSTANCE root and the schema-side dict membership (`closure id → IRI`)
    /// anchored to the SCHEMA root, both bound under the trusted `database_root`.
    ///
    /// No such primitive exists yet, and it is not a thin layer over the audited gadgets. The proof
    /// recursion anchors an ENTIRE relation to a SINGLE layer + `commitment_root` — `verify_relation`
    /// and the `InnerJoin` arm thread one `expected_root` and verify BOTH join children against the SAME
    /// layer, and [`relation_graph`] rejects a relation that mixes instance and schema leaves. The store
    /// commits `(hash(IRI), id)` dict-membership PAIRS per layer (`prove_dict_membership`) but exposes
    /// no in-circuit `id → hash(IRI)` JOINABLE column and no cross-root join binding BOTH roots.
    /// Building the node-regime IsA soundly needs that NEW cross-dict IRI-reconciled join/membership
    /// primitive: a multi-root relation carrying an authenticated `id → hash(IRI)` column on each side,
    /// joined on the shared IRI-hash under both roots. Deferred; rejected closed rather than ship an
    /// unsound raw-id join.
    #[error(
        "IsA in an unsupported position/shape: a standalone `?x isa C` (variable element, BOUND class) NOW SHIPS as a cross-dictionary two-root hash join (downward sys:inherits closure of C over the schema graph, hash-joined against the instance rdf:type relation via a full-field LogUp semi-join). This variant is the residual reject: a NESTED IsA inside a BGP conjunction (the cross-root node is top-level only), a BOUND-ELEMENT ground existence check `<n> isa C`, and a Distinct-gated IsA are all deferred; reject closed"
    )]
    IsASubsumptionUnsupported,

    /// A `Query::IsA` with an UNBOUND/variable class (`type_of` is a variable). The node regime's
    /// downward `sys:inherits` subsumption closure (see [`Self::IsASubsumptionUnsupported`]) must be
    /// SEEDED by a bound class constant to enumerate `{C} ∪ subclasses(C)`; a free class instead
    /// requires enumerating EVERY class in the schema together with its per-class closure — a
    /// whole-schema enumeration (the mirror of [`Self::SubsumptionBothEndpointsFree`]). Deferred;
    /// rejected closed. (This case rejects on the free class independently of the cross-dict blocker.)
    #[error(
        "IsA with an unbound/variable class: the downward sys:inherits subsumption closure needs the class bound as a constant to seed it (a whole-schema class enumeration is deferred); reject closed"
    )]
    IsAVariableClass,

    /// A malformed `Eval` composition: not a conjunction of an operand-binding child BGP with
    /// exactly ONE `Eval`, or the child carried a projection/count/distinct wrapper the
    /// per-row derived-column layer does not model.
    #[error("Eval must be a conjunction of exactly one Eval with a non-empty operand-binding child BGP (no projection/count/distinct on the child)")]
    EvalUnsupportedShape,

    /// `Eval`'s `result_value` was a numeric literal rather than a fresh result variable.
    #[error("Eval result must be a variable")]
    EvalResultNotVariable,

    /// `Eval`'s result variable is already bound by the child BGP; the derived column must be
    /// a fresh output column, never a redefinition of an authenticated binding.
    #[error("Eval result variable `?{var}` is already bound by the operand-binding BGP")]
    EvalResultAlreadyBound { var: String },

    /// An out-of-scope arithmetic operator. `Divide` (`/`/`rdiv`) is EXACT RATIONAL and `Floor`
    /// over rationals both need the rational value layer; a STATIC negative `Exp` exponent is a
    /// rational result. All reject closed pending the rational layer. (Integer `Plus`/`Minus`/
    /// `Times`/`Div`/`Exp`-with-nonneg-exponent ARE provable via verifier recompute.)
    #[error("Eval operator `{operator}` is unsupported: only integer Plus/Minus/Times/Div/Exp (non-negative exponent) are provable")]
    EvalUnsupportedOperator { operator: &'static str },

    /// An `Eval` operand variable is not bound by any store read in the same query, so its
    /// value cannot be authenticated. Arithmetic reads no store itself; an operand value must
    /// come from an authenticated store-derived binding.
    #[error("Eval operand `?{var}` is not bound by a store read in the same query: it cannot be authenticated")]
    EvalOperandNotBound { var: String },

    /// An `Eval` operand variable IS bound by the child, but not as a nonnullable, exact
    /// fixed-width INTEGER value (e.g. it is a node, a predicate, nullable, a float/decimal, or
    /// an unconstrained value whose datatype is not pinned by a range comparison). Only the
    /// order-preserving fixed-width integer datatypes the value-auth path supports are allowed.
    #[error("Eval operand `?{var}` is not a nonnullable exact fixed-width integer store value")]
    EvalOperandNotInteger { var: String },

    /// An `Eval` literal operand is not an integer (a float, decimal, string, boolean, or other
    /// non-integer XSD datatype). Only exact integer literals are provable in v1.
    #[error("Eval literal operand of datatype `{datatype}` is unsupported: only integer literals are provable in v1")]
    EvalUnsupportedLiteral { datatype: &'static str },

    /// An exact-rational-layer (`Floor`/`Divide`/decimal) operand variable IS bound by the child,
    /// but not as a nonnullable node/value STORE value (it is a node IRI, predicate, or nullable
    /// Optional column). Only a store-authenticated numeric operand — an integer family or
    /// `xsd:decimal` value, resolved at recompute — is provable; anything else rejects closed.
    #[error("decimal-layer operand `?{var}` is not a nonnullable node/value store value (numeric integer/xsd:decimal)")]
    EvalOperandNotNumeric { var: String },

    /// A WOQL `Divide` (`rdiv`) has a STATIC zero constant divisor. Prolog `rdiv`/`is` throws
    /// `evaluation_error(zero_divisor)`, so the query fails; reject closed at compile time (a
    /// runtime/column zero divisor fails closed in the recompute instead).
    #[error("Divide by a static zero divisor: Prolog `rdiv` throws, so the query fails closed")]
    EvalDivideByZero,

    /// A malformed `Typecast` composition: not a conjunction of exactly ONE `Typecast` with a
    /// non-empty source-binding child BGP (no projection/count/distinct on the child).
    #[error("Typecast must be a conjunction of exactly one Typecast with a non-empty source-binding child BGP (no projection/count/distinct on the child)")]
    TypecastUnsupportedShape,

    /// `Typecast`'s `value` (the source) is not a variable, so it is not an authenticated store
    /// read. Only a store-bound source column can be verifier-recomputed.
    #[error("Typecast source must be a variable bound by a store read")]
    TypecastSourceNotVariable,

    /// `Typecast`'s source variable is not bound by any store read in the same query, so its
    /// value cannot be authenticated.
    #[error("Typecast source `?{var}` is not bound by a store read in the same query: it cannot be authenticated")]
    TypecastSourceNotBound { var: String },

    /// `Typecast`'s source variable IS bound, but not as a nonnullable, exact fixed-width INTEGER
    /// store value (e.g. a node, predicate, nullable, float/decimal, or a value whose datatype is
    /// not pinned by a range comparison). Only fixed-width integer sources are provable.
    #[error("Typecast source `?{var}` is not a nonnullable exact fixed-width integer store value")]
    TypecastSourceNotInteger { var: String },

    /// A NUMBER↔STRING `Typecast`'s source variable IS bound, but not as a nonnullable node/value
    /// STORE value column (it is a node IRI, predicate, nullable Optional, or generated column), so
    /// its per-row value cannot be authenticated and cast. Reject closed.
    #[error("Typecast source `?{var}` is not a nonnullable store value column (number↔string cast)")]
    TypecastValueSourceInvalid { var: String },

    /// `Typecast`'s `result` is not a variable, or is already bound by the source BGP; the cast
    /// column must be a fresh output column, never a literal or a redefinition of a binding.
    #[error("Typecast result must be a fresh variable not already bound by the source BGP")]
    TypecastResultInvalid,

    /// `Typecast`'s target `type` is not a concrete node IRI (it is a variable), so the target
    /// type is unknown at compile time. Reject closed.
    #[error("Typecast target type must be a concrete node IRI, not a variable")]
    TypecastTargetNotNode,

    /// `Typecast`'s target type IRI is not one of the supported fixed-width integer types
    /// (xsd:byte/short/int/long and their unsigned forms). A string/float/decimal target, or an
    /// arbitrary-precision `xsd:integer`, needs a different value layer. Reject closed.
    #[error("Typecast target type `{iri}` is unsupported: only fixed-width integer→integer casts are provable")]
    TypecastUnsupportedTarget { iri: String },

    /// A malformed `Concatenate` composition: not a conjunction of exactly ONE `Concatenate`
    /// with a non-empty operand-binding child BGP (no projection/count/distinct on the child).
    #[error("Concat must be a conjunction of exactly one Concatenate with a non-empty operand-binding child BGP (no projection/count/distinct on the child)")]
    ConcatUnsupportedShape,

    /// `Concatenate`'s `result` (`result_string`) is not a variable (it is a literal/list), so
    /// there is no fresh output variable to bind the synthesized string to.
    #[error("Concat result must be a variable")]
    ConcatResultNotVariable,

    /// `Concatenate`'s result variable is already bound by the child BGP; the synthesized string
    /// must be a fresh output, never a redefinition of an authenticated binding.
    #[error("Concat result variable `?{var}` is already bound by the operand-binding BGP")]
    ConcatResultAlreadyBound { var: String },

    /// `Concatenate`'s list is a runtime list VARIABLE, not a concrete inline list. v1 needs the
    /// element sequence known at compile time to pin string-ness/order into the query digest.
    #[error("Concat list must be a concrete inline list, not a runtime list variable")]
    ConcatListNotConcrete,

    /// `Concatenate`'s list is empty. `interpolate_string([], "")` has no per-row operand to
    /// authenticate; it is outside the per-row provable fragment.
    #[error("Concat list must be non-empty")]
    ConcatEmptyList,

    /// A `Concatenate` operand VARIABLE is not bound by any store read in the same query, so its
    /// value cannot be authenticated.
    #[error("Concat operand `?{var}` is not bound by a store read in the same query: it cannot be authenticated")]
    ConcatOperandNotBound { var: String },

    /// A `Concatenate` operand variable IS bound, but not as a nonnullable node/value store value,
    /// or it is pinned to a NON-string datatype. Only a store-authenticated xsd:string operand is
    /// provable in v1 (a non-string would need SWI's stringification, a separate circuit).
    #[error("Concat operand `?{var}` is not a nonnullable store-authenticated xsd:string value")]
    ConcatOperandNotString { var: String },

    /// A `Concatenate` list element is a NON-string scalar literal (a number/boolean/…). SWI
    /// `interpolate_string` would stringify it; that rendering is a separate circuit. Reject closed.
    #[error("Concat literal element is not an xsd:string: non-string element stringification is unsupported in v1")]
    ConcatLiteralNotString,

    /// A `Concatenate` list element is itself a nested list, not a scalar string operand.
    #[error("Concat list element must be a scalar string, not a nested list")]
    ConcatNestedList,

    // ----- WOQL `Join` (separator-interposed concat) -----
    /// A malformed `Join` composition: not a conjunction of exactly ONE `Join` with a non-empty
    /// operand-binding child BGP (no projection/count/distinct on the child).
    #[error("Join must be a conjunction of exactly one Join with a non-empty operand-binding child BGP (no projection/count/distinct on the child)")]
    JoinUnsupportedShape,

    /// `Join`'s `result` (`result_string`) is not a variable (it is a literal/list); the
    /// synthesized string binds a FRESH variable.
    #[error("Join result must be a variable")]
    JoinResultNotVariable,

    /// `Join`'s result variable is already bound by the child BGP; it must be fresh.
    #[error("Join result variable `?{var}` is already bound by the operand-binding BGP")]
    JoinResultAlreadyBound { var: String },

    /// `Join`'s list is a runtime list VARIABLE, not a concrete inline list.
    #[error("Join list must be a concrete inline list, not a runtime list variable")]
    JoinListNotConcrete,

    /// `Join`'s list is empty. `join([], Sep, "")` has no per-row operand to authenticate.
    #[error("Join list must be non-empty")]
    JoinEmptyList,

    // ----- WOQL `Substring` (per-row constant-index character substring) -----
    /// A malformed `Substring` composition: not a conjunction of exactly ONE `Substring` with a
    /// non-empty operand-binding child BGP (no projection/count/distinct on the child).
    #[error("Substring must be a conjunction of exactly one Substring with a non-empty operand-binding child BGP (no projection/count/distinct on the child)")]
    SubstringUnsupportedShape,

    /// `Substring`'s `String` operand is not a variable (it is a literal/list); the task requires
    /// an AUTHENTICATED store xsd:string input, so a constant string rejects closed.
    #[error("Substring input string must be a variable bound to a store-authenticated xsd:string")]
    SubstringStringNotVariable,

    /// `Substring`'s `String` variable is not bound by any store read in the same query.
    #[error("Substring input string `?{var}` is not bound by a store read in the same query: it cannot be authenticated")]
    SubstringStringNotBound { var: String },

    /// `Substring`'s `String` variable IS bound, but not as a nonnullable node/value store value,
    /// or it is pinned to a NON-string datatype.
    #[error("Substring input string `?{var}` is not a nonnullable store-authenticated xsd:string value")]
    SubstringStringNotAuthenticated { var: String },

    /// `Substring`'s `Before`/`Length` is not an integer LITERAL (e.g. a variable index — a
    /// per-row store-authenticated index is a deferred follow-up).
    #[error("Substring before/length must be a non-negative integer literal (variable indices are a deferred follow-up)")]
    SubstringIndexNotLiteral,

    /// `Substring`'s `Before`/`Length` literal is not an exact integer (a float/decimal/other).
    #[error("Substring before/length must be an integer literal")]
    SubstringIndexNotInteger,

    /// `Substring`'s `Before`/`Length` literal is NEGATIVE. SWI `sub_string/5` requires
    /// non-negative offsets (a negative one fails/throws) — reject closed, matching the reference.
    #[error("Substring before/length must be non-negative (SWI sub_string fails/throws on a negative offset)")]
    SubstringNegativeIndex,

    /// `Substring`'s `Before`/`Length` literal does not fit `usize`.
    #[error("Substring before/length literal is too large")]
    SubstringIndexTooLarge,

    /// `Substring`'s `After` is not a fresh output variable (it is a constant/list, which would add
    /// a length-equality filter, or it rebinds an existing column).
    #[error("Substring after must be a fresh output variable (a bound After adds an unmodeled length/occurrence filter)")]
    SubstringAfterNotVariable,

    /// `Substring`'s `SubString` (result) is not a variable (it is a literal/list); the synthesized
    /// substring binds a FRESH variable.
    #[error("Substring result must be a variable")]
    SubstringResultNotVariable,

    /// `Substring`'s `After` or `SubString` result variable is already bound by the child BGP; it
    /// must be fresh (a rebinding would be an unmodeled join/filter).
    #[error("Substring after/result variable `?{var}` is already bound by the operand-binding BGP")]
    SubstringResultAlreadyBound { var: String },

    /// `Substring`'s `SubString` result variable aliases its `After` variable (an integer output
    /// unified with a string output — the reference would fail on the type mismatch). Reject closed.
    #[error("Substring result variable must differ from its after variable")]
    SubstringResultAliasesAfter,

    /// A DEFERRED string-case op (`Upper`/`Lower`) appeared. Their SWI `string_upper`/
    /// `string_lower` fold case via LOCALE-DEPENDENT `towupper`/`towlower`; TerminusDB's Docker
    /// deployment pins no `LC_CTYPE`, so under a Turkic locale even ASCII 'i'/'I' diverge from an
    /// ASCII case map. Fidelity is unprovable across deployments, so reject closed (see
    /// `reject_unsupported_string_case_ops`) pending a locale-pinning follow-up.
    #[error("Upper/Lower are unsupported in v1: SWI string_upper/string_lower are locale-dependent (unpinned LC_CTYPE), so ASCII fidelity is not provable — deferred")]
    StringCaseOpLocaleUnsupported,

    /// `Pad` is not a conjunction of exactly one `Pad` with a non-empty operand-binding child BGP
    /// (or the child carries a projection/count/distinct we cannot layer a pad over).
    #[error("Pad must be a conjunction of exactly one Pad with a non-empty operand-binding child BGP (no projection/count/distinct on the child)")]
    PadUnsupportedShape,

    /// `Pad`'s string operand is not a variable (a literal is a constant with no per-row read
    /// tying it to the trusted root; the task requires an AUTHENTICATED xsd:string input).
    #[error("Pad string operand must be a variable bound by a store read in the same query")]
    PadStringNotVariable,

    /// `Pad`'s string operand variable is not bound by a store read in the same query.
    #[error("Pad string operand `?{var}` is not bound by a store read in the same query: it cannot be authenticated")]
    PadStringNotBound { var: String },

    /// `Pad`'s string operand is not a nonnullable store-authenticated xsd:string value.
    #[error("Pad string operand `?{var}` is not a nonnullable store-authenticated xsd:string value")]
    PadStringNotAuthenticated { var: String },

    /// `Pad`'s pad char is not an xsd:string LITERAL (a variable pad char is a follow-up; a
    /// non-string is unsupported).
    #[error("Pad char must be an xsd:string literal (a variable pad char is a deferred follow-up)")]
    PadCharNotLiteral,

    /// `Pad`'s pad char literal is not an xsd:string.
    #[error("Pad char literal must be an xsd:string")]
    PadCharNotString,

    /// `Pad`'s pad char is not exactly ONE code point (reference `utils:pad` throws
    /// `pad_not_length_one` unless `string_length == 1`).
    #[error("Pad char must be exactly one character (code point): reference utils:pad requires string_length == 1")]
    PadCharNotSingleChar,

    /// `Pad`'s target length is not an integer LITERAL (a variable target length is a follow-up).
    #[error("Pad target length must be an integer literal (a variable target length is a deferred follow-up)")]
    PadLengthNotLiteral,

    /// `Pad`'s target-length literal is not an integer.
    #[error("Pad target length literal must be an integer")]
    PadLengthNotInteger,

    /// `Pad`'s target length is negative (`L < 0` always throws `bad_pad_length` in the reference).
    #[error("Pad target length must be non-negative")]
    PadNegativeLength,

    /// `Pad`'s target length exceeds `usize`.
    #[error("Pad target length exceeds the platform usize")]
    PadLengthTooLarge,

    /// `Pad`'s result is not a variable.
    #[error("Pad result must be a variable")]
    PadResultNotVariable,

    /// `Pad`'s result variable is already bound by the child BGP; it must be fresh.
    #[error("Pad result variable `?{var}` is already bound by the operand-binding BGP")]
    PadResultAlreadyBound { var: String },

    /// `HashKey`/`hash` is not a conjunction of exactly one `HashKey` with a non-empty
    /// operand-binding child BGP (or the child carries a projection/count/distinct).
    #[error("HashKey must be a conjunction of exactly one HashKey with a non-empty operand-binding child BGP (no projection/count/distinct on the child)")]
    HashKeyUnsupportedShape,

    /// `HashKey`/`hash`'s result URI is not a variable (a constant node result would add an
    /// equality constraint we do not model).
    #[error("HashKey result URI must be a variable")]
    HashKeyResultNotVariable,

    /// `HashKey`/`hash`'s result variable is already bound by the child BGP; it must be fresh.
    #[error("HashKey result variable `?{var}` is already bound by the operand-binding BGP")]
    HashKeyResultAlreadyBound { var: String },

    // ----- WOQL single-input CALENDAR ops (Weekday/WeekdaySundayStart/IsoWeek/Day*/Month*) -----
    /// A malformed calendar composition: not a conjunction of exactly ONE calendar op with a
    /// non-empty operand-binding child BGP (no projection/count/distinct on the child).
    #[error("a calendar op must be a conjunction of exactly one calendar op with a non-empty operand-binding child BGP (no projection/count/distinct on the child)")]
    CalendarUnsupportedShape,

    /// A two-input / interval / duration / range-generator calendar op (`DateDuration`, `Interval`,
    /// `IntervalStartDuration`, `IntervalDurationEnd`, `IntervalRelation`, `IntervalRelationTyped`,
    /// the 3-arg `MonthStartDates`/`MonthEndDates`). These take multiple bound inputs and/or generate
    /// multiple rows; they are heavier follow-ups and reject closed rather than being approximated.
    #[error("calendar op `{op}` (two-input / interval / duration / range-generator) is a deferred follow-up: reject closed")]
    CalendarTwoInputUnsupported { op: &'static str },

    /// A single-input calendar op's operand is not a VARIABLE (it is a literal/list), so there is no
    /// per-row store-authenticated operand to recompute over.
    #[error("calendar op operand must be a variable bound by a store read")]
    CalendarOperandNotVariable,

    /// A calendar op's operand variable is not bound by any store read in the same query, so its
    /// value cannot be authenticated.
    #[error("calendar op operand `?{var}` is not bound by a store read in the same query: it cannot be authenticated")]
    CalendarOperandNotBound { var: String },

    /// A date-input calendar op's operand variable IS bound, but not as a nonnullable node/value store
    /// value, or it is pinned to a NON-`xsd:date` datatype. Only a store-authenticated `xsd:date`
    /// operand is provable (an `xsd:dateTime` operand is reject-closed — see the calendar layer notes).
    #[error("calendar op operand `?{var}` is not a nonnullable store-authenticated xsd:date value")]
    CalendarOperandNotDate { var: String },

    /// A month-boundary calendar op's operand variable IS bound, but not as a nonnullable node/value
    /// store value, or it is pinned to a NON-`xsd:gYearMonth` datatype. Only a store-authenticated
    /// `xsd:gYearMonth` operand is provable for `MonthStartDate`/`MonthEndDate`.
    #[error("calendar op operand `?{var}` is not a nonnullable store-authenticated xsd:gYearMonth value")]
    CalendarOperandNotYearMonth { var: String },

    /// A calendar op's result field is not a fresh output VARIABLE (it is a literal — i.e. the
    /// reference's validate mode). Only a fresh variable the op binds is provable; a validate-mode
    /// comparison against a bound value is out of the per-row derived-output fragment.
    #[error("calendar op result must be a fresh output variable, not a literal (validate mode is reject-closed)")]
    CalendarResultNotVariable,

    /// A calendar op's result variable is already bound by the child BGP (or, for `IsoWeek`, the two
    /// result variables collide); each derived output must be a fresh, distinct output column.
    #[error("calendar op result variable `?{var}` is already bound by the operand-binding BGP or collides with another result")]
    CalendarResultAlreadyBound { var: String },
}

/// The name of a `Query` variant, for descriptive rejection messages. Kept tiny and
/// covering the constructs a caller is most likely to hit.
fn query_variant_name(q: &Query) -> &'static str {
    match q {
        Query::And(_) => "And",
        Query::Or(_) => "Or",
        Query::Not(_) => "Not",
        Query::True(_) => "True",
        Query::Triple(_) => "Triple",
        Query::AddedTriple(_) => "AddedTriple",
        Query::DeletedTriple(_) => "DeletedTriple",
        Query::AddTriple(_) => "AddTriple",
        Query::DeleteTriple(_) => "DeleteTriple",
        Query::Link(_) => "Link",
        Query::Data(_) => "Data",
        Query::Eval(_) => "Eval",
        Query::Path(_) => "Path",
        Query::Equals(_) => "Equals",
        Query::Less(_) => "Less",
        Query::Greater(_) => "Greater",
        Query::Gte(_) => "Gte",
        Query::Lte(_) => "Lte",
        Query::Select(_) => "Select",
        Query::Distinct(_) => "Distinct",
        Query::If(_) => "When",
        Query::WoqlOptional(_) => "Optional",
        Query::Once(_) => "Once",
        Query::Immediately(_) => "Immediately",
        Query::OrderBy(_) => "OrderBy",
        Query::GroupBy(_) => "GroupBy",
        Query::Limit(_) => "Limit",
        Query::Start(_) => "Start",
        Query::Count(_) => "Count",
        Query::TripleCount(_) => "TripleCount",
        Query::Size(_) => "Size",
        Query::InRange(_) => "InRange",
        Query::TripleSlice(_) => "TripleSlice",
        Query::TripleSliceRev(_) => "TripleSliceRev",
        Query::TripleNext(_) => "TripleNext",
        Query::TriplePrevious(_) => "TriplePrevious",
        Query::Sum(_) => "Sum",
        Query::RangeMin(_) => "RangeMin",
        Query::RangeMax(_) => "RangeMax",
        Query::Length(_) => "Length",
        Query::Member(_) => "Member",
        Query::Collect(_) => "Collect",
        Query::Get(_) => "Get",
        _ => "unsupported",
    }
}

/// A per-atom relation, ready for the variable-arity planner. `layer_atom` is the
/// variable-blind atom emitted into the plan (subject/predicate/object terms). `kept`
/// is the atom's VARIABLE names in S,P,O order — the relation's output columns, one per
/// variable. An all-variable atom keeps two columns `[subj_var, obj_var]`; a
/// subject/object-constant atom keeps one. The planner joins on shared `kept` names and
/// `rel_key` indexes into `kept`.
struct Atom {
    layer_atom: LayerBgpAtom,
    kept: Vec<String>,
    /// The subject variable name, if the subject is a variable (else the subject is a
    /// constant). Used to route comparison conjuncts to the right object-binding atom.
    subj_var: Option<String>,
    /// Predicate variables use the independent predicate dictionary ID namespace.
    pred_var: Option<String>,
    /// The object variable name, if the object is a variable (else a constant). A numeric
    /// range attaches to the unique atom whose `obj_var` is the compared variable.
    obj_var: Option<String>,
}

#[derive(Clone)]
struct TypedTriple {
    triple: Triple,
    object_kind: ObjectKindFilter,
    /// Which committed column set this atom reads. A plain `Triple`/`Link`/`Data` reads the
    /// materialized head view; a WOQL `AddedTriple`/`DeletedTriple` reads this layer's own
    /// addition/removal delta file. Threaded per-atom onto [`LayerBgpAtom::source`] (via
    /// [`atom_from_triple`]), so a BGP leaf MAY mix sources — a cross-source join. The plan
    /// DEFAULT is [`plan_default_column_source`] (the uniform source, else materialized).
    source: ColumnSource,
}

fn plan_group_extrema_form(query: &Query) -> Result<Option<CompiledBgp>, BgpPlanError> {
    let conjuncts = match query {
        Query::And(and) if and.and.len() == 2 => &and.and,
        _ => return Ok(None),
    };
    let (group, range, kind) = match (&conjuncts[0], &conjuncts[1]) {
        (Query::GroupBy(group), Query::RangeMin(range)) => {
            (group, (&range.list, &range.result), GroupExtremaKind::Min)
        }
        (Query::GroupBy(group), Query::RangeMax(range)) => {
            (group, (&range.list, &range.result), GroupExtremaKind::Max)
        }
        _ if conjuncts.iter().any(|query| matches!(query, Query::GroupBy(_) | Query::RangeMin(_) | Query::RangeMax(_))) => {
            return Err(BgpPlanError::GroupExtremaUnsupportedShape)
        }
        _ => return Ok(None),
    };
    if group.group_by.len() > 2
        || (group.group_by.len() == 2 && group.group_by[0] == group.group_by[1])
    {
        return Err(BgpPlanError::GroupExtremaUnsupportedShape);
    }
    let template = match &group.template {
        Value::Variable(v) => v.clone(),
        _ => return Err(BgpPlanError::GroupExtremaUnsupportedValue),
    };
    let hidden = match &group.grouped_value {
        Value::Variable(v) => v,
        _ => return Err(BgpPlanError::GroupExtremaUnsupportedShape),
    };
    let list = match range.0 {
        DataValue::Variable(v) => v,
        _ => return Err(BgpPlanError::GroupExtremaUnsupportedShape),
    };
    let result = match range.1 {
        DataValue::Variable(v) => v.clone(),
        _ => return Err(BgpPlanError::GroupExtremaUnsupportedShape),
    };
    if hidden != list
        || hidden == &result
        || template == *hidden
        || template == result
        || group.group_by.iter().any(|key| key == hidden || key == &result || key == &template)
    {
        return Err(BgpPlanError::GroupExtremaUnsupportedShape);
    }
    let child = plan_bgp(&group.query)?;
    if child.projection.is_some() || child.count_variable.is_some() || child.full_row_distinct {
        return Err(BgpPlanError::GroupExtremaUnsupportedShape);
    }
    if child.relation.schema.iter().any(|variable| variable == hidden || variable == &result) {
        return Err(BgpPlanError::GroupExtremaUnsupportedShape);
    }
    let key_columns = group.group_by.iter().map(|key| {
        child.relation.schema.iter().position(|variable| variable == key)
            .ok_or(BgpPlanError::GroupExtremaUnsupportedShape)
    }).collect::<Result<Vec<_>, _>>()?;
    let value_column = child
        .relation
        .schema
        .iter()
        .position(|v| v == &template)
        .ok_or(BgpPlanError::GroupExtremaUnsupportedValue)?;
    if key_columns.iter().any(|key_column| *key_column == value_column) {
        return Err(BgpPlanError::GroupExtremaUnsupportedShape);
    }
    let descriptors = relation_result_descriptors(&child.relation)
        .map_err(|_| BgpPlanError::GroupExtremaUnsupportedShape)?;
    if key_columns.iter().any(|key_column| {
        child.relation.domains.get(*key_column).copied().flatten()
            != Some(EqualityValueDomain::Node)
            || descriptors[*key_column].nullable
            || descriptors[*key_column].namespace != ResultIdNamespace::NodeValue
    }) || descriptors[value_column].nullable
        || descriptors[value_column].namespace != ResultIdNamespace::NodeValue
    {
        return Err(BgpPlanError::GroupExtremaUnsupportedShape);
    }
    let datatype = match child.relation.domains.get(value_column).copied().flatten() {
        Some(EqualityValueDomain::Datatype(
            datatype @ (CanonicalDatatype::UInt8
            | CanonicalDatatype::UInt16
            | CanonicalDatatype::UInt32
            | CanonicalDatatype::UInt64
            | CanonicalDatatype::Int8
            | CanonicalDatatype::Int16
            | CanonicalDatatype::Int32
            | CanonicalDatatype::Int64),
        )) => datatype,
        _ => return Err(BgpPlanError::GroupExtremaUnsupportedValue),
    };
    let collection = child.collection;
    let mut schema = group.group_by.clone();
    schema.push(result);
    let mut domains = key_columns.iter().map(|key_column| child.relation.domains[*key_column])
        .collect::<Vec<_>>();
    domains.push(Some(EqualityValueDomain::Datatype(datatype)));
    Ok(Some(CompiledBgp {
        relation: CompiledRelation {
            schema,
            domains,
            statement: CompiledRelationStatement::GroupExtrema {
                input: Box::new(child.relation),
                key_columns,
                value_column,
                datatype,
                kind,
            },
        },
        projection: None,
        collection,
        count_variable: None,
        full_row_distinct: false,
    }))
}

fn plan_collection_extrema_form(query: &Query) -> Result<Option<CompiledBgp>, BgpPlanError> {
    let conjuncts = match query {
        Query::And(and) if and.and.len() == 2 => &and.and,
        _ => return Ok(None),
    };
    let (collect, range, kind) = match (&conjuncts[0], &conjuncts[1]) {
        (Query::Collect(collect), Query::RangeMin(range)) => {
            (collect, (&range.list, &range.result), GroupExtremaKind::Min)
        }
        (Query::Collect(collect), Query::RangeMax(range)) => {
            (collect, (&range.list, &range.result), GroupExtremaKind::Max)
        }
        // GroupBy extrema has its own planner and remains reachable.
        _ if conjuncts.iter().any(|query| matches!(query, Query::GroupBy(_)))
            && conjuncts.iter().any(|query| matches!(query, Query::RangeMin(_) | Query::RangeMax(_))) =>
        {
            return Ok(None)
        }
        _ if conjuncts
            .iter()
            .any(|query| matches!(query, Query::RangeMin(_) | Query::RangeMax(_))) =>
        {
            return Err(BgpPlanError::CollectionExtremaUnsupportedShape)
        }
        _ => return Ok(None),
    };
    let template = match &collect.template {
        Value::Variable(variable) => variable,
        _ => return Err(BgpPlanError::CollectionExtremaUnsupportedValue),
    };
    let hidden = match &collect.into {
        Value::Variable(variable) => variable,
        _ => return Err(BgpPlanError::CollectionExtremaUnsupportedShape),
    };
    let list = match range.0 {
        DataValue::Variable(variable) => variable,
        _ => return Err(BgpPlanError::CollectionExtremaUnsupportedShape),
    };
    let result = match range.1 {
        DataValue::Variable(variable) => variable,
        _ => return Err(BgpPlanError::CollectionExtremaUnsupportedShape),
    };
    if hidden != list || hidden == result || template == hidden || template == result {
        return Err(BgpPlanError::CollectionExtremaUnsupportedShape);
    }
    let child = plan_bgp(&collect.query)?;
    if child.projection.is_some() || child.count_variable.is_some() || child.full_row_distinct {
        return Err(BgpPlanError::CollectionExtremaUnsupportedShape);
    }
    if child
        .relation
        .schema
        .iter()
        .any(|variable| variable == hidden || variable == result)
    {
        return Err(BgpPlanError::CollectionExtremaUnsupportedShape);
    }
    let value_column = child
        .relation
        .schema
        .iter()
        .position(|variable| variable == template)
        .ok_or(BgpPlanError::CollectionExtremaUnsupportedValue)?;
    let descriptors = relation_result_descriptors(&child.relation)
        .map_err(|_| BgpPlanError::CollectionExtremaUnsupportedShape)?;
    if descriptors[value_column].nullable
        || descriptors[value_column].namespace != ResultIdNamespace::NodeValue
    {
        return Err(BgpPlanError::CollectionExtremaUnsupportedValue);
    }
    let datatype = match child.relation.domains.get(value_column).copied().flatten() {
        Some(EqualityValueDomain::Datatype(
            datatype @ (CanonicalDatatype::UInt8
            | CanonicalDatatype::UInt16
            | CanonicalDatatype::UInt32
            | CanonicalDatatype::UInt64
            | CanonicalDatatype::Int8
            | CanonicalDatatype::Int16
            | CanonicalDatatype::Int32
            | CanonicalDatatype::Int64),
        )) => datatype,
        _ => return Err(BgpPlanError::CollectionExtremaUnsupportedValue),
    };
    let collection = child.collection;
    Ok(Some(CompiledBgp {
        relation: CompiledRelation {
            schema: vec![result.clone()],
            domains: vec![Some(EqualityValueDomain::Datatype(datatype))],
            statement: CompiledRelationStatement::CollectionExtrema {
                input: Box::new(child.relation),
                value_column,
                datatype,
                kind,
            },
        },
        projection: None,
        collection,
        count_variable: None,
        full_row_distinct: false,
    }))
}

fn exact_integral_decimal_i64(value: &XSDAnySimpleType) -> Result<i64, BgpPlanError> {
    let XSDAnySimpleType::Decimal(decimal) = value else {
        return Err(BgpPlanError::GroupSumComparisonUnsupportedConstant);
    };
    if decimal.scale() < 0 {
        return Err(BgpPlanError::GroupSumComparisonUnsupportedConstant);
    }
    let value = i64::try_from(decimal)
        .map_err(|_| BgpPlanError::GroupSumComparisonUnsupportedConstant)?;
    if decimal != &decimal_rs::Decimal::from(value) {
        return Err(BgpPlanError::GroupSumComparisonUnsupportedConstant);
    }
    Ok(value)
}

fn generated_group_sum_constraint(
    query: &Query,
    variable: &str,
    signed: bool,
) -> Result<(GeneratedU64ComparisonOp, ResultIdNamespace, i128, bool), BgpPlanError> {
    let constant = |value: &XSDAnySimpleType| -> Result<_, BgpPlanError> {
        if signed {
            Ok((
                ResultIdNamespace::GeneratedDecimalI64,
                i128::from(exact_integral_decimal_i64(value)?),
            ))
        } else {
            let value = exact_integral_decimal_u64(value)
                .map_err(|_| BgpPlanError::GroupSumComparisonUnsupportedConstant)?;
            Ok((ResultIdNamespace::GeneratedDecimalU64, i128::from(value)))
        }
    };
    let range = |left: &DataValue,
                 right: &DataValue,
                 op: GeneratedU64ComparisonOp|
     -> Result<_, BgpPlanError> {
        match (left, right) {
            (DataValue::Variable(candidate), DataValue::Data(value)) if candidate == variable => {
                let (domain, constant) = constant(value)?;
                Ok((op, domain, constant, false))
            }
            (DataValue::Data(value), DataValue::Variable(candidate)) if candidate == variable => {
                let (domain, constant) = constant(value)?;
                Ok((op, domain, constant, true))
            }
            _ => Err(BgpPlanError::GroupSumComparisonUnsupportedShape),
        }
    };
    match query {
        Query::Greater(value) => range(&value.left, &value.right, GeneratedU64ComparisonOp::Gt),
        Query::Gte(value) => range(&value.left, &value.right, GeneratedU64ComparisonOp::Ge),
        Query::Less(value) => range(&value.left, &value.right, GeneratedU64ComparisonOp::Lt),
        Query::Lte(value) => range(&value.left, &value.right, GeneratedU64ComparisonOp::Le),
        Query::Equals(value) => {
            use crate::value::WoqlValue;
            match (&value.left, &value.right) {
                (WoqlValue::Variable(candidate), WoqlValue::Data(value))
                    if candidate == variable =>
                {
                    let (domain, constant) = constant(value)?;
                    Ok((GeneratedU64ComparisonOp::Eq, domain, constant, false))
                }
                (WoqlValue::Data(value), WoqlValue::Variable(candidate))
                    if candidate == variable =>
                {
                    let (domain, constant) = constant(value)?;
                    Ok((GeneratedU64ComparisonOp::Eq, domain, constant, true))
                }
                _ => Err(BgpPlanError::GroupSumComparisonUnsupportedShape),
            }
        }
        _ => Err(BgpPlanError::GroupSumComparisonUnsupportedShape),
    }
}

fn exact_collection_sum_constant(
    value: &XSDAnySimpleType,
) -> Result<(CollectionSumTemplate, CanonicalDatatype), BgpPlanError> {
    Ok(match value {
        XSDAnySimpleType::UnsignedByte(value) => (
            CollectionSumTemplate::UnsignedConstant(u64::from(*value)),
            CanonicalDatatype::UInt8,
        ),
        XSDAnySimpleType::Byte(value) => (
            CollectionSumTemplate::SignedConstant(i64::from(*value)),
            CanonicalDatatype::Int8,
        ),
        XSDAnySimpleType::UnsignedShort(value) => (
            CollectionSumTemplate::UnsignedConstant(u64::from(*value)),
            CanonicalDatatype::UInt16,
        ),
        XSDAnySimpleType::Short(value) => (
            CollectionSumTemplate::SignedConstant(i64::from(*value)),
            CanonicalDatatype::Int16,
        ),
        XSDAnySimpleType::UnsignedInt(value) => (
            CollectionSumTemplate::UnsignedConstant(u64::from(
                u32::try_from(*value).map_err(|_| BgpPlanError::CollectionSumUnsupportedValue)?,
            )),
            CanonicalDatatype::UInt32,
        ),
        XSDAnySimpleType::Int(value) => (
            CollectionSumTemplate::SignedConstant(i64::from(*value)),
            CanonicalDatatype::Int32,
        ),
        XSDAnySimpleType::UnsignedLong(value) => (
            CollectionSumTemplate::UnsignedConstant(*value),
            CanonicalDatatype::UInt64,
        ),
        XSDAnySimpleType::Long(value) | XSDAnySimpleType::Integer(value) => (
            CollectionSumTemplate::SignedConstant(*value),
            CanonicalDatatype::Int64,
        ),
        _ => return Err(BgpPlanError::CollectionSumUnsupportedValue),
    })
}

fn plan_collection_sum_form(query: &Query) -> Result<Option<CompiledBgp>, BgpPlanError> {
    let conjuncts = match query {
        Query::And(and) if and.and.len() >= 2 => &and.and,
        _ => return Ok(None),
    };
    if conjuncts.iter().skip(2).any(|query| {
        matches!(query, Query::If(_) | Query::WoqlOptional(_) | Query::Not(_))
    }) {
        return Ok(None);
    }
    let (Query::Collect(collect), Query::Sum(sum)) = (&conjuncts[0], &conjuncts[1]) else {
        if matches!(conjuncts.first(), Some(Query::GroupBy(_)))
            || conjuncts.iter().any(|query| matches!(query, Query::Length(_)))
        {
            return Ok(None);
        }
        if conjuncts.iter().any(|query| matches!(query, Query::Collect(_)))
            && !conjuncts.iter().any(|query| matches!(query, Query::GroupBy(_)))
        {
            return Err(BgpPlanError::CollectionSumUnsupportedShape);
        }
        return Ok(None);
    };
    let hidden = match &collect.into {
        Value::Variable(variable) => variable,
        _ => return Err(BgpPlanError::CollectionSumUnsupportedShape),
    };
    let list = match &sum.list {
        DataValue::Variable(variable) => variable,
        _ => return Err(BgpPlanError::CollectionSumUnsupportedShape),
    };
    let result = match &sum.result {
        DataValue::Variable(variable) => variable,
        _ => return Err(BgpPlanError::CollectionSumUnsupportedShape),
    };
    if hidden != list || hidden == result {
        return Err(BgpPlanError::CollectionSumUnsupportedShape);
    }
    if matches!(&collect.template, Value::Variable(variable) if variable == hidden || variable == result)
    {
        return Err(BgpPlanError::CollectionSumUnsupportedShape);
    }
    let child = plan_bgp(&collect.query)?;
    if child.projection.is_some() || child.count_variable.is_some() || child.full_row_distinct {
        return Err(BgpPlanError::CollectionSumUnsupportedShape);
    }
    if child
        .relation
        .schema
        .iter()
        .any(|variable| variable == hidden || variable == result)
    {
        return Err(BgpPlanError::CollectionSumUnsupportedShape);
    }
    let descriptors = relation_result_descriptors(&child.relation)
        .map_err(|_| BgpPlanError::CollectionSumUnsupportedShape)?;
    let (template, datatype) = match &collect.template {
        Value::Variable(variable) => {
            let column = child
                .relation
                .schema
                .iter()
                .position(|candidate| candidate == variable)
                .ok_or(BgpPlanError::CollectionSumUnsupportedValue)?;
            if descriptors[column].nullable
                || descriptors[column].namespace != ResultIdNamespace::NodeValue
            {
                return Err(BgpPlanError::CollectionSumUnsupportedValue);
            }
            let datatype = match child.relation.domains.get(column).copied().flatten() {
                Some(EqualityValueDomain::Datatype(
                    datatype @ (CanonicalDatatype::UInt8
                    | CanonicalDatatype::UInt16
                    | CanonicalDatatype::UInt32
                    | CanonicalDatatype::UInt64
                    | CanonicalDatatype::Int8
                    | CanonicalDatatype::Int16
                    | CanonicalDatatype::Int32
                    | CanonicalDatatype::Int64),
                )) => datatype,
                _ => return Err(BgpPlanError::CollectionSumUnsupportedValue),
            };
            (CollectionSumTemplate::Column(column), datatype)
        }
        Value::Data(value) => exact_collection_sum_constant(value)?,
        _ => return Err(BgpPlanError::CollectionSumUnsupportedValue),
    };
    let collection = child.collection;
    let signed = matches!(
        datatype,
        CanonicalDatatype::Int8
            | CanonicalDatatype::Int16
            | CanonicalDatatype::Int32
            | CanonicalDatatype::Int64
    );
    let mut relation = CompiledRelation {
        schema: vec![result.clone()],
        domains: vec![None],
        statement: CompiledRelationStatement::CollectionSum {
            input: Box::new(child.relation),
            template,
            datatype,
        },
    };
    for comparison in conjuncts.iter().skip(2) {
        let (op, domain, constant, constant_on_left) =
            generated_group_sum_constraint(comparison, result, signed)?;
        let input = relation;
        relation = CompiledRelation {
            schema: input.schema.clone(),
            domains: input.domains.clone(),
            statement: match domain {
                ResultIdNamespace::GeneratedDecimalU64 => {
                    CompiledRelationStatement::GeneratedU64Filter {
                        input: Box::new(input),
                        column: 0,
                        op,
                        constant: u64::try_from(constant).map_err(|_| {
                            BgpPlanError::GroupSumComparisonUnsupportedConstant
                        })?,
                        constant_on_left,
                    }
                }
                ResultIdNamespace::GeneratedDecimalI64 => {
                    CompiledRelationStatement::GeneratedI64Filter {
                        input: Box::new(input),
                        column: 0,
                        op,
                        constant: i64::try_from(constant).map_err(|_| {
                            BgpPlanError::GroupSumComparisonUnsupportedConstant
                        })?,
                        constant_on_left,
                    }
                }
                _ => unreachable!(),
            },
        };
    }
    Ok(Some(CompiledBgp {
        relation,
        projection: None,
        collection,
        count_variable: None,
        full_row_distinct: false,
    }))
}

/// Compile a WOQL `Eval` over integer arithmetic into a per-row derived-column relation.
///
/// Shape: an `And` whose conjuncts are an operand-binding child BGP plus EXACTLY ONE `Eval`.
/// The child is compiled with [`plan_bgp`] (so its triples + range comparisons authenticate
/// and datatype-pin the operand columns exactly as the Sum forms rely on), and the `Eval`
/// expression is lowered to an [`EvalExpr`] over child column indices and integer literals.
///
/// Everything outside the provable integer fragment rejects closed with a specific error:
/// `Divide`/`Floor` (rational layer), a static negative `Exp` exponent, non-integer
/// literals/operands, operands not bound by a store read, and result overflow (enforced later,
/// in the recompute's checked i64 narrowing). `Div`/`Exp` (non-negative exponent) ARE provable.
fn plan_eval_form(query: &Query) -> Result<Option<CompiledBgp>, BgpPlanError> {
    let conjuncts = match query {
        Query::And(and) => &and.and,
        _ => return Ok(None),
    };
    let eval_positions: Vec<usize> = conjuncts
        .iter()
        .enumerate()
        .filter(|(_, conjunct)| matches!(conjunct, Query::Eval(_)))
        .map(|(index, _)| index)
        .collect();
    if eval_positions.is_empty() {
        // No Eval conjunct: let the other planners handle this query.
        return Ok(None);
    }
    if eval_positions.len() > 1 {
        // Multiple Evals would need a dependency-ordered chain of derived columns; v1 takes
        // exactly one. Reject closed rather than proving only the first.
        return Err(BgpPlanError::EvalUnsupportedShape);
    }
    let Query::Eval(eval) = &conjuncts[eval_positions[0]] else {
        unreachable!("eval_positions only indexes Eval conjuncts")
    };

    // The operand-binding child is every OTHER conjunct.
    let child_conjuncts: Vec<Query> = conjuncts
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != eval_positions[0])
        .map(|(_, conjunct)| conjunct.clone())
        .collect();
    if child_conjuncts.is_empty() {
        // A bare Eval with no store binding cannot authenticate any operand variable.
        return Err(BgpPlanError::EvalUnsupportedShape);
    }
    let child_query = if child_conjuncts.len() == 1 {
        child_conjuncts.into_iter().next().unwrap()
    } else {
        Query::And(And {
            and: child_conjuncts,
        })
    };
    let child = plan_bgp(&child_query)?;
    if child.projection.is_some() || child.count_variable.is_some() || child.full_row_distinct {
        return Err(BgpPlanError::EvalUnsupportedShape);
    }

    let result = match &eval.result_value {
        ArithmeticValue::Variable(variable) => variable.clone(),
        ArithmeticValue::Data(_) => return Err(BgpPlanError::EvalResultNotVariable),
    };
    if child.relation.schema.iter().any(|variable| variable == &result) {
        return Err(BgpPlanError::EvalResultAlreadyBound { var: result });
    }

    // ROUTING. Pure-INTEGER arithmetic (pinned fixed-width integer operands, only
    // Plus/Minus/Times/Div/Exp) keeps the existing integer `Eval` delivery (a committed
    // `GeneratedDecimalI64` column). Anything with a RATIONAL SIGNAL — `Divide` (exact rational),
    // `Floor`, or an `xsd:decimal` literal — uses the exact-rational value layer: a TOP-LEVEL
    // `Floor` still delivers an integer (`GeneratedDecimalI64`), everything else delivers a
    // TERMINAL, REVEALED `xsd:decimal` value (like `Concat`). Decimal OPERAND columns without such
    // a signal are indistinguishable from unpinned integers at compile time, so they remain
    // reject-closed (via the integer path) rather than guessed — a decimal operand is provable only
    // when the tree ALSO carries a `Divide`/`Floor`/decimal-literal that routes it here.
    let collection = child.collection.clone();
    if !arith_signals_rational(&eval.expression) {
        // ---- Integer `Eval` (unchanged) ----
        let expr = compile_eval_expr(&eval.expression, &child)?;
        let mut schema = child.relation.schema.clone();
        schema.push(result);
        let mut domains = child.relation.domains.clone();
        // The derived column is a generated integer, not an executor value domain.
        domains.push(None);
        let relation = CompiledRelation {
            schema,
            domains,
            statement: CompiledRelationStatement::Eval {
                input: Box::new(child.relation),
                expr,
            },
        };
        return Ok(Some(CompiledBgp {
            relation,
            projection: None,
            collection,
            count_variable: None,
            full_row_distinct: false,
        }));
    }

    // ---- Exact-rational value layer ----
    let relation = if let ArithmeticExpression::Floor(floor) = &eval.expression {
        // TOP-LEVEL `floor(x)` -> INTEGER delivery (append a `GeneratedDecimalI64` column, exactly
        // like `Eval`). The floor is applied by `expected_floor_eval_columns`; store its ARGUMENT.
        let expr = compile_decimal_expr(&floor.argument, &child)?;
        let mut schema = child.relation.schema.clone();
        schema.push(result);
        let mut domains = child.relation.domains.clone();
        domains.push(None);
        CompiledRelation {
            schema,
            domains,
            statement: CompiledRelationStatement::FloorEval {
                input: Box::new(child.relation),
                expr,
            },
        }
    } else {
        // Fractional-capable result -> TERMINAL, REVEALED `xsd:decimal` (like `Concat`): the child
        // schema/domains pass through UNCHANGED and the result variable lives only in the statement.
        let expr = compile_decimal_expr(&eval.expression, &child)?;
        CompiledRelation {
            schema: child.relation.schema.clone(),
            domains: child.relation.domains.clone(),
            statement: CompiledRelationStatement::RationalArith {
                input: Box::new(child.relation),
                expr,
                result,
            },
        }
    };
    Ok(Some(CompiledBgp {
        relation,
        projection: None,
        collection,
        count_variable: None,
        full_row_distinct: false,
    }))
}

/// Does this arithmetic tree carry a RATIONAL SIGNAL that routes it to the exact-rational value
/// layer? True iff it contains a `Divide` (`rdiv` — always fractional-capable), a `Floor`, or an
/// `xsd:decimal` literal. `Div` (integer floor division) is NOT itself a signal (it stays on the
/// integer path unless a nested `Divide`/`Floor`/decimal forces routing, in which case
/// [`compile_decimal_expr`] rejects the integer-only `Div`). A bare integer/variable leaf is not a
/// signal — so an UNPINNED store operand keeps its existing reject-closed behavior on the integer
/// path rather than being silently reinterpreted as a decimal.
fn arith_signals_rational(expression: &ArithmeticExpression) -> bool {
    match expression {
        ArithmeticExpression::Value(ArithmeticValue::Data(XSDAnySimpleType::Decimal(_))) => true,
        ArithmeticExpression::Value(_) => false,
        ArithmeticExpression::Divide(_) => true,
        ArithmeticExpression::Floor(_) => true,
        ArithmeticExpression::Plus(plus) => {
            arith_signals_rational(&plus.left) || arith_signals_rational(&plus.right)
        }
        ArithmeticExpression::Minus(minus) => {
            arith_signals_rational(&minus.left) || arith_signals_rational(&minus.right)
        }
        ArithmeticExpression::Times(times) => {
            arith_signals_rational(&times.left) || arith_signals_rational(&times.right)
        }
        ArithmeticExpression::Div(div) => {
            arith_signals_rational(&div.left) || arith_signals_rational(&div.right)
        }
        ArithmeticExpression::Exp(exp) => {
            arith_signals_rational(&exp.left) || arith_signals_rational(&exp.right)
        }
    }
}

/// Lower a WOQL `ArithmeticExpression` to a [`DecimalExpr`] (exact-rational layer), rejecting every
/// out-of-scope node. Integer-only `Div` is rejected here (it belongs to the integer `Eval` path);
/// `Exp` requires a static non-negative integer exponent; a static zero `Divide` divisor rejects
/// closed at compile time (a runtime zero divisor fails closed in the recompute).
fn compile_decimal_expr(
    expression: &ArithmeticExpression,
    child: &CompiledBgp,
) -> Result<DecimalExpr, BgpPlanError> {
    match expression {
        ArithmeticExpression::Value(value) => compile_decimal_value(value, child),
        ArithmeticExpression::Plus(plus) => Ok(DecimalExpr::Plus(
            Box::new(compile_decimal_expr(&plus.left, child)?),
            Box::new(compile_decimal_expr(&plus.right, child)?),
        )),
        ArithmeticExpression::Minus(minus) => Ok(DecimalExpr::Minus(
            Box::new(compile_decimal_expr(&minus.left, child)?),
            Box::new(compile_decimal_expr(&minus.right, child)?),
        )),
        ArithmeticExpression::Times(times) => Ok(DecimalExpr::Times(
            Box::new(compile_decimal_expr(&times.left, child)?),
            Box::new(compile_decimal_expr(&times.right, child)?),
        )),
        ArithmeticExpression::Divide(divide) => {
            // A STATIC zero divisor is `evaluation_error(zero_divisor)` in Prolog: reject at compile
            // time (a runtime/column zero divisor fails closed in the recompute instead).
            if arith_is_static_zero(&divide.right) {
                return Err(BgpPlanError::EvalDivideByZero);
            }
            Ok(DecimalExpr::Divide(
                Box::new(compile_decimal_expr(&divide.left, child)?),
                Box::new(compile_decimal_expr(&divide.right, child)?),
            ))
        }
        ArithmeticExpression::Exp(exp) => {
            // Only a STATIC non-negative integer exponent gives an exact rational power here.
            let exponent = match exp.right.as_ref() {
                ArithmeticExpression::Value(ArithmeticValue::Data(literal)) => {
                    eval_integer_literal(literal)?
                }
                _ => {
                    return Err(BgpPlanError::EvalUnsupportedOperator {
                        operator: "Exp (non-static exponent in the decimal layer)",
                    })
                }
            };
            if exponent < 0 {
                return Err(BgpPlanError::EvalUnsupportedOperator {
                    operator: "Exp (negative exponent)",
                });
            }
            Ok(DecimalExpr::Exp(
                Box::new(compile_decimal_expr(&exp.left, child)?),
                Box::new(DecimalExpr::IntegerLiteral(exponent)),
            ))
        }
        ArithmeticExpression::Floor(floor) => Ok(DecimalExpr::Floor(Box::new(compile_decimal_expr(
            &floor.argument,
            child,
        )?))),
        // Integer FLOOR division (`div`) is not part of the exact-rational layer (its operands must
        // be integers, and its result is an integer already handled by the integer `Eval` path). If
        // it appears alongside a rational signal, the mixed form is not provable: reject closed.
        ArithmeticExpression::Div(_) => Err(BgpPlanError::EvalUnsupportedOperator {
            operator: "Div (integer floor division mixed with rational arithmetic)",
        }),
    }
}

/// Lower an arithmetic LEAF (numeric operand variable or integer/decimal literal) to a
/// [`DecimalExpr`] leaf for the exact-rational layer. An operand VARIABLE must be a nonnullable
/// node/value store column (its exact numeric datatype is resolved at recompute — see
/// [`DecimalExpr::Column`]); a non-numeric literal (float/string/boolean/…) rejects closed.
fn compile_decimal_value(
    value: &ArithmeticValue,
    child: &CompiledBgp,
) -> Result<DecimalExpr, BgpPlanError> {
    match value {
        ArithmeticValue::Variable(variable) => {
            let index = child
                .relation
                .schema
                .iter()
                .position(|candidate| candidate == variable)
                .ok_or_else(|| BgpPlanError::EvalOperandNotBound {
                    var: variable.clone(),
                })?;
            // The operand column must be a nonnullable node/value STORE column (never a node IRI,
            // predicate, or nullable Optional column). Its exact datatype (integer family or
            // xsd:decimal) is NOT pinned at compile time — it is read from the ROOT-authenticated
            // dictionary value at recompute, where a non-numeric row rejects closed.
            let descriptors = relation_result_descriptors(&child.relation).map_err(|_| {
                BgpPlanError::EvalOperandNotNumeric {
                    var: variable.clone(),
                }
            })?;
            if descriptors[index].nullable
                || descriptors[index].namespace != ResultIdNamespace::NodeValue
            {
                return Err(BgpPlanError::EvalOperandNotNumeric {
                    var: variable.clone(),
                });
            }
            Ok(DecimalExpr::Column { index })
        }
        ArithmeticValue::Data(data) => match data {
            // Exact integer literals (all fit i128) — interpreted as `value/1`.
            XSDAnySimpleType::UnsignedByte(_)
            | XSDAnySimpleType::Byte(_)
            | XSDAnySimpleType::UnsignedShort(_)
            | XSDAnySimpleType::Short(_)
            | XSDAnySimpleType::UnsignedInt(_)
            | XSDAnySimpleType::Int(_)
            | XSDAnySimpleType::UnsignedLong(_)
            | XSDAnySimpleType::Long(_)
            | XSDAnySimpleType::Integer(_) => {
                Ok(DecimalExpr::IntegerLiteral(eval_integer_literal(data)?))
            }
            // Exact xsd:decimal literal — canonicalize to its decimal string and validate it parses
            // to an exact rational (rejecting scientific/garbage forms) so the recompute re-parses
            // it identically.
            XSDAnySimpleType::Decimal(decimal) => {
                let lexical = decimal.to_string();
                parse_decimal_to_rational(&lexical).map_err(|_| {
                    BgpPlanError::EvalUnsupportedLiteral {
                        datatype: "xsd:decimal (non-canonical)",
                    }
                })?;
                Ok(DecimalExpr::DecimalLiteral(lexical))
            }
            other => Err(BgpPlanError::EvalUnsupportedLiteral {
                datatype: xsd_simple_type_name(other),
            }),
        },
    }
}

/// Is this arithmetic node a STATIC numeric literal equal to zero (an integer `0` or a decimal
/// whose value is zero)? Used to reject a compile-time `Divide` by a zero constant.
fn arith_is_static_zero(expression: &ArithmeticExpression) -> bool {
    let ArithmeticExpression::Value(ArithmeticValue::Data(data)) = expression else {
        return false;
    };
    match data {
        XSDAnySimpleType::UnsignedByte(v) => *v == 0,
        XSDAnySimpleType::Byte(v) => *v == 0,
        XSDAnySimpleType::UnsignedShort(v) => *v == 0,
        XSDAnySimpleType::Short(v) => *v == 0,
        XSDAnySimpleType::UnsignedInt(v) => *v == 0,
        XSDAnySimpleType::Int(v) => *v == 0,
        XSDAnySimpleType::UnsignedLong(v) => *v == 0,
        XSDAnySimpleType::Long(v) => *v == 0,
        XSDAnySimpleType::Integer(v) => *v == 0,
        XSDAnySimpleType::Decimal(d) => parse_decimal_to_rational(&d.to_string())
            .map(|r| r.numer().is_zero())
            .unwrap_or(false),
        _ => false,
    }
}

/// Lower a WOQL `ArithmeticExpression` to an [`EvalExpr`], rejecting every out-of-scope node.
fn compile_eval_expr(
    expression: &ArithmeticExpression,
    child: &CompiledBgp,
) -> Result<EvalExpr, BgpPlanError> {
    match expression {
        ArithmeticExpression::Value(value) => compile_eval_value(value, child),
        ArithmeticExpression::Plus(plus) => Ok(EvalExpr::Plus(
            Box::new(compile_eval_expr(&plus.left, child)?),
            Box::new(compile_eval_expr(&plus.right, child)?),
        )),
        ArithmeticExpression::Minus(minus) => Ok(EvalExpr::Minus(
            Box::new(compile_eval_expr(&minus.left, child)?),
            Box::new(compile_eval_expr(&minus.right, child)?),
        )),
        ArithmeticExpression::Times(times) => Ok(EvalExpr::Times(
            Box::new(compile_eval_expr(&times.left, child)?),
            Box::new(compile_eval_expr(&times.right, child)?),
        )),
        // INTEGER FLOOR division: recompute-provable (verifier re-derives the floored quotient,
        // fails closed on a zero divisor — see [`EvalExpr::Div`]).
        ArithmeticExpression::Div(div) => Ok(EvalExpr::Div(
            Box::new(compile_eval_expr(&div.left, child)?),
            Box::new(compile_eval_expr(&div.right, child)?),
        )),
        // INTEGER power over a non-negative exponent: recompute-provable with checked pow.
        // If the exponent is a STATIC negative literal, reject closed at compile time (its
        // rational result is out of this integer layer); a runtime column exponent that turns
        // out negative fails closed in the recompute instead.
        ArithmeticExpression::Exp(exp) => {
            if let ArithmeticExpression::Value(ArithmeticValue::Data(literal)) = exp.right.as_ref() {
                if eval_integer_literal(literal)? < 0 {
                    return Err(BgpPlanError::EvalUnsupportedOperator {
                        operator: "Exp (negative exponent)",
                    });
                }
            }
            Ok(EvalExpr::Exp(
                Box::new(compile_eval_expr(&exp.left, child)?),
                Box::new(compile_eval_expr(&exp.right, child)?),
            ))
        }
        // `Divide` (`/`/`rdiv`) is EXACT RATIONAL, and `Floor` over rationals needs the
        // rational value layer; both are out of the integer fragment. Reject closed.
        ArithmeticExpression::Divide(_) => Err(BgpPlanError::EvalUnsupportedOperator {
            operator: "Divide",
        }),
        ArithmeticExpression::Floor(_) => Err(BgpPlanError::EvalUnsupportedOperator {
            operator: "Floor",
        }),
    }
}

/// Lower an arithmetic LEAF (variable operand or literal) to an [`EvalExpr`] leaf.
fn compile_eval_value(
    value: &ArithmeticValue,
    child: &CompiledBgp,
) -> Result<EvalExpr, BgpPlanError> {
    match value {
        ArithmeticValue::Variable(variable) => {
            let index = child
                .relation
                .schema
                .iter()
                .position(|candidate| candidate == variable)
                .ok_or_else(|| BgpPlanError::EvalOperandNotBound {
                    var: variable.clone(),
                })?;
            // The operand column must be a nonnullable node/value column (a store value), not a
            // node IRI, predicate, or nullable Optional column.
            let descriptors = relation_result_descriptors(&child.relation)
                .map_err(|_| BgpPlanError::EvalOperandNotInteger {
                    var: variable.clone(),
                })?;
            if descriptors[index].nullable
                || descriptors[index].namespace != ResultIdNamespace::NodeValue
            {
                return Err(BgpPlanError::EvalOperandNotInteger {
                    var: variable.clone(),
                });
            }
            // Its exact fixed-width integer datatype must be pinned (by a range comparison, as
            // in the Sum forms). Anything else — floats, decimals, unpinned values — rejects.
            let datatype = match child.relation.domains.get(index).copied().flatten() {
                Some(EqualityValueDomain::Datatype(
                    datatype @ (CanonicalDatatype::UInt8
                    | CanonicalDatatype::UInt16
                    | CanonicalDatatype::UInt32
                    | CanonicalDatatype::UInt64
                    | CanonicalDatatype::Int8
                    | CanonicalDatatype::Int16
                    | CanonicalDatatype::Int32
                    | CanonicalDatatype::Int64),
                )) => datatype,
                _ => {
                    return Err(BgpPlanError::EvalOperandNotInteger {
                        var: variable.clone(),
                    })
                }
            };
            Ok(EvalExpr::Column { index, datatype })
        }
        ArithmeticValue::Data(data) => Ok(EvalExpr::Literal(eval_integer_literal(data)?)),
    }
}

/// Accept only exact integer XSD literals (all fit `i128`); reject float/decimal/other closed.
fn eval_integer_literal(value: &XSDAnySimpleType) -> Result<i128, BgpPlanError> {
    Ok(match value {
        XSDAnySimpleType::UnsignedByte(v) => i128::from(*v),
        XSDAnySimpleType::Byte(v) => i128::from(*v),
        XSDAnySimpleType::UnsignedShort(v) => i128::from(*v),
        XSDAnySimpleType::Short(v) => i128::from(*v),
        XSDAnySimpleType::UnsignedInt(v) => i128::try_from(*v).map_err(|_| {
            BgpPlanError::EvalUnsupportedLiteral {
                datatype: "xsd:unsignedInt",
            }
        })?,
        XSDAnySimpleType::Int(v) => i128::from(*v),
        XSDAnySimpleType::UnsignedLong(v) => i128::from(*v),
        XSDAnySimpleType::Long(v) => i128::from(*v),
        XSDAnySimpleType::Integer(v) => i128::from(*v),
        other => {
            return Err(BgpPlanError::EvalUnsupportedLiteral {
                datatype: xsd_simple_type_name(other),
            })
        }
    })
}

/// A stable XSD datatype name for a non-integer literal, used only in reject-closed errors.
fn xsd_simple_type_name(value: &XSDAnySimpleType) -> &'static str {
    match value {
        XSDAnySimpleType::String(_) => "xsd:string",
        XSDAnySimpleType::Decimal(_) => "xsd:decimal",
        XSDAnySimpleType::Float(_) => "xsd:double",
        XSDAnySimpleType::Boolean(_) => "xsd:boolean",
        XSDAnySimpleType::HexBinary(_) => "xsd:hexBinary",
        XSDAnySimpleType::URI(_) => "xsd:anyURI",
        XSDAnySimpleType::DateTime(_) => "xsd:dateTime",
        XSDAnySimpleType::Date(_) => "xsd:date",
        XSDAnySimpleType::Time(_) => "xsd:time",
        XSDAnySimpleType::UnsignedByte(_) => "xsd:unsignedByte",
        XSDAnySimpleType::Byte(_) => "xsd:byte",
        XSDAnySimpleType::UnsignedShort(_) => "xsd:unsignedShort",
        XSDAnySimpleType::Short(_) => "xsd:short",
        XSDAnySimpleType::UnsignedInt(_) => "xsd:unsignedInt",
        XSDAnySimpleType::Int(_) => "xsd:int",
        XSDAnySimpleType::UnsignedLong(_) => "xsd:unsignedLong",
        XSDAnySimpleType::Long(_) => "xsd:long",
        XSDAnySimpleType::Integer(_) => "xsd:integer",
    }
}

/// Compile a WOQL `Typecast` of ONE authenticated source column into a per-row cast relation.
///
/// Shape: an `And` whose conjuncts are a source-binding child BGP plus EXACTLY ONE `Typecast`.
/// The child is compiled with [`plan_bgp`] (so its triples + range comparisons authenticate and
/// datatype-pin the source column exactly as the Eval/Sum forms rely on). Provable cast families,
/// dispatched on the TARGET type:
///   * fixed-width integer → fixed-width integer (value-preserving; appends a COMMITTED derived
///     integer column) — the source must be a pinned fixed-width integer;
///   * number → `xsd:string` (integer via `~w`, xsd:decimal via `rational_to_decimal_string_20`),
///     and `xsd:string` → `xsd:integer` / `xsd:decimal` (exact reference-grammar parse) — delivered
///     as TERMINAL, revealed values (like `Concat`), the source a nonnullable store value column.
/// Every other case (float/double↔string, scientific/`+`/malformed string parses, a non-numeric /
/// non-string source, `xsd:string`→fixed-width, a variable target, an unbound source, a literal or
/// already-bound result) rejects closed with a specific error, or fails closed at recompute.
fn plan_typecast_form(query: &Query) -> Result<Option<CompiledBgp>, BgpPlanError> {
    let conjuncts = match query {
        Query::And(and) => &and.and,
        _ => return Ok(None),
    };
    let typecast_positions: Vec<usize> = conjuncts
        .iter()
        .enumerate()
        .filter(|(_, conjunct)| matches!(conjunct, Query::Typecast(_)))
        .map(|(index, _)| index)
        .collect();
    if typecast_positions.is_empty() {
        // No Typecast conjunct: let the other planners handle this query.
        return Ok(None);
    }
    if typecast_positions.len() > 1 {
        // Multiple casts would need a dependency-ordered chain of derived columns; v1 takes
        // exactly one. Reject closed rather than proving only the first.
        return Err(BgpPlanError::TypecastUnsupportedShape);
    }
    let Query::Typecast(typecast) = &conjuncts[typecast_positions[0]] else {
        unreachable!("typecast_positions only indexes Typecast conjuncts")
    };

    // The source-binding child is every OTHER conjunct.
    let child_conjuncts: Vec<Query> = conjuncts
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != typecast_positions[0])
        .map(|(_, conjunct)| conjunct.clone())
        .collect();
    if child_conjuncts.is_empty() {
        // A bare Typecast with no store binding cannot authenticate the source variable.
        return Err(BgpPlanError::TypecastUnsupportedShape);
    }
    let child_query = if child_conjuncts.len() == 1 {
        child_conjuncts.into_iter().next().unwrap()
    } else {
        Query::And(And {
            and: child_conjuncts,
        })
    };
    let child = plan_bgp(&child_query)?;
    if child.projection.is_some() || child.count_variable.is_some() || child.full_row_distinct {
        return Err(BgpPlanError::TypecastUnsupportedShape);
    }

    // Source: a variable (its authentication contract differs per cast kind, resolved below).
    let source_variable = match &typecast.value {
        Value::Variable(variable) => variable.clone(),
        _ => return Err(BgpPlanError::TypecastSourceNotVariable),
    };

    // Target: a concrete node IRI (never a variable).
    let target_iri = match &typecast.type_uri {
        NodeValue::Node(iri) => iri.clone(),
        NodeValue::Variable(_) => return Err(BgpPlanError::TypecastTargetNotNode),
    };

    // Result: a fresh output variable, never a literal and never a redefinition of a binding.
    let result = match &typecast.result_value {
        Value::Variable(variable) => variable.clone(),
        _ => return Err(BgpPlanError::TypecastResultInvalid),
    };
    if child.relation.schema.iter().any(|variable| variable == &result) {
        return Err(BgpPlanError::TypecastResultInvalid);
    }

    let collection = child.collection.clone();

    // A fixed-width INTEGER target is the value-PRESERVING integer→integer cast: it appends a
    // COMMITTED derived integer column, and the source must be a pinned fixed-width integer.
    if let Some(target_datatype) = canonical_integer_datatype_from_iri(&target_iri) {
        let (source_column, source_datatype) = resolve_typecast_source(&child, &source_variable)?;
        let mut schema = child.relation.schema.clone();
        schema.push(result);
        let mut domains = child.relation.domains.clone();
        // The derived cast column is a generated integer, not an executor value domain.
        domains.push(None);
        let relation = CompiledRelation {
            schema,
            domains,
            statement: CompiledRelationStatement::Typecast {
                input: Box::new(child.relation),
                source_column,
                source_datatype,
                target_datatype,
            },
        };
        return Ok(Some(CompiledBgp {
            relation,
            projection: None,
            collection,
            count_variable: None,
            full_row_distinct: false,
        }));
    }

    // Otherwise: a NUMBER↔STRING cast, dispatched on the target's XSD local name. The source must be
    // a NONNULLABLE node/value store column; its exact datatype (numeric for number→string,
    // xsd:string for string→number) is re-checked against the ROOT-authenticated value at recompute,
    // so a wrong-typed row FAILS CLOSED there. Unlike the integer→integer path, the result is a
    // TERMINAL, revealed value (child schema/domains pass through verbatim; the result is NOT a
    // committed column) — exactly like `plan_concat_form`.
    let cast = cast_string_target_from_iri(&target_iri)
        .ok_or(BgpPlanError::TypecastUnsupportedTarget { iri: target_iri })?;
    let source_kind = match cast {
        CastStringTarget::ToString => ValueSourceKind::Numeric,
        CastStringTarget::FromStringInteger | CastStringTarget::FromStringDecimal => {
            ValueSourceKind::Stringy
        }
    };
    let source_column = resolve_authenticated_value_source(&child, &source_variable, source_kind)?;
    let schema = child.relation.schema.clone();
    let domains = child.relation.domains.clone();
    let statement = match cast {
        // number → xsd:string
        CastStringTarget::ToString => CompiledRelationStatement::TypecastNumberToString {
            input: Box::new(child.relation),
            source_column,
            result,
        },
        // xsd:string → xsd:integer
        CastStringTarget::FromStringInteger => CompiledRelationStatement::TypecastStringToNumber {
            input: Box::new(child.relation),
            source_column,
            target: StringNumericTarget::Integer,
            result,
        },
        // xsd:string → xsd:decimal
        CastStringTarget::FromStringDecimal => CompiledRelationStatement::TypecastStringToNumber {
            input: Box::new(child.relation),
            source_column,
            target: StringNumericTarget::Decimal,
            result,
        },
    };
    let relation = CompiledRelation {
        schema,
        domains,
        statement,
    };
    Ok(Some(CompiledBgp {
        relation,
        projection: None,
        collection,
        count_variable: None,
        full_row_distinct: false,
    }))
}

/// Which NUMBER↔STRING cast a `Typecast` target IRI selects, by XSD local name (the segment after
/// the last `#`/`:`, covering full-URI, prefixed, and bare-local forms — exactly like
/// [`canonical_integer_datatype_from_iri`]). Disjoint from the fixed-width integer set that function
/// recognizes: `integer`/`decimal`/`string` are never fixed-width integers.
enum CastStringTarget {
    /// target `xsd:string` — a NUMBER→STRING cast.
    ToString,
    /// target `xsd:integer` (arbitrary precision) — a STRING→INTEGER cast.
    FromStringInteger,
    /// target `xsd:decimal` — a STRING→DECIMAL cast.
    FromStringDecimal,
}

fn cast_string_target_from_iri(iri: &str) -> Option<CastStringTarget> {
    let local = iri.rsplit(['#', ':']).next().unwrap_or(iri);
    Some(match local {
        "string" => CastStringTarget::ToString,
        "integer" => CastStringTarget::FromStringInteger,
        "decimal" => CastStringTarget::FromStringDecimal,
        _ => return None,
    })
}

/// The kind of source a NUMBER↔STRING `Typecast` expects, used to reject a PINNED-incompatible
/// source datatype at compile (an UNPINNED source is always allowed — its type is re-checked at
/// recompute, exactly like `compile_concat_element`).
#[derive(Clone, Copy)]
enum ValueSourceKind {
    /// number→string: the source must be numeric (integer family or `xsd:decimal`).
    Numeric,
    /// string→number: the source must be `xsd:string`.
    Stringy,
}

/// Resolve a NUMBER↔STRING `Typecast` source variable to its child result-column index, requiring a
/// NONNULLABLE node/value store column (a store value). Its exact per-row datatype is re-checked
/// against the ROOT-authenticated value at recompute (numeric for number→string, xsd:string for
/// string→number), so a wrong-typed row FAILS CLOSED there. A node IRI / predicate / nullable /
/// generated column rejects here (namespace/nullable check); an unbound variable rejects here; and —
/// mirroring `compile_concat_element` — a PINNED datatype INCOMPATIBLE with `kind` (e.g. a range
/// comparison pinned the column to a fixed-width integer but the cast is string→number) rejects
/// closed here, while an UNPINNED (`None`) column is allowed.
fn resolve_authenticated_value_source(
    child: &CompiledBgp,
    variable: &str,
    kind: ValueSourceKind,
) -> Result<usize, BgpPlanError> {
    let index = child
        .relation
        .schema
        .iter()
        .position(|candidate| candidate == variable)
        .ok_or_else(|| BgpPlanError::TypecastSourceNotBound {
            var: variable.to_string(),
        })?;
    let descriptors = relation_result_descriptors(&child.relation).map_err(|_| {
        BgpPlanError::TypecastValueSourceInvalid {
            var: variable.to_string(),
        }
    })?;
    if descriptors[index].nullable || descriptors[index].namespace != ResultIdNamespace::NodeValue {
        return Err(BgpPlanError::TypecastValueSourceInvalid {
            var: variable.to_string(),
        });
    }
    // Pinned-datatype compatibility (like Concat): `None` (unpinned) is always allowed; a pinned
    // datatype must match the cast direction, else reject closed.
    let compatible = match (kind, child.relation.domains.get(index).copied().flatten()) {
        (_, None) => true,
        (ValueSourceKind::Stringy, Some(EqualityValueDomain::Datatype(CanonicalDatatype::String))) => {
            true
        }
        (
            ValueSourceKind::Numeric,
            Some(EqualityValueDomain::Datatype(
                CanonicalDatatype::UInt8
                | CanonicalDatatype::UInt16
                | CanonicalDatatype::UInt32
                | CanonicalDatatype::UInt64
                | CanonicalDatatype::Int8
                | CanonicalDatatype::Int16
                | CanonicalDatatype::Int32
                | CanonicalDatatype::Int64
                | CanonicalDatatype::BigInt
                | CanonicalDatatype::Decimal,
            )),
        ) => true,
        (_, Some(_)) => false,
    };
    if !compatible {
        return Err(BgpPlanError::TypecastValueSourceInvalid {
            var: variable.to_string(),
        });
    }
    Ok(index)
}

/// Resolve a `Typecast` source variable to its (nonnullable, fixed-width-integer) child result
/// column and pinned datatype — the same authentication contract Eval operands require.
fn resolve_typecast_source(
    child: &CompiledBgp,
    variable: &str,
) -> Result<(usize, CanonicalDatatype), BgpPlanError> {
    let index = child
        .relation
        .schema
        .iter()
        .position(|candidate| candidate == variable)
        .ok_or_else(|| BgpPlanError::TypecastSourceNotBound {
            var: variable.to_string(),
        })?;
    // The source column must be a nonnullable node/value column (a store value), not a node IRI,
    // predicate, or nullable Optional column.
    let descriptors = relation_result_descriptors(&child.relation).map_err(|_| {
        BgpPlanError::TypecastSourceNotInteger {
            var: variable.to_string(),
        }
    })?;
    if descriptors[index].nullable || descriptors[index].namespace != ResultIdNamespace::NodeValue {
        return Err(BgpPlanError::TypecastSourceNotInteger {
            var: variable.to_string(),
        });
    }
    // Its exact fixed-width integer datatype must be pinned (by a range comparison, as in the Sum
    // forms). Anything else — floats, decimals, unpinned values — rejects.
    let datatype = match child.relation.domains.get(index).copied().flatten() {
        Some(EqualityValueDomain::Datatype(
            datatype @ (CanonicalDatatype::UInt8
            | CanonicalDatatype::UInt16
            | CanonicalDatatype::UInt32
            | CanonicalDatatype::UInt64
            | CanonicalDatatype::Int8
            | CanonicalDatatype::Int16
            | CanonicalDatatype::Int32
            | CanonicalDatatype::Int64),
        )) => datatype,
        _ => {
            return Err(BgpPlanError::TypecastSourceNotInteger {
                var: variable.to_string(),
            })
        }
    };
    Ok((index, datatype))
}

/// Map a target type IRI (full `http://www.w3.org/2001/XMLSchema#long`, prefixed `xsd:long`, or a
/// bare local name) to its fixed-width integer [`CanonicalDatatype`], or `None` for any
/// non-fixed-width-integer type (string/float/decimal, `xsd:integer` arbitrary precision, the
/// `positiveInteger`/`nonNegativeInteger`/… derived unbounded types, or an unknown IRI). Reject
/// closed on `None`: only the eight order-preserving fixed-width integer widths are provable.
fn canonical_integer_datatype_from_iri(iri: &str) -> Option<CanonicalDatatype> {
    // The local name is the segment after the last '#' or ':' separator (covers all three forms).
    let local = iri.rsplit(['#', ':']).next().unwrap_or(iri);
    Some(match local {
        "byte" => CanonicalDatatype::Int8,
        "short" => CanonicalDatatype::Int16,
        "int" => CanonicalDatatype::Int32,
        "long" => CanonicalDatatype::Int64,
        "unsignedByte" => CanonicalDatatype::UInt8,
        "unsignedShort" => CanonicalDatatype::UInt16,
        "unsignedInt" => CanonicalDatatype::UInt32,
        "unsignedLong" => CanonicalDatatype::UInt64,
        _ => return None,
    })
}

/// Compile a WOQL `Concatenate` conjunct (`concat(List, Result)`) into a verifier-recompute
/// [`CompiledRelationStatement::Concat`] — the FOUNDATION of the read-only string value-circuit
/// family, modelled on [`plan_eval_form`].
///
/// V1 SCOPE (reject-closed everywhere else): the list must be a CONCRETE inline list whose
/// elements are xsd:string LITERALS and/or variables bound (in the SAME query) as nonnullable
/// store-authenticated node/value columns; the result must be a FRESH variable (not a literal,
/// not already bound). Non-string list elements (numbers/nodes — `interpolate_string` would
/// stringify them, a separate circuit), a runtime list variable, a pinned non-string operand
/// datatype, and a result literal/already-bound variable all reject closed.
///
/// The result variable is deliberately NOT added to the committed schema: the concatenation is a
/// TERMINAL, revealed value (a synthesized string does not encode into a committed field element
/// and feeds nothing downstream). It is exposed post-verify via [`ProvedBgp::concat_result_objects`].
fn plan_concat_form(query: &Query) -> Result<Option<CompiledBgp>, BgpPlanError> {
    let conjuncts = match query {
        Query::And(and) => &and.and,
        _ => return Ok(None),
    };
    let positions: Vec<usize> = conjuncts
        .iter()
        .enumerate()
        .filter(|(_, conjunct)| matches!(conjunct, Query::Concatenate(_)))
        .map(|(index, _)| index)
        .collect();
    if positions.is_empty() {
        // No Concatenate conjunct: let the other planners handle this query.
        return Ok(None);
    }
    if positions.len() > 1 {
        // Multiple concats would need a dependency-ordered chain of derived strings; v1 takes
        // exactly one. Reject closed rather than proving only the first.
        return Err(BgpPlanError::ConcatUnsupportedShape);
    }
    let Query::Concatenate(concat) = &conjuncts[positions[0]] else {
        unreachable!("positions only indexes Concatenate conjuncts")
    };

    // The operand-binding child is every OTHER conjunct.
    let child_conjuncts: Vec<Query> = conjuncts
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != positions[0])
        .map(|(_, conjunct)| conjunct.clone())
        .collect();
    if child_conjuncts.is_empty() {
        // A bare concat with no store binding cannot authenticate any operand variable. (A concat
        // of only literals is a constant string with no rows — out of the per-row read fragment.)
        return Err(BgpPlanError::ConcatUnsupportedShape);
    }
    let child_query = if child_conjuncts.len() == 1 {
        child_conjuncts.into_iter().next().unwrap()
    } else {
        Query::And(And {
            and: child_conjuncts,
        })
    };
    let child = plan_bgp(&child_query)?;
    if child.projection.is_some() || child.count_variable.is_some() || child.full_row_distinct {
        return Err(BgpPlanError::ConcatUnsupportedShape);
    }

    // The result must be a FRESH variable (never a literal, never a rebinding of an operand).
    let result = match &concat.result_string {
        DataValue::Variable(variable) => variable.clone(),
        _ => return Err(BgpPlanError::ConcatResultNotVariable),
    };
    if child.relation.schema.iter().any(|variable| variable == &result) {
        return Err(BgpPlanError::ConcatResultAlreadyBound { var: result });
    }

    // The list must be a CONCRETE inline list (a runtime list variable's elements are unknown at
    // compile time, so their string-ness/order cannot be pinned into the query — reject closed).
    let items = match &concat.list {
        ListOrVariable::List(items) => items,
        ListOrVariable::Variable(_) => return Err(BgpPlanError::ConcatListNotConcrete),
    };
    if items.is_empty() {
        // `interpolate_string([], "")` yields the empty string with no per-row derivation; there
        // is no operand to authenticate, so it is out of the per-row provable fragment.
        return Err(BgpPlanError::ConcatEmptyList);
    }
    let elements = items
        .iter()
        .map(|item| compile_concat_element(item, &child))
        .collect::<Result<Vec<_>, _>>()?;

    // Keep the child schema/domains: the result variable is NOT a committed column (terminal,
    // revealed). Track it only inside the statement (for the query digest and result exposure).
    let collection = child.collection.clone();
    let relation = CompiledRelation {
        schema: child.relation.schema.clone(),
        domains: child.relation.domains.clone(),
        statement: CompiledRelationStatement::Concat {
            input: Box::new(child.relation),
            elements,
            result,
        },
    };
    Ok(Some(CompiledBgp {
        relation,
        projection: None,
        collection,
        count_variable: None,
        full_row_distinct: false,
    }))
}

/// Lower one `concat` list element (a `DataValue`) to a [`ConcatElement`].
///
/// A variable resolves to a nonnullable store-authenticated node/value column (its per-row
/// xsd:string-ness is re-checked against the AUTHENTICATED value at recompute time — a runtime
/// non-string row fails closed there); a pinned NON-string operand datatype rejects at compile
/// time. A string literal contributes its UTF-8 bytes. Non-string literals and nested lists
/// reject closed (they would require the stringification circuit).
fn compile_concat_element(
    value: &DataValue,
    child: &CompiledBgp,
) -> Result<ConcatElement, BgpPlanError> {
    match value {
        DataValue::Variable(variable) => {
            let index = child
                .relation
                .schema
                .iter()
                .position(|candidate| candidate == variable)
                .ok_or_else(|| BgpPlanError::ConcatOperandNotBound {
                    var: variable.clone(),
                })?;
            // The operand column must be a nonnullable node/value column (a store value), not a
            // node IRI, predicate, or nullable Optional column.
            let descriptors = relation_result_descriptors(&child.relation).map_err(|_| {
                BgpPlanError::ConcatOperandNotString {
                    var: variable.clone(),
                }
            })?;
            if descriptors[index].nullable
                || descriptors[index].namespace != ResultIdNamespace::NodeValue
            {
                return Err(BgpPlanError::ConcatOperandNotString {
                    var: variable.clone(),
                });
            }
            // If a datatype IS pinned for this column (e.g. by a range comparison), it must be
            // xsd:string. An UNPINNED column (`None`) is allowed: its string-ness is enforced
            // per-row at recompute (a non-string row fails closed there). A pinned NON-string
            // datatype (a number/bool via a range filter) rejects closed here.
            match child.relation.domains.get(index).copied().flatten() {
                None | Some(EqualityValueDomain::Datatype(CanonicalDatatype::String)) => {}
                Some(_) => {
                    return Err(BgpPlanError::ConcatOperandNotString {
                        var: variable.clone(),
                    })
                }
            }
            Ok(ConcatElement::Column { index })
        }
        // An xsd:string literal contributes its UTF-8 content bytes verbatim.
        DataValue::Data(XSDAnySimpleType::String(literal)) => {
            Ok(ConcatElement::Literal(literal.clone().into_bytes()))
        }
        // A non-string scalar literal (number/bool/…) would be STRINGIFIED by SWI
        // `interpolate_string` (`term_to_atom`/`atom_string`); that rendering is a separate
        // circuit, so reject closed rather than guess it.
        DataValue::Data(_) => Err(BgpPlanError::ConcatLiteralNotString),
        // A nested list element is not a scalar string operand.
        DataValue::List(_) => Err(BgpPlanError::ConcatNestedList),
    }
}

/// Compile a WOQL `Join` conjunct (`join(List, Sep, Result)`) into a verifier-recompute
/// [`CompiledRelationStatement::Join`] — a SEPARATOR-interposed extension of `plan_concat_form`.
///
/// V1 SCOPE (reject-closed everywhere else, mirroring concat): the list must be a CONCRETE inline
/// list of xsd:string LITERALS and/or variables bound (in the SAME query) as nonnullable
/// store-authenticated string columns; the SEPARATOR must be a string literal or such a bound
/// variable (a non-string separator would be `term_to_atom`-stringified by `interpolate_string`,
/// a separate circuit — reject); the result must be a FRESH variable. `join`'s reference mode is
/// `[ground, ground, any]` (`woql_compile.pl:2902`), matched by the concrete-list + string-sep +
/// fresh-result shape. Runtime list variable, non-string element/separator, unbound/pinned-
/// non-string operand, result literal/already-bound all reject closed.
///
/// The result variable is NOT added to the committed schema (terminal, revealed — exactly as
/// concat). Exposed post-verify via [`ProvedBgp::join_result_objects`].
fn plan_join_form(query: &Query) -> Result<Option<CompiledBgp>, BgpPlanError> {
    let conjuncts = match query {
        Query::And(and) => &and.and,
        _ => return Ok(None),
    };
    let positions: Vec<usize> = conjuncts
        .iter()
        .enumerate()
        .filter(|(_, conjunct)| matches!(conjunct, Query::Join(_)))
        .map(|(index, _)| index)
        .collect();
    if positions.is_empty() {
        return Ok(None);
    }
    if positions.len() > 1 {
        // Multiple joins would need a dependency-ordered chain of derived strings; v1 takes one.
        return Err(BgpPlanError::JoinUnsupportedShape);
    }
    let Query::Join(join) = &conjuncts[positions[0]] else {
        unreachable!("positions only indexes Join conjuncts")
    };

    // The operand-binding child is every OTHER conjunct.
    let child_conjuncts: Vec<Query> = conjuncts
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != positions[0])
        .map(|(_, conjunct)| conjunct.clone())
        .collect();
    if child_conjuncts.is_empty() {
        // A bare join with no store binding cannot authenticate any operand variable.
        return Err(BgpPlanError::JoinUnsupportedShape);
    }
    let child_query = if child_conjuncts.len() == 1 {
        child_conjuncts.into_iter().next().unwrap()
    } else {
        Query::And(And {
            and: child_conjuncts,
        })
    };
    let child = plan_bgp(&child_query)?;
    if child.projection.is_some() || child.count_variable.is_some() || child.full_row_distinct {
        return Err(BgpPlanError::JoinUnsupportedShape);
    }

    // The result must be a FRESH variable (never a literal, never a rebinding of an operand).
    let result = match &join.result_string {
        DataValue::Variable(variable) => variable.clone(),
        _ => return Err(BgpPlanError::JoinResultNotVariable),
    };
    if child.relation.schema.iter().any(|variable| variable == &result) {
        return Err(BgpPlanError::JoinResultAlreadyBound { var: result });
    }

    // The list must be a CONCRETE inline list (a runtime list variable's elements/order/string-ness
    // cannot be pinned into the query — reject closed).
    let items = match &join.list {
        ListOrVariable::List(items) => items,
        ListOrVariable::Variable(_) => return Err(BgpPlanError::JoinListNotConcrete),
    };
    if items.is_empty() {
        // `join([], Sep, "")` yields the empty string with no per-row derivation.
        return Err(BgpPlanError::JoinEmptyList);
    }
    // Elements and separator reuse concat-element lowering (string literal or authenticated string
    // column; non-string/unbound/pinned-non-string/nested-list all reject closed there). The
    // concat-element errors surfaced here are still accurate for join operands.
    let elements = items
        .iter()
        .map(|item| compile_concat_element(item, &child))
        .collect::<Result<Vec<_>, _>>()?;
    let separator = compile_concat_element(&join.separator, &child)?;

    let collection = child.collection.clone();
    let relation = CompiledRelation {
        schema: child.relation.schema.clone(),
        domains: child.relation.domains.clone(),
        statement: CompiledRelationStatement::Join {
            input: Box::new(child.relation),
            elements,
            separator,
            result,
        },
    };
    Ok(Some(CompiledBgp {
        relation,
        projection: None,
        collection,
        count_variable: None,
        full_row_distinct: false,
    }))
}

/// Extract a non-negative `usize` character index from a `Substring` position `DataValue`.
///
/// Accepts only an integer LITERAL (reusing [`eval_integer_literal`]'s exact-integer decode) that
/// is `>= 0`; a variable index (a store-authenticated per-row index — a heavier circuit) is a
/// documented follow-up and rejects closed, as does a non-integer literal and a NEGATIVE literal
/// (SWI `sub_string/5` requires non-negative offsets — a negative one fails/throws, so FAIL
/// CLOSED, matching the reference).
fn substring_index_literal(value: &DataValue) -> Result<usize, BgpPlanError> {
    let raw = match value {
        DataValue::Data(data) => {
            eval_integer_literal(data).map_err(|_| BgpPlanError::SubstringIndexNotInteger)?
        }
        DataValue::Variable(_) | DataValue::List(_) => {
            return Err(BgpPlanError::SubstringIndexNotLiteral)
        }
    };
    if raw < 0 {
        return Err(BgpPlanError::SubstringNegativeIndex);
    }
    usize::try_from(raw).map_err(|_| BgpPlanError::SubstringIndexTooLarge)
}

/// Resolve a `Substring` string operand to an authenticated child string column index. The operand
/// must be a VARIABLE bound (in the same query) as a nonnullable node/value column whose per-row
/// xsd:string-ness is enforced at recompute (an unpinned column) or pinned to xsd:string; a pinned
/// NON-string datatype, a nullable/node/predicate column, an unbound variable, or a literal all
/// reject closed (the task requires an AUTHENTICATED xsd:string input).
fn substring_string_column(value: &DataValue, child: &CompiledBgp) -> Result<usize, BgpPlanError> {
    match value {
        DataValue::Variable(variable) => {
            let index = child
                .relation
                .schema
                .iter()
                .position(|candidate| candidate == variable)
                .ok_or_else(|| BgpPlanError::SubstringStringNotBound {
                    var: variable.clone(),
                })?;
            let descriptors = relation_result_descriptors(&child.relation).map_err(|_| {
                BgpPlanError::SubstringStringNotAuthenticated {
                    var: variable.clone(),
                }
            })?;
            if descriptors[index].nullable
                || descriptors[index].namespace != ResultIdNamespace::NodeValue
            {
                return Err(BgpPlanError::SubstringStringNotAuthenticated {
                    var: variable.clone(),
                });
            }
            match child.relation.domains.get(index).copied().flatten() {
                None | Some(EqualityValueDomain::Datatype(CanonicalDatatype::String)) => {}
                Some(_) => {
                    return Err(BgpPlanError::SubstringStringNotAuthenticated {
                        var: variable.clone(),
                    })
                }
            }
            Ok(index)
        }
        // A literal string is a constant substring (no per-row read tying it to the trusted root);
        // the task requires an AUTHENTICATED input. Reject closed.
        _ => Err(BgpPlanError::SubstringStringNotVariable),
    }
}

/// Compile a WOQL `Substring` conjunct (`sub_string(String, Before, Length, After, SubString)`)
/// into a verifier-recompute [`CompiledRelationStatement::Substring`].
///
/// V1 SCOPE (reject-closed everywhere else): `String` an authenticated child xsd:string column
/// (variable); `Before`/`Length` NON-NEGATIVE integer LITERALS (the deterministic constant-index
/// mode we prove faithful — variable indices are a follow-up); `After` a FRESH variable (an
/// output — a bound `After` adds a length-equality/occurrence filter we do not model); `SubString`
/// a FRESH variable (the terminal revealed result), distinct from `After`. Out-of-range is handled
/// at recompute (FAIL CLOSED, matching SWI `sub_string`'s failure), so only the clean per-row-map
/// case ever produces a proof (see the `Substring` variant docs for the full fidelity argument).
///
/// Neither `After` nor `SubString` is added to the committed schema (terminal, revealed — exactly
/// as concat/join). The substring is exposed post-verify via
/// [`ProvedBgp::substring_result_objects`]; `After` is derived but NOT revealed.
fn plan_substring_form(query: &Query) -> Result<Option<CompiledBgp>, BgpPlanError> {
    let conjuncts = match query {
        Query::And(and) => &and.and,
        _ => return Ok(None),
    };
    let positions: Vec<usize> = conjuncts
        .iter()
        .enumerate()
        .filter(|(_, conjunct)| matches!(conjunct, Query::Substring(_)))
        .map(|(index, _)| index)
        .collect();
    if positions.is_empty() {
        return Ok(None);
    }
    if positions.len() > 1 {
        return Err(BgpPlanError::SubstringUnsupportedShape);
    }
    let Query::Substring(substring) = &conjuncts[positions[0]] else {
        unreachable!("positions only indexes Substring conjuncts")
    };

    let child_conjuncts: Vec<Query> = conjuncts
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != positions[0])
        .map(|(_, conjunct)| conjunct.clone())
        .collect();
    if child_conjuncts.is_empty() {
        return Err(BgpPlanError::SubstringUnsupportedShape);
    }
    let child_query = if child_conjuncts.len() == 1 {
        child_conjuncts.into_iter().next().unwrap()
    } else {
        Query::And(And {
            and: child_conjuncts,
        })
    };
    let child = plan_bgp(&child_query)?;
    if child.projection.is_some() || child.count_variable.is_some() || child.full_row_distinct {
        return Err(BgpPlanError::SubstringUnsupportedShape);
    }

    let string_column = substring_string_column(&substring.string, &child)?;
    let before = substring_index_literal(&substring.before)?;
    let length = substring_index_literal(&substring.length)?;

    // `After` must be a FRESH output variable (not a constant filter, not a rebinding of an
    // existing column).
    let after = match &substring.after {
        DataValue::Variable(variable) => variable.clone(),
        _ => return Err(BgpPlanError::SubstringAfterNotVariable),
    };
    if child.relation.schema.iter().any(|variable| variable == &after) {
        return Err(BgpPlanError::SubstringResultAlreadyBound { var: after });
    }

    // `SubString` (the result) must be a FRESH variable, distinct from `After`.
    let result = match &substring.substring {
        DataValue::Variable(variable) => variable.clone(),
        _ => return Err(BgpPlanError::SubstringResultNotVariable),
    };
    if child.relation.schema.iter().any(|variable| variable == &result) {
        return Err(BgpPlanError::SubstringResultAlreadyBound { var: result });
    }
    if result == after {
        return Err(BgpPlanError::SubstringResultAliasesAfter);
    }

    let collection = child.collection.clone();
    let relation = CompiledRelation {
        schema: child.relation.schema.clone(),
        domains: child.relation.domains.clone(),
        statement: CompiledRelationStatement::Substring {
            input: Box::new(child.relation),
            string_column,
            before,
            length,
            result,
        },
    };
    Ok(Some(CompiledBgp {
        relation,
        projection: None,
        collection,
        count_variable: None,
        full_row_distinct: false,
    }))
}

/// Resolve a `Pad` string operand to an authenticated child string column index. Same discipline
/// as [`substring_string_column`]: the operand must be a VARIABLE bound (in the same query) as a
/// nonnullable node/value column whose per-row xsd:string-ness is enforced at recompute (unpinned)
/// or pinned to xsd:string; a pinned NON-string datatype, a nullable/node column, an unbound
/// variable, or a literal all reject closed (the task requires an AUTHENTICATED xsd:string input).
fn pad_string_column(value: &DataValue, child: &CompiledBgp) -> Result<usize, BgpPlanError> {
    match value {
        DataValue::Variable(variable) => {
            let index = child
                .relation
                .schema
                .iter()
                .position(|candidate| candidate == variable)
                .ok_or_else(|| BgpPlanError::PadStringNotBound {
                    var: variable.clone(),
                })?;
            let descriptors = relation_result_descriptors(&child.relation).map_err(|_| {
                BgpPlanError::PadStringNotAuthenticated {
                    var: variable.clone(),
                }
            })?;
            if descriptors[index].nullable
                || descriptors[index].namespace != ResultIdNamespace::NodeValue
            {
                return Err(BgpPlanError::PadStringNotAuthenticated {
                    var: variable.clone(),
                });
            }
            match child.relation.domains.get(index).copied().flatten() {
                None | Some(EqualityValueDomain::Datatype(CanonicalDatatype::String)) => {}
                Some(_) => {
                    return Err(BgpPlanError::PadStringNotAuthenticated {
                        var: variable.clone(),
                    })
                }
            }
            Ok(index)
        }
        // A literal string is a constant with no per-row read tying it to the trusted root; the
        // task requires an AUTHENTICATED input. Reject closed.
        _ => Err(BgpPlanError::PadStringNotVariable),
    }
}

/// Resolve a `Pad` pad-char operand to its UTF-8 bytes. Reference `utils:pad` requires the char to
/// interpolate to a string of `string_length == 1` (exactly ONE code point) else it throws
/// `pad_not_length_one`. A variable char is a deferred follow-up; a non-string is unsupported.
fn pad_char_literal(value: &DataValue) -> Result<Vec<u8>, BgpPlanError> {
    match value {
        DataValue::Data(XSDAnySimpleType::String(literal)) => {
            // CODE-POINT count, not byte length: a multi-BYTE single char (e.g. 'é') is length 1.
            if literal.chars().count() != 1 {
                return Err(BgpPlanError::PadCharNotSingleChar);
            }
            Ok(literal.clone().into_bytes())
        }
        DataValue::Data(_) => Err(BgpPlanError::PadCharNotString),
        DataValue::List(_) => Err(BgpPlanError::PadCharNotString),
        DataValue::Variable(_) => Err(BgpPlanError::PadCharNotLiteral),
    }
}

/// Resolve a `Pad` target-length operand to a non-negative `usize`. A negative `L` always throws
/// `bad_pad_length` in the reference (`L2 = L - L1 < 0` for any `L1 >= 0`), so it rejects closed. A
/// variable target length is a deferred follow-up.
fn pad_target_length(value: &DataValue) -> Result<usize, BgpPlanError> {
    let raw = match value {
        DataValue::Data(data) => {
            eval_integer_literal(data).map_err(|_| BgpPlanError::PadLengthNotInteger)?
        }
        DataValue::Variable(_) | DataValue::List(_) => {
            return Err(BgpPlanError::PadLengthNotLiteral)
        }
    };
    if raw < 0 {
        return Err(BgpPlanError::PadNegativeLength);
    }
    usize::try_from(raw).map_err(|_| BgpPlanError::PadLengthTooLarge)
}

/// Compile a WOQL `Pad` conjunct (`pad(String, Char, Times, Result)`) into a verifier-recompute
/// [`CompiledRelationStatement::Pad`].
///
/// V1 SCOPE (reject-closed everywhere else): `String` an authenticated child xsd:string column
/// (variable); `Char` a single-code-point xsd:string LITERAL; `Times` (semantically the TARGET
/// total CHARACTER length `L`) a NON-NEGATIVE integer LITERAL (a variable char/length is a
/// follow-up); `Result` a FRESH variable (the terminal revealed result). Operand-longer-than-target
/// is handled at recompute (FAIL CLOSED, matching `utils:pad`'s `bad_pad_length` throw), so only
/// the clean per-row-map case ever produces a proof. The result is NOT added to the committed
/// schema (terminal, revealed — exactly as concat/substring); it is exposed post-verify via
/// [`ProvedBgp::pad_result_objects`].
fn plan_pad_form(query: &Query) -> Result<Option<CompiledBgp>, BgpPlanError> {
    let conjuncts = match query {
        Query::And(and) => &and.and,
        _ => return Ok(None),
    };
    let positions: Vec<usize> = conjuncts
        .iter()
        .enumerate()
        .filter(|(_, conjunct)| matches!(conjunct, Query::Pad(_)))
        .map(|(index, _)| index)
        .collect();
    if positions.is_empty() {
        return Ok(None);
    }
    if positions.len() > 1 {
        return Err(BgpPlanError::PadUnsupportedShape);
    }
    let Query::Pad(pad) = &conjuncts[positions[0]] else {
        unreachable!("positions only indexes Pad conjuncts")
    };

    let child_conjuncts: Vec<Query> = conjuncts
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != positions[0])
        .map(|(_, conjunct)| conjunct.clone())
        .collect();
    if child_conjuncts.is_empty() {
        return Err(BgpPlanError::PadUnsupportedShape);
    }
    let child_query = if child_conjuncts.len() == 1 {
        child_conjuncts.into_iter().next().unwrap()
    } else {
        Query::And(And {
            and: child_conjuncts,
        })
    };
    let child = plan_bgp(&child_query)?;
    if child.projection.is_some() || child.count_variable.is_some() || child.full_row_distinct {
        return Err(BgpPlanError::PadUnsupportedShape);
    }

    let string_column = pad_string_column(&pad.string, &child)?;
    let pad_char = pad_char_literal(&pad.char)?;
    let target_len = pad_target_length(&pad.times)?;

    let result = match &pad.result_string {
        DataValue::Variable(variable) => variable.clone(),
        _ => return Err(BgpPlanError::PadResultNotVariable),
    };
    if child.relation.schema.iter().any(|variable| variable == &result) {
        return Err(BgpPlanError::PadResultAlreadyBound { var: result });
    }

    let collection = child.collection.clone();
    let relation = CompiledRelation {
        schema: child.relation.schema.clone(),
        domains: child.relation.domains.clone(),
        statement: CompiledRelationStatement::Pad {
            input: Box::new(child.relation),
            string_column,
            pad_char,
            target_len,
            result,
        },
    };
    Ok(Some(CompiledBgp {
        relation,
        projection: None,
        collection,
        count_variable: None,
        full_row_distinct: false,
    }))
}

/// Compile a WOQL `HashKey`/`hash` conjunct (`hash(Base, KeyList, Uri)`) into a verifier-recompute
/// [`CompiledRelationStatement::HashKey`].
///
/// V1 SCOPE (reject-closed everywhere else): `Base` an authenticated child xsd:string column or an
/// xsd:string LITERAL; each `KeyList` element likewise (authenticated column or string literal —
/// reusing the [`compile_concat_element`] auth, so non-string/unbound/nested-list elements reject
/// closed with the shared `Concat*` operand errors); `Uri` a FRESH variable (the terminal revealed
/// result). `KeyList` is ALWAYS a concrete `Vec<DataValue>` in the AST (there is no runtime list
/// variable to reject). The result is NOT added to the committed schema (terminal, revealed); it is
/// exposed post-verify via [`ProvedBgp::hashkey_result_objects`] as a `CanonicalObject::Node`.
fn plan_hashkey_form(query: &Query) -> Result<Option<CompiledBgp>, BgpPlanError> {
    let conjuncts = match query {
        Query::And(and) => &and.and,
        _ => return Ok(None),
    };
    let positions: Vec<usize> = conjuncts
        .iter()
        .enumerate()
        .filter(|(_, conjunct)| matches!(conjunct, Query::HashKey(_)))
        .map(|(index, _)| index)
        .collect();
    if positions.is_empty() {
        return Ok(None);
    }
    if positions.len() > 1 {
        return Err(BgpPlanError::HashKeyUnsupportedShape);
    }
    let Query::HashKey(hash) = &conjuncts[positions[0]] else {
        unreachable!("positions only indexes HashKey conjuncts")
    };

    let child_conjuncts: Vec<Query> = conjuncts
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != positions[0])
        .map(|(_, conjunct)| conjunct.clone())
        .collect();
    if child_conjuncts.is_empty() {
        return Err(BgpPlanError::HashKeyUnsupportedShape);
    }
    let child_query = if child_conjuncts.len() == 1 {
        child_conjuncts.into_iter().next().unwrap()
    } else {
        Query::And(And {
            and: child_conjuncts,
        })
    };
    let child = plan_bgp(&child_query)?;
    if child.projection.is_some() || child.count_variable.is_some() || child.full_row_distinct {
        return Err(BgpPlanError::HashKeyUnsupportedShape);
    }

    // Base + args authenticated via the reusable ConcatElement lowering (Concat* operand errors on
    // an unbound/non-string/nested-list operand). Base is later RAW-prepended; args are encoded.
    let base = compile_concat_element(&hash.base, &child)?;
    let args = hash
        .key_list
        .iter()
        .map(|item| compile_concat_element(item, &child))
        .collect::<Result<Vec<_>, _>>()?;

    // Result URI must be a FRESH variable (a constant node result would add an equality constraint).
    let result = match &hash.uri {
        NodeValue::Variable(variable) => variable.clone(),
        NodeValue::Node(_) => return Err(BgpPlanError::HashKeyResultNotVariable),
    };
    if child.relation.schema.iter().any(|variable| variable == &result) {
        return Err(BgpPlanError::HashKeyResultAlreadyBound { var: result });
    }

    let collection = child.collection.clone();
    let relation = CompiledRelation {
        schema: child.relation.schema.clone(),
        domains: child.relation.domains.clone(),
        statement: CompiledRelationStatement::HashKey {
            input: Box::new(child.relation),
            base,
            args,
            result,
        },
    };
    Ok(Some(CompiledBgp {
        relation,
        projection: None,
        collection,
        count_variable: None,
        full_row_distinct: false,
    }))
}

/// Reject-closed for the DEFERRED string-case operators `Upper`/`Lower`.
///
/// SEMANTIC-FIDELITY DEFERRAL (soundness-first): WOQL `upper`/`lower` compile to SWI-Prolog
/// `string_upper`/`string_lower`, which fold case per code point via C `towupper`/`towlower` —
/// LOCALE-DEPENDENT (`LC_CTYPE`). TerminusDB's canonical Docker deployment pins NO locale
/// (no `LANG`/`LC_ALL`/`LC_CTYPE` in the Dockerfiles or `start.pl`; SWI inherits the environment
/// locale), so under a Turkic `LC_CTYPE` `string_upper("i")` yields "İ" (U+0130) and
/// `string_lower("I")` yields "ı" (U+0131) — a NON-ASCII result diverging from a naive ASCII
/// a–z↔A–Z map EVEN ON ASCII INPUT 'i'/'I'. A "reject non-ASCII INPUT" guard therefore does NOT
/// establish fidelity. We cannot prove agreement with Rust ASCII casing across deployments, so
/// we REJECT CLOSED here and defer upper/lower to a follow-up that pins the runtime locale (e.g.
/// `LC_ALL=C`) or proves the case fold directly. (`concat`/`interpolate_string` is unaffected:
/// it is pure `string_concat`, locale-independent — and IS shipped.)
fn reject_unsupported_string_case_ops(query: &Query) -> Result<(), BgpPlanError> {
    fn is_case_op(query: &Query) -> bool {
        matches!(query, Query::Upper(_) | Query::Lower(_))
    }
    let present = match query {
        Query::And(and) => and.and.iter().any(is_case_op),
        other => is_case_op(other),
    };
    if present {
        return Err(BgpPlanError::StringCaseOpLocaleUnsupported);
    }
    Ok(())
}

fn plan_collection_count_form(query: &Query) -> Result<Option<CompiledBgp>, BgpPlanError> {
    let conjuncts = match query {
        Query::And(and) if and.and.len() >= 2 => &and.and,
        _ => return Ok(None),
    };
    if conjuncts.iter().skip(2).any(|query| {
        matches!(query, Query::If(_) | Query::WoqlOptional(_) | Query::Not(_))
    }) {
        return Ok(None);
    }
    let (Query::Collect(collect), Query::Length(length)) = (&conjuncts[0], &conjuncts[1]) else {
        if matches!(conjuncts.first(), Some(Query::GroupBy(_))) {
            return Ok(None);
        }
        if conjuncts
            .iter()
            .any(|query| matches!(query, Query::Collect(_) | Query::Length(_)))
        {
            return Err(BgpPlanError::CollectionCountUnsupportedShape);
        }
        return Ok(None);
    };
    let hidden = match &collect.into {
        Value::Variable(variable) => variable,
        _ => return Err(BgpPlanError::CollectionCountUnsupportedShape),
    };
    let list = match &length.list {
        DataValue::Variable(variable) => variable,
        _ => return Err(BgpPlanError::CollectionCountUnsupportedShape),
    };
    let result = match &length.length {
        DataValue::Variable(variable) => variable,
        _ => return Err(BgpPlanError::CollectionCountUnsupportedShape),
    };
    if hidden != list || hidden == result {
        return Err(BgpPlanError::CollectionCountUnsupportedShape);
    }
    if matches!(&collect.template, Value::Variable(variable) if variable == hidden || variable == result)
    {
        return Err(BgpPlanError::CollectionCountUnsupportedShape);
    }
    let child = plan_bgp(&collect.query)?;
    if child.projection.is_some() || child.count_variable.is_some() || child.full_row_distinct {
        return Err(BgpPlanError::CollectionCountUnsupportedShape);
    }
    if child
        .relation
        .schema
        .iter()
        .any(|variable| variable == hidden || variable == result)
    {
        return Err(BgpPlanError::CollectionCountUnsupportedShape);
    }
    let collection = child.collection;
    let mut relation = CompiledRelation {
        schema: vec![result.clone()],
        domains: vec![None],
        statement: CompiledRelationStatement::CollectionCount {
            input: Box::new(child.relation),
        },
    };
    for comparison in conjuncts.iter().skip(2) {
        let (op, domain, constant, constant_on_left) =
            generated_group_sum_constraint(comparison, result, false)?;
        if domain != ResultIdNamespace::GeneratedDecimalU64 {
            return Err(BgpPlanError::GroupSumComparisonUnsupportedConstant);
        }
        let input = relation;
        relation = CompiledRelation {
            schema: input.schema.clone(),
            domains: input.domains.clone(),
            statement: CompiledRelationStatement::GeneratedU64Filter {
                input: Box::new(input),
                column: 0,
                op,
                constant: u64::try_from(constant)
                    .map_err(|_| BgpPlanError::GroupSumComparisonUnsupportedConstant)?,
                constant_on_left,
            },
        };
    }
    Ok(Some(CompiledBgp {
        relation,
        projection: None,
        collection,
        count_variable: None,
        full_row_distinct: false,
    }))
}

fn plan_group_count_form(query: &Query) -> Result<Option<CompiledBgp>, BgpPlanError> {
    let conjuncts = match query {
        Query::And(and) if and.and.len() >= 2 => &and.and,
        _ => return Ok(None),
    };
    if conjuncts.iter().skip(2).any(|query| {
        matches!(query, Query::If(_) | Query::WoqlOptional(_) | Query::Not(_))
    }) {
        return Ok(None);
    }
    let (Query::GroupBy(group), Query::Length(length)) = (&conjuncts[0], &conjuncts[1]) else {
        if conjuncts.iter().any(|query| matches!(query, Query::Length(_))) {
            return Err(BgpPlanError::GroupCountUnsupportedShape);
        }
        return Ok(None);
    };
    if group.group_by.len() > 2
        || (group.group_by.len() == 2 && group.group_by[0] == group.group_by[1])
    {
        return Err(BgpPlanError::GroupCountUnsupportedShape);
    }
    let template = match &group.template {
        Value::Variable(variable) => variable,
        _ => return Err(BgpPlanError::GroupCountUnsupportedValue),
    };
    let hidden = match &group.grouped_value {
        Value::Variable(variable) => variable,
        _ => return Err(BgpPlanError::GroupCountUnsupportedShape),
    };
    let list = match &length.list {
        DataValue::Variable(variable) => variable,
        _ => return Err(BgpPlanError::GroupCountUnsupportedShape),
    };
    let result = match &length.length {
        DataValue::Variable(variable) => variable,
        _ => return Err(BgpPlanError::GroupCountUnsupportedShape),
    };
    if hidden != list
        || group
            .group_by
            .iter()
            .any(|key| key == template || key == hidden || key == result)
        || template == hidden
        || template == result
        || hidden == result
    {
        return Err(BgpPlanError::GroupCountUnsupportedShape);
    }
    let child = plan_bgp(&group.query)?;
    if child.projection.is_some() || child.count_variable.is_some() || child.full_row_distinct {
        return Err(BgpPlanError::GroupCountUnsupportedShape);
    }
    let key_columns = group
        .group_by
        .iter()
        .map(|key| {
            child
                .relation
                .schema
                .iter()
                .position(|variable| variable == key)
                .ok_or(BgpPlanError::GroupCountUnsupportedShape)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let template_column = child
        .relation
        .schema
        .iter()
        .position(|variable| variable == template)
        .ok_or(BgpPlanError::GroupCountUnsupportedValue)?;
    if key_columns.iter().any(|key_column| {
        *key_column == template_column
            || child.relation.domains.get(*key_column).copied().flatten()
                != Some(EqualityValueDomain::Node)
    }) {
        return Err(BgpPlanError::GroupCountUnsupportedShape);
    }
    let descriptors = relation_result_descriptors(&child.relation)
        .map_err(|_| BgpPlanError::GroupCountUnsupportedShape)?;
    if key_columns.iter().any(|key_column| {
        descriptors[*key_column].namespace != ResultIdNamespace::NodeValue
            || descriptors[*key_column].nullable
    }) || descriptors[template_column].nullable
        || !matches!(
            descriptors[template_column].namespace,
            ResultIdNamespace::NodeValue | ResultIdNamespace::Predicate
        )
    {
        return Err(BgpPlanError::GroupCountUnsupportedValue);
    }
    let collection = child.collection;
    let mut schema = Vec::with_capacity(key_columns.len() + 1);
    let mut domains = Vec::with_capacity(schema.capacity());
    for (key, key_column) in group.group_by.iter().zip(&key_columns) {
        schema.push(key.clone());
        domains.push(child.relation.domains[*key_column]);
    }
    schema.push(result.clone());
    domains.push(None);
    let mut relation = CompiledRelation {
        schema,
        domains,
        statement: CompiledRelationStatement::GroupCount {
            input: Box::new(child.relation),
            key_columns: key_columns.clone(),
        },
    };
    for comparison in conjuncts.iter().skip(2) {
        let (op, domain, constant, constant_on_left) =
            generated_group_sum_constraint(comparison, result, false)?;
        if domain != ResultIdNamespace::GeneratedDecimalU64 {
            return Err(BgpPlanError::GroupSumComparisonUnsupportedConstant);
        }
        let input = relation;
        relation = CompiledRelation {
            schema: input.schema.clone(),
            domains: input.domains.clone(),
            statement: CompiledRelationStatement::GeneratedU64Filter {
                input: Box::new(input),
                column: key_columns.len(),
                op,
                constant: u64::try_from(constant)
                    .map_err(|_| BgpPlanError::GroupSumComparisonUnsupportedConstant)?,
                constant_on_left,
            },
        };
    }
    Ok(Some(CompiledBgp {
        relation,
        projection: None,
        collection,
        count_variable: None,
        full_row_distinct: false,
    }))
}

fn plan_group_sum_form(query: &Query) -> Result<Option<CompiledBgp>, BgpPlanError> {
    let conjuncts = match query {
        Query::And(and) if and.and.len() >= 2 => &and.and,
        _ => return Ok(None),
    };
    // Let the general sequential-relation planner compose supported When/Optional/Not
    // operators over the aggregate prefix. Equals and range comparisons remain here so
    // generated-decimal domains are handled without Store-ID coercion.
    if conjuncts.iter().skip(2).any(|query| {
        matches!(query, Query::If(_) | Query::WoqlOptional(_) | Query::Not(_))
    }) {
        return Ok(None);
    }
    let (Query::GroupBy(group), Query::Sum(sum)) = (&conjuncts[0], &conjuncts[1]) else {
        if conjuncts.iter().any(|query| matches!(query, Query::Sum(_))) {
            return Err(BgpPlanError::GroupSumUnsupportedShape);
        }
        return Ok(None);
    };
    if group.group_by.len() > 2
        || (group.group_by.len() == 2 && group.group_by[0] == group.group_by[1])
    {
        return Err(BgpPlanError::GroupSumUnsupportedShape);
    }
    let template = match &group.template {
        Value::Variable(variable) => variable,
        _ => return Err(BgpPlanError::GroupSumUnsupportedValue),
    };
    let hidden = match &group.grouped_value {
        Value::Variable(variable) => variable,
        _ => return Err(BgpPlanError::GroupSumUnsupportedShape),
    };
    let list = match &sum.list {
        DataValue::Variable(variable) => variable,
        _ => return Err(BgpPlanError::GroupSumUnsupportedShape),
    };
    let result = match &sum.result {
        DataValue::Variable(variable) => variable,
        _ => return Err(BgpPlanError::GroupSumUnsupportedShape),
    };
    if hidden != list
        || group
            .group_by
            .iter()
            .any(|key| key == template || key == hidden || key == result)
        || template == hidden
        || template == result
        || hidden == result
    {
        return Err(BgpPlanError::GroupSumUnsupportedShape);
    }
    let child = plan_bgp(&group.query)?;
    if child.projection.is_some() || child.count_variable.is_some() || child.full_row_distinct {
        return Err(BgpPlanError::GroupSumUnsupportedShape);
    }
    let key_columns = group
        .group_by
        .iter()
        .map(|key| {
            child
                .relation
                .schema
                .iter()
                .position(|variable| variable == key)
                .ok_or(BgpPlanError::GroupSumUnsupportedShape)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let value_column = child
        .relation
        .schema
        .iter()
        .position(|variable| variable == template)
        .ok_or(BgpPlanError::GroupSumUnsupportedValue)?;
    if key_columns.iter().any(|key_column| {
        *key_column == value_column
            || child.relation.domains.get(*key_column).copied().flatten()
                != Some(EqualityValueDomain::Node)
    }) {
        return Err(BgpPlanError::GroupSumUnsupportedShape);
    }
    let descriptors = relation_result_descriptors(&child.relation)
        .map_err(|_| BgpPlanError::GroupSumUnsupportedShape)?;
    if key_columns.iter().any(|key_column| {
        descriptors[*key_column].namespace != ResultIdNamespace::NodeValue
            || descriptors[*key_column].nullable
    }) || descriptors[value_column].namespace != ResultIdNamespace::NodeValue
        || descriptors[value_column].nullable
    {
        return Err(BgpPlanError::GroupSumUnsupportedShape);
    }
    let datatype = match child.relation.domains.get(value_column).copied().flatten() {
        Some(EqualityValueDomain::Datatype(
            datatype @ (CanonicalDatatype::UInt8
            | CanonicalDatatype::UInt16
            | CanonicalDatatype::UInt32
            | CanonicalDatatype::UInt64
            | CanonicalDatatype::Int8
            | CanonicalDatatype::Int16
            | CanonicalDatatype::Int32
            | CanonicalDatatype::Int64),
        )) => datatype,
        _ => return Err(BgpPlanError::GroupSumUnsupportedValue),
    };
    let collection = child.collection;
    let mut schema = Vec::with_capacity(key_columns.len() + 1);
    let mut domains = Vec::with_capacity(schema.capacity());
    for (key, key_column) in group.group_by.iter().zip(&key_columns) {
        schema.push(key.clone());
        domains.push(child.relation.domains[*key_column]);
    }
    schema.push(result.clone());
    domains.push(None);
    let signed = matches!(
        datatype,
        CanonicalDatatype::Int8
            | CanonicalDatatype::Int16
            | CanonicalDatatype::Int32
            | CanonicalDatatype::Int64
    );
    let mut relation = CompiledRelation {
            schema,
            domains,
            statement: CompiledRelationStatement::GroupSum {
                input: Box::new(child.relation),
                key_columns: key_columns.clone(),
                value_column,
                datatype,
            },
        };
    for comparison in conjuncts.iter().skip(2) {
        let (op, domain, constant, constant_on_left) =
            generated_group_sum_constraint(comparison, result, signed)?;
        let input = relation;
        relation = CompiledRelation {
            schema: input.schema.clone(),
            domains: input.domains.clone(),
            statement: match domain {
                ResultIdNamespace::GeneratedDecimalU64 => {
                    CompiledRelationStatement::GeneratedU64Filter {
                        input: Box::new(input),
                        column: key_columns.len(),
                        op,
                        constant: u64::try_from(constant).map_err(|_| {
                            BgpPlanError::GroupSumComparisonUnsupportedConstant
                        })?,
                        constant_on_left,
                    }
                }
                ResultIdNamespace::GeneratedDecimalI64 => {
                    CompiledRelationStatement::GeneratedI64Filter {
                        input: Box::new(input),
                        column: key_columns.len(),
                        op,
                        constant: i64::try_from(constant).map_err(|_| {
                            BgpPlanError::GroupSumComparisonUnsupportedConstant
                        })?,
                        constant_on_left,
                    }
                }
                _ => unreachable!(),
            },
        };
    }
    Ok(Some(CompiledBgp {
        relation,
        projection: None,
        collection,
        count_variable: None,
        full_row_distinct: false,
    }))
}

fn plan_exact_scalar_order(
    order_by: &crate::order::OrderBy,
) -> Result<CompiledBgp, BgpPlanError> {
    if !(1..=3).contains(&order_by.ordering.len()) {
        return Err(BgpPlanError::OrderByUnsupportedShape);
    }
    let child = plan_bgp(&order_by.query)?;
    if child.projection.is_some()
        || child.count_variable.is_some()
        || child.full_row_distinct
        || child.relation.schema.len() != order_by.ordering.len()
    {
        return Err(BgpPlanError::OrderByUnsupportedShape);
    }
    let key_indices = order_by.ordering.iter()
        .map(|ordering| child.relation.schema.iter().position(|schema| schema == &ordering.variable))
        .collect::<Option<Vec<_>>>()
        .ok_or(BgpPlanError::OrderByUnsupportedShape)?;
    let mut complete_key_set = key_indices.clone();
    complete_key_set.sort_unstable();
    if complete_key_set != (0..child.relation.schema.len()).collect::<Vec<_>>() {
        return Err(BgpPlanError::OrderByUnsupportedShape);
    }
    let descriptor = relation_result_descriptors(&child.relation)
        .map_err(|_| BgpPlanError::OrderByUnsupportedShape)?;
    if descriptor.iter().any(|descriptor| descriptor.namespace != ResultIdNamespace::NodeValue || descriptor.nullable) {
        return Err(BgpPlanError::OrderByUnsupportedValue);
    }
    let datatypes = child.relation.domains.iter().map(|domain| match domain {
        Some(EqualityValueDomain::Datatype(datatype)) if matches!(datatype,
                CanonicalDatatype::Boolean
                    | CanonicalDatatype::UInt8
                    | CanonicalDatatype::Int8
                    | CanonicalDatatype::UInt16
                    | CanonicalDatatype::Int16
                    | CanonicalDatatype::UInt32
                    | CanonicalDatatype::Int32
                    | CanonicalDatatype::Float32
                    | CanonicalDatatype::UInt64
                    | CanonicalDatatype::Int64
                    | CanonicalDatatype::Float64
            ) => Ok(*datatype),
        _ => return Err(BgpPlanError::OrderByUnsupportedValue),
    }).collect::<Result<Vec<_>, _>>()?;
    let directions = order_by.ordering.iter().map(|ordering| match ordering.order {
        crate::order::Order::Asc => OrderDirection::Ascending,
        crate::order::Order::Desc => OrderDirection::Descending,
    }).collect::<Vec<_>>();
    let schema = child.relation.schema.clone();
    let domains = child.relation.domains.clone();
    let collection = child.collection;
    let statement = if datatypes.len() == 1 {
        CompiledRelationStatement::ExactScalarOrder { input: Box::new(child.relation), datatype: datatypes[0], direction: directions[0] }
    } else {
        CompiledRelationStatement::ExactMultiOrder { input: Box::new(child.relation), key_indices, datatypes, directions }
    };
    Ok(CompiledBgp {
        relation: CompiledRelation {
            schema,
            domains,
            statement,
        },
        projection: None,
        collection,
        count_variable: None,
        full_row_distinct: false,
    })
}

fn plan_exact_ordered_slice(query: &Query) -> Result<CompiledBgp, BgpPlanError> {
    let (child_query, offset, limit, once) = match query {
        Query::Start(start) => (start.query.as_ref(), start.start, None, false),
        Query::Limit(limit) => (limit.query.as_ref(), 0, Some(limit.limit), false),
        Query::Once(once) => (once.query.as_ref(), 0, Some(1), true),
        _ => return Err(BgpPlanError::OrderedSliceUnsupportedShape),
    };
    let child = plan_bgp(child_query)?;
    if child.projection.is_some()
        || child.count_variable.is_some()
        || child.full_row_distinct
        || !matches!(
            child.relation.statement,
            CompiledRelationStatement::ExactScalarOrder { .. }
                | CompiledRelationStatement::ExactMultiOrder { .. }
                | CompiledRelationStatement::ExactOrderedSlice { .. }
        )
    {
        return Err(BgpPlanError::OrderedSliceUnsupportedShape);
    }
    let collection = child.collection;
    Ok(CompiledBgp {
        relation: CompiledRelation {
            schema: child.relation.schema.clone(),
            domains: child.relation.domains.clone(),
            statement: CompiledRelationStatement::ExactOrderedSlice {
                input: Box::new(child.relation),
                offset,
                limit,
                once,
            },
        },
        projection: None,
        collection,
        count_variable: None,
        full_row_distinct: false,
    })
}

fn encoded_ordered_triple_value(
    value: &Value,
) -> Result<(ArkFr, ArkFr, CanonicalDatatype), BgpPlanError> {
    let Value::Data(value) = value else {
        return Err(BgpPlanError::TripleValueOrderUnsupportedValue);
    };
    let encoded = encode_range_constant(value)
        .map_err(|_| BgpPlanError::TripleValueOrderUnsupportedValue)?;
    if !matches!(
        encoded.2,
        CanonicalDatatype::UInt8
            | CanonicalDatatype::Int8
            | CanonicalDatatype::UInt16
            | CanonicalDatatype::Int16
            | CanonicalDatatype::UInt32
            | CanonicalDatatype::Int32
            | CanonicalDatatype::Float32
            | CanonicalDatatype::UInt64
            | CanonicalDatatype::Int64
            | CanonicalDatatype::Float64
    ) {
        return Err(BgpPlanError::TripleValueOrderUnsupportedValue);
    }
    Ok(encoded)
}

fn compile_ordered_triple_value_scan(
    subject: &NodeValue,
    predicate: &NodeValue,
    graph: &Option<GraphType>,
    variable: &str,
    datatype: ArkFr,
    canonical_datatype: CanonicalDatatype,
    bounds: Vec<NormalizedBound>,
    direction: OrderDirection,
    first_only: bool,
) -> Result<CompiledBgp, BgpPlanError> {
    if !matches!(subject, NodeValue::Node(_))
        || !matches!(predicate, NodeValue::Node(_))
        || matches!(graph, Some(GraphType::Schema))
    {
        return Err(BgpPlanError::TripleValueOrderUnsupportedShape);
    }

    let typed = TypedTriple {
        triple: Triple {
            subject: subject.clone(),
            predicate: predicate.clone(),
            object: Value::Variable(variable.to_owned()),
            graph: graph.clone(),
        },
        object_kind: ObjectKindFilter::Any,
        source: ColumnSource::Materialized,
    };
    let mut atoms = vec![atom_from_triple(&typed)?];
    let comparisons = vec![NormalizedRange {
        var: variable.to_owned(),
        datatype,
        canonical_datatype,
        bounds,
    }];
    attach_ranges(&mut atoms, &comparisons)?;
    let (plan, schema) = plan_left_deep(&atoms)?;
    if schema != [variable] {
        return Err(BgpPlanError::TripleValueOrderUnsupportedShape);
    }
    let ordered = CompiledRelation {
        schema: schema.clone(),
        domains: vec![Some(EqualityValueDomain::Datatype(canonical_datatype))],
        statement: CompiledRelationStatement::ExactScalarOrder {
            input: Box::new(CompiledRelation {
                schema: schema.clone(),
                domains: vec![Some(EqualityValueDomain::Datatype(canonical_datatype))],
                statement: CompiledRelationStatement::Bgp(plan),
            }),
            datatype: canonical_datatype,
            direction,
        },
    };
    let relation = if first_only {
        CompiledRelation {
            schema,
            domains: vec![Some(EqualityValueDomain::Datatype(canonical_datatype))],
            statement: CompiledRelationStatement::ExactOrderedSlice {
                input: Box::new(ordered),
                offset: 0,
                limit: Some(1),
                once: false,
            },
        }
    } else {
        ordered
    };
    Ok(CompiledBgp {
        relation,
        projection: None,
        collection: None,
        count_variable: None,
        full_row_distinct: false,
    })
}

fn compile_ordered_triple_slice(
    subject: &NodeValue,
    predicate: &NodeValue,
    object: &Value,
    low: &Value,
    high: &Value,
    graph: &Option<GraphType>,
    direction: OrderDirection,
) -> Result<CompiledBgp, BgpPlanError> {
    let Value::Variable(variable) = object else {
        return Err(BgpPlanError::TripleValueOrderUnsupportedShape);
    };
    let (low_datatype, low_c, low_canonical) = encoded_ordered_triple_value(low)?;
    let (high_datatype, high_c, high_canonical) = encoded_ordered_triple_value(high)?;
    if low_datatype != high_datatype || low_canonical != high_canonical {
        return Err(BgpPlanError::TripleValueOrderUnsupportedValue);
    }
    compile_ordered_triple_value_scan(
        subject,
        predicate,
        graph,
        variable,
        low_datatype,
        low_canonical,
        vec![
            NormalizedBound {
                op: RangeOp::Ge,
                c: low_c,
            },
            NormalizedBound {
                op: RangeOp::Lt,
                c: high_c,
            },
        ],
        direction,
        false,
    )
}

fn constrain_ordered_triple_value_to_ground(
    compiled: CompiledBgp,
    variable: &str,
    expected: &Value,
) -> Result<CompiledBgp, BgpPlanError> {
    let Value::Data(expected) = expected else {
        return Err(BgpPlanError::TripleValueOrderUnsupportedValue);
    };
    let equals = crate::compare::Equals {
        left: Value::Variable(variable.to_owned()),
        right: Value::Data(expected.clone()),
    };
    let constraint = compile_equality_constraint(&compiled.relation, &equals)?;
    let equality = CompiledRelation {
        schema: compiled.relation.schema.clone(),
        domains: compiled.relation.domains.clone(),
        statement: CompiledRelationStatement::Equality {
            input: Box::new(compiled.relation),
            constraint,
        },
    };
    Ok(CompiledBgp {
        relation: CompiledRelation {
            schema: Vec::new(),
            domains: Vec::new(),
            statement: CompiledRelationStatement::BagProjection {
                input: Box::new(equality),
                indices: Vec::new(),
            },
        },
        projection: Some(Vec::new()),
        collection: compiled.collection,
        count_variable: None,
        full_row_distinct: false,
    })
}

/// Compile the largest currently exact ordered-value subset shared by the four
/// TerminusDB 12 cursor constructs.  Constant S/P leaves one complete object column,
/// so the existing range, exact-order, and exact-slice arguments prove both membership
/// and the executor-visible order without relying on physical dictionary IDs.
fn plan_ordered_triple_value_construct(
    query: &Query,
) -> Result<Option<CompiledBgp>, BgpPlanError> {
    let compiled = match query {
        Query::TripleSlice(slice) => match &slice.object {
            Value::Variable(_) => compile_ordered_triple_slice(
                &slice.subject,
                &slice.predicate,
                &slice.object,
                &slice.low,
                &slice.high,
                &slice.graph,
                OrderDirection::Ascending,
            )?,
            Value::Data(_) => {
                const HIDDEN: &str = "__woql_proof_ground_triple_value";
                let hidden = Value::Variable(HIDDEN.to_owned());
                let compiled = compile_ordered_triple_slice(
                    &slice.subject,
                    &slice.predicate,
                    &hidden,
                    &slice.low,
                    &slice.high,
                    &slice.graph,
                    OrderDirection::Ascending,
                )?;
                constrain_ordered_triple_value_to_ground(compiled, HIDDEN, &slice.object)?
            }
            _ => return Err(BgpPlanError::TripleValueOrderUnsupportedShape),
        },
        Query::TripleSliceRev(slice) => match &slice.object {
            Value::Variable(_) => compile_ordered_triple_slice(
                &slice.subject,
                &slice.predicate,
                &slice.object,
                &slice.low,
                &slice.high,
                &slice.graph,
                OrderDirection::Descending,
            )?,
            Value::Data(_) => {
                const HIDDEN: &str = "__woql_proof_ground_triple_value";
                let hidden = Value::Variable(HIDDEN.to_owned());
                let compiled = compile_ordered_triple_slice(
                    &slice.subject,
                    &slice.predicate,
                    &hidden,
                    &slice.low,
                    &slice.high,
                    &slice.graph,
                    OrderDirection::Descending,
                )?;
                constrain_ordered_triple_value_to_ground(compiled, HIDDEN, &slice.object)?
            }
            _ => return Err(BgpPlanError::TripleValueOrderUnsupportedShape),
        },
        Query::TripleNext(next) => {
            let (variable, reference, expected, op, direction) = match (&next.object, &next.next) {
                (Value::Data(reference), Value::Variable(variable)) => {
                    (variable.as_str(), Value::Data(reference.clone()), None, RangeOp::Gt, OrderDirection::Ascending)
                }
                (Value::Variable(variable), Value::Data(reference)) => {
                    (variable.as_str(), Value::Data(reference.clone()), None, RangeOp::Lt, OrderDirection::Descending)
                }
                (Value::Data(reference), Value::Data(expected)) => {
                    ("__woql_proof_ground_triple_value", Value::Data(reference.clone()), Some(Value::Data(expected.clone())), RangeOp::Gt, OrderDirection::Ascending)
                }
                _ => return Err(BgpPlanError::TripleValueOrderUnsupportedShape),
            };
            let (datatype, c, canonical) = encoded_ordered_triple_value(&reference)?;
            let compiled = compile_ordered_triple_value_scan(
                &next.subject,
                &next.predicate,
                &next.graph,
                variable,
                datatype,
                canonical,
                vec![NormalizedBound { op, c }],
                direction,
                true,
            )?;
            match expected {
                Some(expected) => constrain_ordered_triple_value_to_ground(compiled, variable, &expected)?,
                None => compiled,
            }
        }
        Query::TriplePrevious(previous) => {
            let (variable, reference, expected, op, direction) = match (&previous.object, &previous.previous) {
                (Value::Data(reference), Value::Variable(variable)) => {
                    (variable.as_str(), Value::Data(reference.clone()), None, RangeOp::Lt, OrderDirection::Descending)
                }
                (Value::Variable(variable), Value::Data(reference)) => {
                    (variable.as_str(), Value::Data(reference.clone()), None, RangeOp::Gt, OrderDirection::Ascending)
                }
                (Value::Data(reference), Value::Data(expected)) => {
                    ("__woql_proof_ground_triple_value", Value::Data(reference.clone()), Some(Value::Data(expected.clone())), RangeOp::Lt, OrderDirection::Descending)
                }
                _ => return Err(BgpPlanError::TripleValueOrderUnsupportedShape),
            };
            let (datatype, c, canonical) = encoded_ordered_triple_value(&reference)?;
            let compiled = compile_ordered_triple_value_scan(
                &previous.subject,
                &previous.predicate,
                &previous.graph,
                variable,
                datatype,
                canonical,
                vec![NormalizedBound { op, c }],
                direction,
                true,
            )?;
            match expected {
                Some(expected) => constrain_ordered_triple_value_to_ground(compiled, variable, &expected)?,
                None => compiled,
            }
        }
        _ => return Ok(None),
    };
    Ok(Some(compiled))
}

fn plan_layer_triple_count(
    count: &crate::misc::TripleCount,
) -> Result<CompiledBgp, BgpPlanError> {
    let collection = count
        .resource
        .strip_suffix("/instance")
        .filter(|collection| !collection.is_empty())
        .ok_or(BgpPlanError::TripleCountUnsupportedResource)?;
    let variable = match &count.count {
        DataValue::Variable(variable) => variable.clone(),
        _ => return Err(BgpPlanError::TripleCountOutputNotVariable),
    };
    Ok(CompiledBgp {
        relation: CompiledRelation {
            schema: vec![variable],
            domains: vec![None],
            statement: CompiledRelationStatement::LayerTripleCount,
        },
        projection: None,
        collection: Some(collection.to_owned()),
        count_variable: None,
        full_row_distinct: false,
    })
}

fn exact_integral_decimal_u64(value: &XSDAnySimpleType) -> Result<u64, BgpPlanError> {
    let XSDAnySimpleType::Decimal(decimal) = value else {
        return Err(BgpPlanError::TripleCountComparisonUnsupportedConstant);
    };
    // A negative scale is the retained representation of a positive decimal exponent.
    // Do not silently broaden this bounded contract to scientific notation.
    if decimal.is_sign_negative() || decimal.scale() < 0 {
        return Err(BgpPlanError::TripleCountComparisonUnsupportedConstant);
    }
    let value = u64::try_from(decimal)
        .map_err(|_| BgpPlanError::TripleCountComparisonUnsupportedConstant)?;
    if decimal != &decimal_rs::Decimal::from(value) {
        return Err(BgpPlanError::TripleCountComparisonUnsupportedConstant);
    }
    Ok(value)
}

fn flatten_triple_count_comparison_body<'a>(query: &'a Query, leaves: &mut Vec<&'a Query>) {
    match query {
        Query::And(and) => {
            for child in &and.and {
                flatten_triple_count_comparison_body(child, leaves);
            }
        }
        Query::Pin(pin) => flatten_triple_count_comparison_body(pin.query.as_ref(), leaves),
        Query::Immediately(immediately) => {
            flatten_triple_count_comparison_body(immediately.query.as_ref(), leaves)
        }
        Query::True(_) | Query::Comment(_) => {}
        other => leaves.push(other),
    }
}

fn generated_triple_count_constraint(
    query: &Query,
    variable: &str,
) -> Result<(GeneratedU64ComparisonOp, u64, bool), BgpPlanError> {
    let range = |left: &DataValue,
                 right: &DataValue,
                 op: GeneratedU64ComparisonOp|
     -> Result<_, BgpPlanError> {
        match (left, right) {
            (DataValue::Variable(candidate), DataValue::Data(value)) if candidate == variable => {
                Ok((op, exact_integral_decimal_u64(value)?, false))
            }
            (DataValue::Data(value), DataValue::Variable(candidate)) if candidate == variable => {
                Ok((op, exact_integral_decimal_u64(value)?, true))
            }
            _ => Err(BgpPlanError::TripleCountComparisonUnsupportedShape),
        }
    };
    match query {
        Query::Greater(value) => range(&value.left, &value.right, GeneratedU64ComparisonOp::Gt),
        Query::Gte(value) => range(&value.left, &value.right, GeneratedU64ComparisonOp::Ge),
        Query::Less(value) => range(&value.left, &value.right, GeneratedU64ComparisonOp::Lt),
        Query::Lte(value) => range(&value.left, &value.right, GeneratedU64ComparisonOp::Le),
        Query::Equals(value) => {
            use crate::value::WoqlValue;
            match (&value.left, &value.right) {
                (WoqlValue::Variable(candidate), WoqlValue::Data(value))
                    if candidate == variable =>
                {
                    Ok((
                        GeneratedU64ComparisonOp::Eq,
                        exact_integral_decimal_u64(value)?,
                        false,
                    ))
                }
                (WoqlValue::Data(value), WoqlValue::Variable(candidate))
                    if candidate == variable =>
                {
                    Ok((
                        GeneratedU64ComparisonOp::Eq,
                        exact_integral_decimal_u64(value)?,
                        true,
                    ))
                }
                _ => Err(BgpPlanError::TripleCountComparisonUnsupportedShape),
            }
        }
        _ => Err(BgpPlanError::TripleCountComparisonUnsupportedShape),
    }
}

fn plan_triple_count_comparison_body(
    body: &Query,
) -> Result<Option<CompiledBgp>, BgpPlanError> {
    let mut leaves = Vec::new();
    flatten_triple_count_comparison_body(body, &mut leaves);
    let Some(Query::TripleCount(triple_count)) = leaves.first().copied() else {
        if leaves.iter().any(|leaf| matches!(leaf, Query::TripleCount(_))) {
            return Err(BgpPlanError::TripleCountComparisonUnsupportedShape);
        }
        return Ok(None);
    };
    let mut compiled = plan_layer_triple_count(triple_count)?;
    let variable = compiled.relation.schema[0].clone();
    for comparison in leaves.into_iter().skip(1) {
        let (op, constant, constant_on_left) =
            generated_triple_count_constraint(comparison, &variable)?;
        let input = compiled.relation;
        compiled.relation = CompiledRelation {
            schema: input.schema.clone(),
            domains: input.domains.clone(),
            statement: CompiledRelationStatement::GeneratedU64Filter {
                input: Box::new(input),
                column: 0,
                op,
                constant,
                constant_on_left,
            },
        };
    }
    Ok(Some(compiled))
}

// ============================================================================
// Fixed-length property-path lowering: `Query::Path` -> BGP (`Triple`/`And`) +
// bag union (`Or`) + endpoint projection (`Select`).
//
// The lowering reuses the audited BGP/join/union/projection machinery below; it
// adds no cryptography. It is faithful to the leading-Prolog path evaluator
// (`src/core/query/path.pl`), which:
//   * binds the endpoint pair `(subject, object)` (the traversed edge list is
//     bound only when the `path` field is present — we reject that case),
//   * returns BAG multiplicity over endpoint pairs — one solution per distinct
//     path — with NO deduplication (see the `path.pl` `test(p)` oracle), and
//   * is edge-SIMPLE: `Open_Set`/`in_open_set` forbids re-traversing an identical
//     `(source,predicate,target)` edge within a single path.
//
// A conjunctive bag-join over triples reproduces exactly one row per distinct
// intermediate assignment — i.e. one per distinct path — so plain (non-distinct)
// endpoint projection matches Prolog's bag multiplicity EXACTLY *provided no edge
// can repeat*. Edge repetition needs two hops with the SAME predicate (edges with
// different predicates are always distinct). We therefore accept a multi-hop chain
// only when its hop predicates are pairwise distinct (edge-reuse impossible ->
// join == Prolog on all data, including self-loops) and reject repeated-predicate
// chains closed (`PathRepeatedPredicateUnsupported`) — this also excludes every
// `Times{n>=2}`, whose n-fold expansion necessarily repeats a predicate.
//
// Supported (proven exact, data-independently):
//   * `Predicate(p)`            -> `Triple(S, p, T)`
//   * `InversePredicate(p)`     -> `Triple(T, p, S)`
//   * `Sequence([..])`          -> left-deep `And` chain through fresh intermediates,
//                                  projected to the endpoint variables, when hop
//                                  predicates are pairwise distinct
//   * `Or([..])`                -> bag-union `Or` of the (single-hop) alternatives
//   * `Times(pat, n, n)`        -> the n-fold sequence (only n==1 survives the
//                                  distinct-predicate guard)
//   * `Times(pat, a, b)`        -> bag union over exact lengths a..=b (only single-hop
//                                  lengths survive)
// Rejected closed: `Star`/`Plus`/open `Times` (closure), wildcard `.` predicates,
// repeated-predicate / multi-hop union alternatives, zero-length identity, and
// edge-list capture.
// ============================================================================

/// The maximum `Times` repetition count / range width the lowering will expand,
/// guarding against pathological (e.g. `u64::MAX`) bounds before any allocation.
const PATH_TIMES_MAX_BOUND: u64 = 64;

/// A resolved path endpoint or fresh intermediate: a bound node IRI or a variable.
#[derive(Clone)]
enum PathTerm {
    Variable(String),
    Node(String),
}

impl PathTerm {
    /// This term in a `NodeValue` (subject / predicate-less) position.
    fn as_subject(&self) -> NodeValue {
        match self {
            PathTerm::Variable(name) => NodeValue::Variable(name.clone()),
            PathTerm::Node(iri) => NodeValue::Node(iri.clone()),
        }
    }
    /// This term in a `Value` (object) position.
    fn as_object(&self) -> Value {
        match self {
            PathTerm::Variable(name) => Value::Variable(name.clone()),
            PathTerm::Node(iri) => Value::Node(iri.clone()),
        }
    }
    fn variable_name(&self) -> Option<&str> {
        match self {
            PathTerm::Variable(name) => Some(name.as_str()),
            PathTerm::Node(_) => None,
        }
    }
}

fn path_endpoint_term(value: &Value) -> Result<PathTerm, BgpPlanError> {
    match value {
        Value::Variable(name) => Ok(PathTerm::Variable(name.clone())),
        Value::Node(iri) => Ok(PathTerm::Node(iri.clone())),
        _ => Err(BgpPlanError::PathEndpointUnsupported),
    }
}

/// Allocates fresh intermediate variable names that never collide with a query
/// variable (nor with each other), so a lowered sequence's hidden waypoints can
/// never alias a real binding.
struct FreshPathVars {
    reserved: std::collections::BTreeSet<String>,
    counter: usize,
}

impl FreshPathVars {
    fn new(reserved: std::collections::BTreeSet<String>) -> Self {
        Self {
            reserved,
            counter: 0,
        }
    }
    fn next(&mut self) -> String {
        loop {
            let candidate = format!("__path_hop_{}", self.counter);
            self.counter += 1;
            if self.reserved.insert(candidate.clone()) {
                return candidate;
            }
        }
    }
}

/// True iff a `Query::Path` occurs anywhere in the read-composition fragment the
/// lowering rewrites through. (Paths buried in constructs outside this fragment are
/// reached — and lowered — by `plan_bgp`'s own recursion, or rejected closed there.)
fn query_contains_path(query: &Query) -> bool {
    match query {
        Query::Path(_) => true,
        Query::And(and) => and.and.iter().any(query_contains_path),
        Query::Or(or) => or.or.iter().any(query_contains_path),
        Query::Not(not) => query_contains_path(&not.query),
        Query::Select(select) => query_contains_path(&select.query),
        Query::Distinct(distinct) => query_contains_path(&distinct.query),
        Query::Count(count) => query_contains_path(&count.query),
        Query::Using(using) => query_contains_path(&using.query),
        Query::From(from) => query_contains_path(&from.query),
        Query::Pin(pin) => query_contains_path(&pin.query),
        Query::Immediately(immediately) => query_contains_path(&immediately.query),
        _ => false,
    }
}

/// Reject the three woql2 leaves that are triple-SHAPED but are NOT store triple patterns in
/// TerminusDB's semantics (`TypeOf`/`Dot`/`IsA`), returning the specific, documented
/// [`BgpPlanError`] for the first one encountered. It runs as a pre-pass BEFORE any lowering
/// or shape analysis (see the entry wiring in [`plan_bgp`]), mirroring the path pre-pass's
/// traversal so that:
///  * a NESTED occurrence (e.g. a `TypeOf` inside an `And` inside a `Select`) is caught with
///    its precise soundness rationale rather than a generic fall-through error, and
///  * the guard COMPOSES with the path lowering — a query carrying both a `Path` and one of
///    these leaves rejects on the leaf, because this runs first.
/// Recursion covers exactly the read-composition wrappers the lowering fragment threads
/// through; a leaf buried inside a construct outside that fragment is instead rejected by
/// `plan_bgp`'s own recursion (never silently dropped, which would itself be unsound).
fn reject_non_bgp_typed_leaves(query: &Query) -> Result<(), BgpPlanError> {
    // The whole query starts at BODY position (only outer read-control wrappers above it).
    reject_non_bgp_typed_leaves_at(query, true)
}

/// `body_position` is true while `query` is still reachable as the top-level BODY through outer
/// read-control wrappers ONLY (Select/Distinct/Count/From/…). A standalone (body-position) `IsA` NOW
/// LOWERS via [`plan_isa`] (a cross-root two-root hash join), so it is allowed through here and its
/// precise regime rejection (variable class / bound element) is decided by `plan_isa`. A NESTED `IsA`
/// (inside And/Or/Not — a BGP conjunction or logical combinator) still rejects closed: the cross-root
/// node reads two graphs and is top-level only. `TypeOf`/`Dot` never lower and reject in any position.
fn reject_non_bgp_typed_leaves_at(query: &Query, body_position: bool) -> Result<(), BgpPlanError> {
    match query {
        Query::TypeOf(_) => return Err(BgpPlanError::TypeOfNotStoredType),
        Query::Dot(_) => return Err(BgpPlanError::DotNotStoreEdge),
        Query::IsA(isa) => {
            if !body_position {
                return Err(isa_reject_reason(isa));
            }
        }
        Query::And(and) => and
            .and
            .iter()
            .try_for_each(|q| reject_non_bgp_typed_leaves_at(q, false))?,
        Query::Or(or) => or
            .or
            .iter()
            .try_for_each(|q| reject_non_bgp_typed_leaves_at(q, false))?,
        Query::Not(not) => reject_non_bgp_typed_leaves_at(&not.query, false)?,
        Query::Select(select) => reject_non_bgp_typed_leaves_at(&select.query, body_position)?,
        Query::Distinct(distinct) => reject_non_bgp_typed_leaves_at(&distinct.query, body_position)?,
        Query::Count(count) => reject_non_bgp_typed_leaves_at(&count.query, body_position)?,
        Query::Using(using) => reject_non_bgp_typed_leaves_at(&using.query, body_position)?,
        Query::From(from) => reject_non_bgp_typed_leaves_at(&from.query, body_position)?,
        Query::Pin(pin) => reject_non_bgp_typed_leaves_at(&pin.query, body_position)?,
        Query::Immediately(immediately) => {
            reject_non_bgp_typed_leaves_at(&immediately.query, body_position)?
        }
        _ => {}
    }
    Ok(())
}

/// The PRECISE reject reason for a `Query::IsA` leaf, distinguished by its class term. Both regimes
/// reject closed this pass, but with distinct, documented reasons (matching the reference split in
/// terminusdb `src/core/document/instance.pl`):
///   * a VARIABLE class (`type_of`) → [`BgpPlanError::IsAVariableClass`]: the downward `sys:inherits`
///     subsumption closure needs the class bound as a constant to seed it;
///   * a BOUND class (the node regime `is_instance_class`, instance.pl:80-92) →
///     [`BgpPlanError::IsASubsumptionUnsupported`]: it needs a cross-dictionary IRI-reconciled join
///     between the INSTANCE-graph `rdf:type` relation and the SCHEMA-graph downward subsumption closure
///     (the two graphs commit SEPARATE dictionaries under SEPARATE roots, so no sound raw-id join
///     exists — see that variant's rationale).
///
/// The LITERAL-subject regime of `is_instance` (the XSD datatype lattice, `basetype_subsumption_of`,
/// instance.pl:50-66) is structurally UNREACHABLE from woql2 `IsA`: its `element` field is a
/// [`crate::value::NodeValue`], which is only a node IRI or a variable and cannot carry a typed
/// literal, so a "literal isa" cannot be constructed here and needs no separate arm.
fn isa_reject_reason(isa: &crate::compare::IsA) -> BgpPlanError {
    match &isa.type_of {
        crate::value::NodeValue::Variable(_) => BgpPlanError::IsAVariableClass,
        crate::value::NodeValue::Node(_) => BgpPlanError::IsASubsumptionUnsupported,
    }
}

/// Collect every variable name used in the fragment we lower through, so fresh path
/// intermediates avoid them. Constructs outside this fragment reject closed and never
/// share a join namespace with a lowered path, so their variables need not be gathered.
fn collect_reserved_path_variables(query: &Query, out: &mut std::collections::BTreeSet<String>) {
    fn push_value(value: &Value, out: &mut std::collections::BTreeSet<String>) {
        if let Value::Variable(name) = value {
            out.insert(name.clone());
        }
    }
    fn push_node(value: &NodeValue, out: &mut std::collections::BTreeSet<String>) {
        if let NodeValue::Variable(name) = value {
            out.insert(name.clone());
        }
    }
    match query {
        Query::Path(path) => {
            push_value(&path.subject, out);
            push_value(&path.object, out);
            if let Some(captured) = &path.path {
                push_value(captured, out);
            }
        }
        Query::Triple(triple) => {
            push_node(&triple.subject, out);
            push_node(&triple.predicate, out);
            push_value(&triple.object, out);
        }
        Query::And(and) => and
            .and
            .iter()
            .for_each(|q| collect_reserved_path_variables(q, out)),
        Query::Or(or) => or
            .or
            .iter()
            .for_each(|q| collect_reserved_path_variables(q, out)),
        Query::Not(not) => collect_reserved_path_variables(&not.query, out),
        Query::Select(select) => {
            out.extend(select.variables.iter().cloned());
            collect_reserved_path_variables(&select.query, out);
        }
        Query::Distinct(distinct) => {
            out.extend(distinct.variables.iter().cloned());
            collect_reserved_path_variables(&distinct.query, out);
        }
        Query::Count(count) => {
            if let DataValue::Variable(name) = &count.count {
                out.insert(name.clone());
            }
            collect_reserved_path_variables(&count.query, out);
        }
        Query::Using(using) => collect_reserved_path_variables(&using.query, out),
        Query::From(from) => collect_reserved_path_variables(&from.query, out),
        Query::Pin(pin) => collect_reserved_path_variables(&pin.query, out),
        Query::Immediately(immediately) => {
            collect_reserved_path_variables(&immediately.query, out)
        }
        _ => {}
    }
}

/// Rewrite every `Query::Path` in the read-composition fragment into its lowered
/// `Triple`/`And`/`Or`/`Select` equivalent, leaving all other nodes structurally intact.
fn rewrite_query_paths(query: &Query, alloc: &mut FreshPathVars) -> Result<Query, BgpPlanError> {
    let rewrite_boxed =
        |q: &Query, alloc: &mut FreshPathVars| -> Result<Box<Query>, BgpPlanError> {
            Ok(Box::new(rewrite_query_paths(q, alloc)?))
        };
    Ok(match query {
        Query::Path(path) => lower_path(path, alloc)?,
        Query::And(and) => Query::And(crate::query::And {
            and: and
                .and
                .iter()
                .map(|q| rewrite_query_paths(q, alloc))
                .collect::<Result<_, _>>()?,
        }),
        Query::Or(or) => Query::Or(crate::query::Or {
            or: or
                .or
                .iter()
                .map(|q| rewrite_query_paths(q, alloc))
                .collect::<Result<_, _>>()?,
        }),
        Query::Not(not) => Query::Not(crate::query::Not {
            query: rewrite_boxed(&not.query, alloc)?,
        }),
        Query::Select(select) => Query::Select(crate::control::Select {
            variables: select.variables.clone(),
            query: rewrite_boxed(&select.query, alloc)?,
        }),
        Query::Distinct(distinct) => Query::Distinct(crate::control::Distinct {
            variables: distinct.variables.clone(),
            query: rewrite_boxed(&distinct.query, alloc)?,
        }),
        Query::Count(count) => Query::Count(crate::misc::Count {
            query: rewrite_boxed(&count.query, alloc)?,
            count: count.count.clone(),
        }),
        Query::Using(using) => Query::Using(crate::control::Using {
            collection: using.collection.clone(),
            query: rewrite_boxed(&using.query, alloc)?,
        }),
        Query::From(from) => Query::From(crate::control::From {
            graph: from.graph.clone(),
            query: rewrite_boxed(&from.query, alloc)?,
        }),
        Query::Pin(pin) => Query::Pin(crate::control::Pin {
            query: rewrite_boxed(&pin.query, alloc)?,
        }),
        Query::Immediately(immediately) => Query::Immediately(crate::control::Immediately {
            query: rewrite_boxed(&immediately.query, alloc)?,
        }),
        other => other.clone(),
    })
}

/// Lower a single `Path` node to an equivalent BGP/union `Query`.
///
/// A pairwise-distinct-predicate path (the edge-repetition-free case) lowers here to a
/// plain bag-join `Query`. A single-predicate REPEATED path cannot be captured by a plain
/// bag-join (it edge-simple over-counts), so `ensure_edge_simple_exact` raises
/// `PathRepeatedPredicateUnsupported`; we catch exactly that signal and DEFER — returning
/// the `Path` node UN-lowered so `plan_bgp` routes it to
/// [`plan_deferred_repeated_predicate_path`], which builds the join chain plus the
/// `TupleDisequality` edge-simple filter at the plan level (a `Query` cannot express that
/// filter). Every other rejection (unbounded / wildcard / identity / edge capture)
/// propagates unchanged.
fn lower_path(
    path: &crate::query::Path,
    alloc: &mut FreshPathVars,
) -> Result<Query, BgpPlanError> {
    if path.path.is_some() {
        return Err(BgpPlanError::PathWitnessCaptureUnsupported);
    }
    // A `Star`/`Plus` transitive-closure path is compiled at the PLAN level: it needs the endpoint
    // `Distinct` gate and the composite `TransitiveClosure` node, neither of which is expressible as a
    // rewritten `Query`. Leave the `Path` node in place so `plan_bgp` routes it to
    // `plan_transitive_closure_path` (which reject-closes every unsupported closure shape). A closure
    // buried inside a larger `And` is NOT the plan body, so it reaches the conjunctive compiler as a
    // non-triple conjunct and rejects closed there.
    if matches!(
        path.pattern,
        crate::path::PathPattern::Star(_) | crate::path::PathPattern::Plus(_)
    ) {
        return Ok(Query::Path(path.clone()));
    }
    let source = path_endpoint_term(&path.subject)?;
    let target = path_endpoint_term(&path.object)?;
    match lower_top(&path.pattern, &source, &target, alloc) {
        // A repeated-predicate chain is provable via the deferred TupleDisequality
        // lowering — leave the Path node in place for `plan_bgp` to route there.
        Err(BgpPlanError::PathRepeatedPredicateUnsupported { .. }) => {
            Ok(Query::Path(path.clone()))
        }
        other => other,
    }
}

/// Top-level pattern lowering: may produce a bag union (`Or`) or a conjunctive chain
/// projected to its endpoints (`Select` over `And`).
fn lower_top(
    pattern: &crate::path::PathPattern,
    source: &PathTerm,
    target: &PathTerm,
    alloc: &mut FreshPathVars,
) -> Result<Query, BgpPlanError> {
    use crate::path::PathPattern;
    match pattern {
        PathPattern::Or(_) => {
            let mut branches = Vec::new();
            lower_union_branches(pattern, source, target, alloc, &mut branches)?;
            match branches.len() {
                0 => Err(BgpPlanError::PathEmptyPattern),
                1 => Ok(branches.pop().unwrap()),
                _ => Ok(Query::Or(crate::query::Or { or: branches })),
            }
        }
        PathPattern::Times(times) if times.from != times.to => {
            lower_times_range_union(times, source, target, alloc)
        }
        PathPattern::Plus(_) | PathPattern::Star(_) => {
            Err(BgpPlanError::UnboundedPathUnsupported)
        }
        _ => {
            let mut triples = Vec::new();
            let mut predicates = Vec::new();
            lower_chain(pattern, source, target, alloc, &mut triples, &mut predicates)?;
            ensure_edge_simple_exact(&predicates)?;
            build_chain_query(triples, source, target)
        }
    }
}

/// Expand a bounded-range `Times(pat, a, b)` into a bag union over each exact length.
/// Every length must lower to a single hop (multi-hop union branches reject closed).
fn lower_times_range_union(
    times: &crate::path::PathTimes,
    source: &PathTerm,
    target: &PathTerm,
    alloc: &mut FreshPathVars,
) -> Result<Query, BgpPlanError> {
    if times.to < times.from {
        return Err(BgpPlanError::PathTimesInvalidRange);
    }
    if times.from == 0 {
        return Err(BgpPlanError::PathIdentityUnsupported);
    }
    if times.to > PATH_TIMES_MAX_BOUND {
        return Err(BgpPlanError::UnboundedPathUnsupported);
    }
    let mut branches = Vec::new();
    for length in times.from..=times.to {
        let mut triples = Vec::new();
        let mut predicates = Vec::new();
        lower_n_fold(&times.times, length, source, target, alloc, &mut triples, &mut predicates)?;
        ensure_edge_simple_exact(&predicates)?;
        if triples.len() != 1 {
            return Err(BgpPlanError::PathUnionBranchMultiHop);
        }
        branches.push(triples.into_iter().next().unwrap());
    }
    match branches.len() {
        0 => Err(BgpPlanError::PathEmptyPattern),
        1 => Ok(branches.pop().unwrap()),
        _ => Ok(Query::Or(crate::query::Or { or: branches })),
    }
}

/// Flatten a union pattern into its single-hop alternative triples. Nested `Or`s
/// flatten in source order; a multi-hop alternative rejects closed.
fn lower_union_branches(
    pattern: &crate::path::PathPattern,
    source: &PathTerm,
    target: &PathTerm,
    alloc: &mut FreshPathVars,
    out: &mut Vec<Query>,
) -> Result<(), BgpPlanError> {
    use crate::path::PathPattern;
    match pattern {
        PathPattern::Or(or) => {
            if or.or.is_empty() {
                return Err(BgpPlanError::PathEmptyPattern);
            }
            for branch in &or.or {
                lower_union_branches(branch, source, target, alloc, out)?;
            }
            Ok(())
        }
        PathPattern::Plus(_) | PathPattern::Star(_) => Err(BgpPlanError::UnboundedPathUnsupported),
        _ => {
            let mut triples = Vec::new();
            let mut predicates = Vec::new();
            lower_chain(pattern, source, target, alloc, &mut triples, &mut predicates)?;
            ensure_edge_simple_exact(&predicates)?;
            if triples.len() != 1 {
                return Err(BgpPlanError::PathUnionBranchMultiHop);
            }
            out.push(triples.into_iter().next().unwrap());
            Ok(())
        }
    }
}

/// Lower a pattern that must be a pure conjunctive chain (no unions), threading
/// `source -> ... -> target` through fresh intermediates and appending each hop's
/// triple and predicate string. Rejects any construct that would introduce a union.
fn lower_chain(
    pattern: &crate::path::PathPattern,
    source: &PathTerm,
    target: &PathTerm,
    alloc: &mut FreshPathVars,
    triples: &mut Vec<Query>,
    predicates: &mut Vec<String>,
) -> Result<(), BgpPlanError> {
    use crate::path::PathPattern;
    match pattern {
        PathPattern::Predicate(pred) => {
            let predicate = pred
                .predicate
                .clone()
                .ok_or(BgpPlanError::PathWildcardUnsupported)?;
            triples.push(Query::Triple(Triple {
                subject: source.as_subject(),
                predicate: NodeValue::Node(predicate.clone()),
                object: target.as_object(),
                graph: None,
            }));
            predicates.push(predicate);
            Ok(())
        }
        PathPattern::InversePredicate(pred) => {
            let predicate = pred
                .predicate
                .clone()
                .ok_or(BgpPlanError::PathWildcardUnsupported)?;
            // Inverse: the edge points target -> source (swap subject/object).
            triples.push(Query::Triple(Triple {
                subject: target.as_subject(),
                predicate: NodeValue::Node(predicate.clone()),
                object: source.as_object(),
                graph: None,
            }));
            predicates.push(predicate);
            Ok(())
        }
        PathPattern::Sequence(sequence) => match sequence.sequence.len() {
            0 => Err(BgpPlanError::PathEmptyPattern),
            1 => lower_chain(&sequence.sequence[0], source, target, alloc, triples, predicates),
            hops => {
                let mut previous = source.clone();
                for (index, sub) in sequence.sequence.iter().enumerate() {
                    let next = if index == hops - 1 {
                        target.clone()
                    } else {
                        PathTerm::Variable(alloc.next())
                    };
                    lower_chain(sub, &previous, &next, alloc, triples, predicates)?;
                    previous = next;
                }
                Ok(())
            }
        },
        PathPattern::Times(times) => {
            if times.from != times.to {
                // A bounded-RANGE repetition is a union; it cannot be a chain hop.
                return Err(BgpPlanError::PathNonElementaryHopUnsupported);
            }
            if times.from == 0 {
                return Err(BgpPlanError::PathIdentityUnsupported);
            }
            if times.from > PATH_TIMES_MAX_BOUND {
                return Err(BgpPlanError::UnboundedPathUnsupported);
            }
            lower_n_fold(&times.times, times.from, source, target, alloc, triples, predicates)
        }
        PathPattern::Or(_) => Err(BgpPlanError::PathNonElementaryHopUnsupported),
        PathPattern::Plus(_) | PathPattern::Star(_) => Err(BgpPlanError::UnboundedPathUnsupported),
    }
}

/// Lower the `n`-fold sequence of `pattern` into a chain `source -> ... -> target`.
fn lower_n_fold(
    pattern: &crate::path::PathPattern,
    n: u64,
    source: &PathTerm,
    target: &PathTerm,
    alloc: &mut FreshPathVars,
    triples: &mut Vec<Query>,
    predicates: &mut Vec<String>,
) -> Result<(), BgpPlanError> {
    if n == 0 {
        return Err(BgpPlanError::PathIdentityUnsupported);
    }
    if n == 1 {
        return lower_chain(pattern, source, target, alloc, triples, predicates);
    }
    let mut previous = source.clone();
    for index in 0..n {
        let next = if index == n - 1 {
            target.clone()
        } else {
            PathTerm::Variable(alloc.next())
        };
        lower_chain(pattern, &previous, &next, alloc, triples, predicates)?;
        previous = next;
    }
    Ok(())
}

/// Reject a chain whose hop predicates are not pairwise distinct — the guard that
/// keeps a bag-join faithful to edge-simple Prolog path semantics (see the module
/// header). Distinct predicates make edge repetition impossible, so the join equals
/// the Prolog result on ALL data; a repeat could over-count on self-loops.
fn ensure_edge_simple_exact(predicates: &[String]) -> Result<(), BgpPlanError> {
    let mut seen = std::collections::BTreeSet::new();
    for predicate in predicates {
        if !seen.insert(predicate.as_str()) {
            return Err(BgpPlanError::PathRepeatedPredicateUnsupported {
                predicate: predicate.clone(),
            });
        }
    }
    Ok(())
}

/// Assemble a conjunctive chain into a `Query`, projecting out intermediates by
/// wrapping a multi-hop `And` in a `Select` over the endpoint variables. A single
/// hop already has exactly the endpoint schema and needs no projection.
fn build_chain_query(
    triples: Vec<Query>,
    source: &PathTerm,
    target: &PathTerm,
) -> Result<Query, BgpPlanError> {
    let hops = triples.len();
    if hops == 0 {
        return Err(BgpPlanError::PathEmptyPattern);
    }
    if hops == 1 {
        return Ok(triples.into_iter().next().unwrap());
    }
    let body = Query::And(crate::query::And { and: triples });
    // Endpoint variables, in [subject, object] order, deduplicated (subject == object
    // is a legal cycle query binding a single variable).
    let mut endpoints: Vec<String> = Vec::new();
    if let Some(name) = source.variable_name() {
        endpoints.push(name.to_string());
    }
    if let Some(name) = target.variable_name() {
        if !endpoints.iter().any(|existing| existing == name) {
            endpoints.push(name.to_string());
        }
    }
    Ok(Query::Select(crate::control::Select {
        variables: endpoints,
        query: Box::new(body),
    }))
}

/// Build the `C(n,2)` hop-pair disequality specs for a lowered single-predicate chain.
/// Each triple is one stored edge `(subject, object)` (the fixed predicate cancels); a
/// pair drops rows where two edges coincide. A variable position becomes a committed-column
/// reference (variable name -> the join schema index); a bound node position becomes a
/// constant node reference. The verifier reconstructs these EXACT pairs from the trusted
/// compiled plan (folded into the query digest), so the selection is not prover-chosen.
fn build_disequality_pairs(
    triples: &[Query],
    schema: &[String],
) -> Result<Vec<TupleDisequalityPairPlan>, BgpPlanError> {
    fn column_of(name: &str, schema: &[String]) -> Result<TupleDisequalityNode, BgpPlanError> {
        schema
            .iter()
            .position(|candidate| candidate == name)
            .map(TupleDisequalityNode::Column)
            .ok_or(BgpPlanError::PathEndpointUnsupported)
    }
    let subject_ref = |subject: &NodeValue| -> Result<TupleDisequalityNode, BgpPlanError> {
        match subject {
            NodeValue::Variable(name) => column_of(name, schema),
            NodeValue::Node(iri) => Ok(TupleDisequalityNode::Constant(CanonicalObject::Node(
                iri.clone().into_bytes(),
            ))),
        }
    };
    let object_ref = |object: &Value| -> Result<TupleDisequalityNode, BgpPlanError> {
        match object {
            Value::Variable(name) => column_of(name, schema),
            Value::Node(iri) => Ok(TupleDisequalityNode::Constant(CanonicalObject::Node(
                iri.clone().into_bytes(),
            ))),
            // Path node positions are only variables or bound node IRIs (endpoints are
            // guarded by `path_endpoint_term`; intermediates are fresh variables).
            _ => Err(BgpPlanError::PathEndpointUnsupported),
        }
    };
    let mut edges = Vec::with_capacity(triples.len());
    for triple in triples {
        let Query::Triple(triple) = triple else {
            return Err(BgpPlanError::PathNonElementaryHopUnsupported);
        };
        edges.push((subject_ref(&triple.subject)?, object_ref(&triple.object)?));
    }
    let mut pairs = Vec::new();
    for e in 0..edges.len() {
        for f in (e + 1)..edges.len() {
            pairs.push(TupleDisequalityPairPlan {
                a0: edges[e].0.clone(),
                a1: edges[e].1.clone(),
                b0: edges[f].0.clone(),
                b1: edges[f].1.clone(),
            });
        }
    }
    Ok(pairs)
}

/// Project a filtered path chain to its endpoint variables (bag multiplicity preserved), so
/// a single-length chain and every range branch share the `[source?, target?]` endpoint
/// schema. Bound (constant) endpoints contribute no column; a fully-ground path projects to
/// the canonical zero-column (unit) relation.
fn project_path_endpoints(
    relation: CompiledRelation,
    source: &PathTerm,
    target: &PathTerm,
) -> Result<CompiledRelation, BgpPlanError> {
    let mut endpoints: Vec<&str> = Vec::new();
    if let Some(name) = source.variable_name() {
        endpoints.push(name);
    }
    if let Some(name) = target.variable_name() {
        if !endpoints.contains(&name) {
            endpoints.push(name);
        }
    }
    let mut indices = Vec::with_capacity(endpoints.len());
    for name in &endpoints {
        let index = relation
            .schema
            .iter()
            .position(|candidate| candidate == name)
            .ok_or(BgpPlanError::PathEndpointUnsupported)?;
        indices.push(index);
    }
    let schema: Vec<String> = indices.iter().map(|i| relation.schema[*i].clone()).collect();
    let domains: Vec<_> = indices.iter().map(|i| relation.domains[*i]).collect();
    Ok(CompiledRelation {
        schema,
        domains,
        statement: CompiledRelationStatement::BagProjection {
            input: Box::new(relation),
            indices,
        },
    })
}

/// Compile a DEFERRED finite single-predicate repeated-predicate path (see [`lower_path`])
/// to `BagProjection(TupleDisequality(join-chain))` — the edge-simple-exact plan a plain
/// bag-join cannot express. A range `Times{a,b}` fans out to a bag union over each exact
/// length. Reuses the audited BGP compiler for the join chains and the `plan_or_relation`
/// union path; adds only the `TupleDisequality` selection and the endpoint projection.
fn plan_deferred_repeated_predicate_path(
    path: &crate::query::Path,
    selections: &[(&[String], bool)],
    collection: Option<String>,
    count_variable: Option<String>,
    distincts: &[&[String]],
) -> Result<CompiledBgp, BgpPlanError> {
    use crate::path::PathPattern;
    if path.path.is_some() {
        return Err(BgpPlanError::PathWitnessCaptureUnsupported);
    }

    // RANGE `Times{a,b}` (a != b): a bag union over each exact length, each an independently
    // edge-simple-filtered chain projected to the endpoints. Delegating each length to a
    // fresh `Query::Path` re-enters `plan_bgp` -> this handler (single-length arm) per branch.
    if let PathPattern::Times(times) = &path.pattern {
        if times.from != times.to {
            if times.to < times.from {
                return Err(BgpPlanError::PathTimesInvalidRange);
            }
            if times.from == 0 {
                return Err(BgpPlanError::PathIdentityUnsupported);
            }
            if times.to > PATH_TIMES_MAX_BOUND {
                return Err(BgpPlanError::UnboundedPathUnsupported);
            }
            let mut branches = Vec::with_capacity((times.to - times.from + 1) as usize);
            for length in times.from..=times.to {
                branches.push(Query::Path(crate::query::Path {
                    subject: path.subject.clone(),
                    pattern: PathPattern::Times(crate::path::PathTimes {
                        times: times.times.clone(),
                        from: length,
                        to: length,
                    }),
                    object: path.object.clone(),
                    path: None,
                }));
            }
            let disjunction = crate::query::Or { or: branches };
            return plan_or_relation(
                &disjunction,
                selections,
                collection,
                count_variable,
                distincts,
            );
        }
    }

    // SINGLE-LENGTH: lower to the triple chain, then wrap it in the edge-simple filter.
    let source = path_endpoint_term(&path.subject)?;
    let target = path_endpoint_term(&path.object)?;
    let mut reserved = std::collections::BTreeSet::new();
    collect_reserved_path_variables(&Query::Path(path.clone()), &mut reserved);
    let mut alloc = FreshPathVars::new(reserved);
    let mut triples = Vec::new();
    let mut predicates = Vec::new();
    lower_chain(
        &path.pattern,
        &source,
        &target,
        &mut alloc,
        &mut triples,
        &mut predicates,
    )?;

    // Single predicate is the whole scope of this pass: a mixed-predicate repeat needs the
    // predicate id folded into the fingerprint (a later sub-effort) and rejects closed.
    let first = predicates.first().ok_or(BgpPlanError::PathEmptyPattern)?;
    if predicates.iter().any(|predicate| predicate != first) {
        return Err(BgpPlanError::PathMixedPredicateRepeatUnsupported);
    }

    // Compile the join chain by reusing the audited BGP machinery on the raw `And`.
    let chain_query = if triples.len() == 1 {
        triples[0].clone()
    } else {
        Query::And(crate::query::And { and: triples.clone() })
    };
    let chain = plan_bgp(&chain_query)?;
    if chain.projection.is_some() || chain.count_variable.is_some() || chain.full_row_distinct {
        // A bare triple chain never sets these; a defensive guard against a surprising shape.
        return Err(BgpPlanError::PathNonElementaryHopUnsupported);
    }
    let mut relation = chain.relation;
    let mut path_collection = collection;
    merge_collection(
        &mut path_collection,
        chain.collection.as_ref(),
        BgpPlanError::OrRoutingConflict,
    )?;

    // Wrap the join in the edge-simple TupleDisequality filter (all C(n,2) hop-pairs). A
    // single hop yields no pair (nothing to filter) — but a deferred path always has >= 2.
    let pairs = build_disequality_pairs(&triples, &relation.schema)?;
    if !pairs.is_empty() {
        let schema = relation.schema.clone();
        let domains = relation.domains.clone();
        relation = CompiledRelation {
            schema,
            domains,
            statement: CompiledRelationStatement::TupleDisequality {
                input: Box::new(relation),
                pairs,
            },
        };
    }

    // Project to the endpoint variables so the schema matches Prolog's endpoint bindings
    // (and every range branch shares one endpoint schema).
    relation = project_path_endpoints(relation, &source, &target)?;

    // Apply the outer Distinct/Select/Count wrappers exactly as the other body dispatches.
    let (relation, full_row_distinct) =
        compile_distinct_relation(relation, distincts, selections, count_variable.as_deref())?;
    let projection = compile_projection(selections, count_variable.as_deref(), &relation.schema)?;
    let compiled = CompiledBgp {
        relation,
        projection,
        collection: path_collection,
        count_variable,
        full_row_distinct,
    };
    validate_compiled_structure(&compiled.relation)?;
    Ok(compiled)
}

/// Is this conjunct a single-input calendar-family op (whether ship-supported or reject-closed)?
/// The planner claims ALL of them so a supported op compiles and an unsupported one reject-closes —
/// never silently falling through to a mis-plan.
/// Compile a `Star`/`Plus` transitive-closure property path to the store's `TransitiveClosure`
/// composite node, ONLY under the endpoint-`Distinct` gate that makes the reference's edge-simple BAG
/// of endpoints equal the reachable SET. Every unsupported shape rejects CLOSED with a specific error.
///
/// SCOPE (this pass): a SINGLE concrete predicate (forward or inverse), exactly one BOUND endpoint
/// (bound source, or bound target reached by traversing the inverse edge), under a `Distinct` naming
/// the free endpoint. Deferred (reject closed): compound sub-patterns, two free / two bound endpoints,
/// wildcards, edge-list capture, and `Count`-gated closures.
fn plan_transitive_closure_path(
    path: &crate::query::Path,
    selections: &[(&[String], bool)],
    collection: Option<String>,
    count_variable: Option<String>,
    distincts: &[&[String]],
) -> Result<CompiledBgp, BgpPlanError> {
    use crate::path::PathPattern;
    if path.path.is_some() {
        return Err(BgpPlanError::PathWitnessCaptureUnsupported);
    }

    // `star` = zero-or-more (identity branch always succeeds, path.pl:183); `plus` = one-or-more
    // (path.pl:186). This function is only reached for these two top-level operators.
    let (reflexive, inner) = match &path.pattern {
        PathPattern::Star(star) => (true, star.star.as_ref()),
        PathPattern::Plus(plus) => (false, plus.plus.as_ref()),
        _ => return Err(BgpPlanError::UnboundedPathUnsupported),
    };

    // The closure body must be a SINGLE concrete predicate; a compound body (sequence/union/nested
    // repetition) needs the general BGP-in-the-loop lowering (deferred). A wildcard `.` rejects closed.
    let (predicate, inner_inverse) = match inner {
        PathPattern::Predicate(pred) => (
            pred.predicate
                .clone()
                .ok_or(BgpPlanError::PathWildcardUnsupported)?,
            false,
        ),
        PathPattern::InversePredicate(pred) => (
            pred.predicate
                .clone()
                .ok_or(BgpPlanError::PathWildcardUnsupported)?,
            true,
        ),
        _ => return Err(BgpPlanError::ClosureCompoundSubpattern),
    };

    // Exactly one endpoint bound (the seed) and one free (the delivered reachable-set column). A bound
    // TARGET mirrors the traversal to the inverse edge (path.pl:129-134 `run_pattern`, `invert_path`).
    let source = path_endpoint_term(&path.subject)?;
    let target = path_endpoint_term(&path.object)?;
    let (seed_iri, endpoint_var, bound_target) = match (&source, &target) {
        (PathTerm::Node(a), PathTerm::Variable(y)) => (a.clone(), y.clone(), false),
        (PathTerm::Variable(x), PathTerm::Node(b)) => (b.clone(), x.clone(), true),
        (PathTerm::Node(_), PathTerm::Node(_)) => {
            return Err(BgpPlanError::ClosureBothEndpointsBound)
        }
        (PathTerm::Variable(_), PathTerm::Variable(_)) => {
            return Err(BgpPlanError::ClosureBothEndpointsFree)
        }
    };

    // Effective traversal direction. A bound target and an inverse predicate are each ONE mirroring;
    // two mirrorings cancel (`invert_path(plus(n(P))) = plus(p(P))`, path.pl:114-134).
    let inverted = bound_target ^ inner_inverse;

    // `Count`-gated closures are a deferred follow-up; do the `Distinct` form first.
    if count_variable.is_some() {
        return Err(BgpPlanError::ClosureCountGateDeferred);
    }

    // The edge relation E = Triple(?s, p, ?o): a single-atom 2-column [subject, object] node BGP.
    let edge = LayerBgpPlan {
        source: ColumnSource::Materialized,
        atoms: vec![LayerBgpAtom::all_variables(predicate.into_bytes())],
        joins: Vec::new(),
    };
    let relation = CompiledRelation {
        schema: vec![endpoint_var],
        domains: vec![None],
        statement: CompiledRelationStatement::TransitiveClosure {
            edge,
            seed: CanonicalObject::Node(seed_iri.into_bytes()),
            reflexive,
            inverted,
            // A WOQL `Path` closure walks the INSTANCE graph — byte-identical to before the
            // schema-graph foundation (the `Instance` selector folds nothing into the digest).
            graph: ProofGraph::Instance,
        },
    };

    // ENFORCE the gate: the closure delivers a SET, so the surrounding `Distinct` must name its full
    // (single-column) schema. `compile_distinct_relation` returns `full_row_distinct == true` exactly
    // when the `Distinct` covers the schema over a duplicate-free child (the closure is duplicate-free
    // by construction — see `is_provably_duplicate_free`). Anything else (no `Distinct`, or a
    // `Distinct` on a different variable) fails the gate and rejects closed.
    let (relation, full_row_distinct) =
        compile_distinct_relation(relation, distincts, selections, count_variable.as_deref())?;
    if !full_row_distinct {
        return Err(BgpPlanError::ClosureRequiresDistinct);
    }
    let projection = compile_projection(selections, count_variable.as_deref(), &relation.schema)?;
    let compiled = CompiledBgp {
        relation,
        projection,
        collection,
        count_variable,
        full_row_distinct,
    };
    validate_compiled_structure(&compiled.relation)?;
    Ok(compiled)
}

/// The `sys:inherits` subclass edge IRI (schema.pl:223-235,591). An edge `(s, sys:inherits, o)` in
/// the SCHEMA graph records that class `s` DIRECTLY inherits class `o` (subject = subclass, object =
/// superclass). `class_subsumed(A,B)` = `B ∈ {A} ∪ transitive-supers(A)` is the REFLEXIVE-TRANSITIVE
/// closure of these edges from `A` forward.
const SYS_INHERITS_IRI: &str = "http://terminusdb.com/schema/sys#inherits";

/// One resolved endpoint of a `Subsumption`: a bound class IRI or a free variable.
enum SubsumptionEndpoint {
    Class(String),
    Variable(String),
}

fn subsumption_endpoint(value: &crate::value::NodeValue) -> Result<SubsumptionEndpoint, BgpPlanError> {
    match value {
        crate::value::NodeValue::Node(iri) => Ok(SubsumptionEndpoint::Class(iri.clone())),
        crate::value::NodeValue::Variable(name) => Ok(SubsumptionEndpoint::Variable(name.clone())),
    }
}

/// SCHEMA-GRAPH FOUNDATION — compile WOQL `Subsumption child << parent` (class subsumption).
///
/// Reference (`woql_compile.pl:2423`): `Subsumption(A,B)` compiles to `distinct([A,B],
/// class_subsumed(A,B))`, and `class_subsumed` (schema.pl:223-235,591) is the REFLEXIVE-TRANSITIVE
/// closure of the `sys:inherits` subclass edges. So `child << parent` holds iff `parent ∈ {child} ∪
/// transitive-supers(child)`. Edge subject = subclass, object = superclass.
///
/// We emit EXACTLY the audited transitive-closure plan (reused UNCHANGED) over the SCHEMA graph:
///   * `child` bound (a class), `parent` free  → seed=child, reflexive, FORWARD  → {child} ∪ supers(child),
///     delivered as `parent`. (`class_subsumed(child, ·)`.)
///   * `child` free, `parent` bound (a class)  → seed=parent, reflexive, INVERTED → {parent} ∪ subs(parent),
///     delivered as `child`. (Walking the inverse `sys:inherits` edge collects subclasses.)
///   * both bound → the FORWARD closure from `child` (a set) is MEMBERSHIP-GATED to the constant
///     `parent` via an authenticated `ColumnConstant` equality (a ground existence check).
///
/// The `distinct([child,parent])` wrapper is INTRINSIC (the reference lowering supplies it), so it is
/// synthesized here as the endpoint `Distinct` the closure planner already requires — never relying on
/// the user to write it. `reflexive = true` always (the `Class ⊑ Class` base case). Both endpoints
/// FREE is rejected closed (a whole-schema pairs enumeration; deferred).
fn plan_subsumption(
    sub: &crate::compare::Subsumption,
    selections: &[(&[String], bool)],
    collection: Option<String>,
    count_variable: Option<String>,
    distincts: &[&[String]],
) -> Result<CompiledBgp, BgpPlanError> {
    let child = subsumption_endpoint(&sub.child)?;
    let parent = subsumption_endpoint(&sub.parent)?;
    match (child, parent) {
        // `child` bound, `parent` free: forward reflexive closure from the child delivers the
        // superclasses (incl. the child itself), bound to the `parent` variable.
        (SubsumptionEndpoint::Class(child_iri), SubsumptionEndpoint::Variable(parent_var)) => {
            build_subsumption_closure(
                child_iri,
                parent_var,
                /* inverted */ false,
                selections,
                collection,
                count_variable,
                distincts,
            )
        }
        // `parent` bound, `child` free: inverted reflexive closure from the parent walks the inverse
        // `sys:inherits` edge, delivering the subclasses (incl. the parent itself), bound to `child`.
        (SubsumptionEndpoint::Variable(child_var), SubsumptionEndpoint::Class(parent_iri)) => {
            build_subsumption_closure(
                parent_iri,
                child_var,
                /* inverted */ true,
                selections,
                collection,
                count_variable,
                distincts,
            )
        }
        // Both bound: an existence check. Reflexive FORWARD closure from `child` delivers `{child} ∪
        // supers(child)`; a `ColumnConstant` equality gate keeps only the rows equal to the constant
        // `parent`, so the result is non-empty iff `child << parent`. Reuses the audited closure and
        // the audited equality gadget — no new gadget.
        (SubsumptionEndpoint::Class(child_iri), SubsumptionEndpoint::Class(parent_iri)) => {
            build_subsumption_membership_gate(
                child_iri,
                parent_iri,
                selections,
                collection,
                count_variable,
                distincts,
            )
        }
        // Both free: whole-schema pairs enumeration (deferred, mirrors `ClosureBothEndpointsFree`).
        (SubsumptionEndpoint::Variable(_), SubsumptionEndpoint::Variable(_)) => {
            Err(BgpPlanError::SubsumptionBothEndpointsFree)
        }
    }
}

/// The shared schema-graph `sys:inherits` edge `E = Triple(?s, sys:inherits, ?o)`: a single-atom
/// 2-column node BGP over the SCHEMA graph (identical to the instance-layer edge BGP in
/// [`plan_transitive_closure_path`], but the enclosing closure carries `graph: ProofGraph::Schema`).
fn sys_inherits_edge() -> LayerBgpPlan {
    LayerBgpPlan {
        source: ColumnSource::Materialized,
        atoms: vec![LayerBgpAtom::all_variables(SYS_INHERITS_IRI.as_bytes().to_vec())],
        joins: Vec::new(),
    }
}

/// The `rdf:type` predicate IRI the instance `rdf:type` edge reads.
const RDF_TYPE_IRI: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";

/// The INSTANCE-graph `Triple(?x, rdf:type, ?class)` edge — a single-atom 2-column node BGP delivering
/// `[?x, class]` (subject var, `rdf:type` predicate, object var).
fn rdf_type_edge() -> LayerBgpPlan {
    LayerBgpPlan {
        source: ColumnSource::Materialized,
        atoms: vec![LayerBgpAtom::all_variables(RDF_TYPE_IRI.as_bytes().to_vec())],
        joins: Vec::new(),
    }
}

/// SCHEMA-GRAPH FOUNDATION — compile WOQL `IsA(?x, C)` (node regime, BOUND class `C`).
///
/// Reference (`src/core/document/instance.pl:80-92`, `is_instance_class`): `X isa C` holds iff `X` has
/// a DIRECT stored `rdf:type Class` with `class_subsumed(Class, C)` — i.e. `Class` lies in the DOWNWARD
/// `sys:inherits` closure of `C` (`{C} ∪ subclasses(C)`). We lower this to a CROSS-DICTIONARY two-root
/// hash join:
///   * LEFT (schema graph): the downward `sys:inherits` closure of `C` — the reflexive INVERTED
///     transitive closure from `C` (walk the inverse subclass edge to collect `C`'s subclasses),
///     delivering the class SET as one node column.
///   * RIGHT (instance graph): `Triple(?x, rdf:type, ?class)` — the `[?x, class]` direct-type relation.
///   * [`CompiledRelationStatement::CrossRootHashJoin`] joins the two by IRI-HASH (each side's class id
///     authenticated to `hash_object(IRI)` against ITS OWN root; a full-field LogUp SEMI-JOIN keeps
///     instance rows whose class-hash is a member of the closure set), delivering the `?x` set.
///
/// Reject-closed: a VARIABLE class ([`BgpPlanError::IsAVariableClass`]) has no seed for the downward
/// closure; a BOUND ELEMENT (`<n> isa C`, a ground existence check) and any NESTED `IsA` (inside a BGP
/// conjunction) are deferred ([`BgpPlanError::IsASubsumptionUnsupported`]); the LITERAL-subject regime
/// is structurally unrepresentable (`element` is a `NodeValue`, never a typed literal).
fn plan_isa(
    isa: &crate::compare::IsA,
    selections: &[(&[String], bool)],
    collection: Option<String>,
    count_variable: Option<String>,
    distincts: &[&[String]],
) -> Result<CompiledBgp, BgpPlanError> {
    let x_var = match &isa.element {
        crate::value::NodeValue::Variable(v) => v.clone(),
        // A bound element is a ground existence check `<n> isa C`; deferred (needs a different shape).
        crate::value::NodeValue::Node(_) => return Err(BgpPlanError::IsASubsumptionUnsupported),
    };
    let class_iri = match &isa.type_of {
        crate::value::NodeValue::Node(iri) => iri.clone(),
        crate::value::NodeValue::Variable(_) => return Err(BgpPlanError::IsAVariableClass),
    };
    // The cross-root node is TOP-LEVEL only (it cannot be nested under a Store `Distinct`); a user
    // `Distinct` gate is therefore deferred. `Count`/`Select` are `CompiledBgp`-level (no nesting).
    if !distincts.is_empty() {
        return Err(BgpPlanError::IsASubsumptionUnsupported);
    }
    // A fresh internal class variable that cannot collide with the user's `?x`.
    let class_var = {
        let mut name = String::from("__isa_class");
        while name == x_var {
            name.push('_');
        }
        name
    };

    // LEFT (schema): downward `sys:inherits` closure of `C` — reflexive, INVERTED (subclasses of C).
    let closure = CompiledRelation {
        schema: vec![class_var.clone()],
        domains: vec![Some(EqualityValueDomain::Node)],
        statement: CompiledRelationStatement::TransitiveClosure {
            edge: sys_inherits_edge(),
            seed: CanonicalObject::Node(class_iri.into_bytes()),
            reflexive: true,
            inverted: true,
            graph: ProofGraph::Schema,
        },
    };
    // RIGHT (instance): `Triple(?x, rdf:type, ?class)` = [?x, class].
    let rdf_type = CompiledRelation {
        schema: vec![x_var.clone(), class_var.clone()],
        domains: vec![None, Some(EqualityValueDomain::Node)],
        statement: CompiledRelationStatement::Bgp(rdf_type_edge()),
    };
    // The cross-root hash join delivers the `?x` set (right columns minus the class key at index 1).
    let relation = CompiledRelation {
        schema: vec![x_var.clone()],
        domains: vec![None],
        statement: CompiledRelationStatement::CrossRootHashJoin {
            left: Box::new(closure),
            right: Box::new(rdf_type),
            left_key_col: 0,
            right_key_col: 1,
        },
    };

    let projection = compile_projection(selections, count_variable.as_deref(), &relation.schema)?;
    let compiled = CompiledBgp {
        relation,
        projection,
        collection,
        count_variable,
        full_row_distinct: false,
    };
    validate_compiled_structure(&compiled.relation)?;
    Ok(compiled)
}

/// Emit the one-endpoint-bound subsumption closure over the SCHEMA graph and gate it with the
/// INTRINSIC endpoint `Distinct` (set semantics). `reflexive = true` always. `count`-gated closures
/// are a deferred follow-up (mirrors the path closure planner).
fn build_subsumption_closure(
    seed_iri: String,
    endpoint_var: String,
    inverted: bool,
    selections: &[(&[String], bool)],
    collection: Option<String>,
    count_variable: Option<String>,
    distincts: &[&[String]],
) -> Result<CompiledBgp, BgpPlanError> {
    if count_variable.is_some() {
        return Err(BgpPlanError::ClosureCountGateDeferred);
    }
    let relation = CompiledRelation {
        schema: vec![endpoint_var.clone()],
        domains: vec![None],
        statement: CompiledRelationStatement::TransitiveClosure {
            edge: sys_inherits_edge(),
            seed: CanonicalObject::Node(seed_iri.into_bytes()),
            // `class_subsumed` is REFLEXIVE-transitive: the seed class subsumes itself.
            reflexive: true,
            inverted,
            graph: ProofGraph::Schema,
        },
    };
    // The reference lowering is `distinct([child,parent], …)`: synthesize the endpoint `Distinct`
    // (in addition to any user-supplied one) so the closure's bag-of-endpoints collapses to the set
    // `class_subsumed` denotes. This is the exact Distinct-gate the closure planner requires.
    let intrinsic = [endpoint_var.clone()];
    let mut gated: Vec<&[String]> = distincts.to_vec();
    gated.push(&intrinsic);
    let (relation, full_row_distinct) =
        compile_distinct_relation(relation, &gated, selections, count_variable.as_deref())?;
    if !full_row_distinct {
        return Err(BgpPlanError::ClosureRequiresDistinct);
    }
    let projection = compile_projection(selections, count_variable.as_deref(), &relation.schema)?;
    let compiled = CompiledBgp {
        relation,
        projection,
        collection,
        count_variable,
        full_row_distinct,
    };
    validate_compiled_structure(&compiled.relation)?;
    Ok(compiled)
}

/// Emit the BOTH-endpoints-bound subsumption existence check: the reflexive FORWARD closure from
/// `child` delivers `{child} ∪ supers(child)` (a single node column), then an authenticated
/// `ColumnConstant` equality gate keeps only the rows equal to the constant `parent`. The result is
/// non-empty iff `child << parent`. The delivered column is pinned to the `Node` equality domain so
/// the audited equality gadget can compare it against the `parent` node constant.
fn build_subsumption_membership_gate(
    child_iri: String,
    parent_iri: String,
    selections: &[(&[String], bool)],
    collection: Option<String>,
    count_variable: Option<String>,
    distincts: &[&[String]],
) -> Result<CompiledBgp, BgpPlanError> {
    if count_variable.is_some() {
        return Err(BgpPlanError::ClosureCountGateDeferred);
    }
    // A fresh internal endpoint variable: both user endpoints are constants, so this column is never
    // user-visible; it exists only to carry the reachable set into the membership gate.
    let endpoint_var = "__subsumption_super".to_string();
    let closure = CompiledRelation {
        schema: vec![endpoint_var.clone()],
        // Pin the delivered column to the Node equality domain so the `ColumnConstant` gate below can
        // compare it against the `parent` node constant (an unpinned column has no equality domain).
        domains: vec![Some(EqualityValueDomain::Node)],
        statement: CompiledRelationStatement::TransitiveClosure {
            edge: sys_inherits_edge(),
            seed: CanonicalObject::Node(child_iri.into_bytes()),
            reflexive: true,
            inverted: false,
            graph: ProofGraph::Schema,
        },
    };
    // Membership gate: keep only rows whose reachable class == the constant `parent`. Reuses the
    // audited `RelationEqualityPlan::ColumnConstant` gadget over the Node domain.
    let gated = CompiledRelation {
        schema: closure.schema.clone(),
        domains: closure.domains.clone(),
        statement: CompiledRelationStatement::Equality {
            input: Box::new(closure),
            constraint: RelationEqualityPlan::ColumnConstant {
                column: 0,
                domain: EqualityValueDomain::Node,
                value: CanonicalObject::Node(parent_iri.into_bytes()),
                constant_on_left: false,
            },
        },
    };
    // Collapse to the set (intrinsic `distinct([child,parent])`), then project. With both endpoints
    // constant the query has NO free variables, so the projection is empty and the result is a ground
    // existence (0 rows = `child << parent` false, ≥1 row = true).
    let intrinsic = [endpoint_var.clone()];
    let mut gated_distincts: Vec<&[String]> = distincts.to_vec();
    gated_distincts.push(&intrinsic);
    let (relation, full_row_distinct) =
        compile_distinct_relation(gated, &gated_distincts, selections, count_variable.as_deref())?;
    if !full_row_distinct {
        return Err(BgpPlanError::ClosureRequiresDistinct);
    }
    let projection = compile_projection(selections, count_variable.as_deref(), &relation.schema)?;
    let compiled = CompiledBgp {
        relation,
        projection,
        collection,
        count_variable,
        full_row_distinct,
    };
    validate_compiled_structure(&compiled.relation)?;
    Ok(compiled)
}

fn is_calendar_conjunct(query: &Query) -> bool {
    matches!(
        query,
        Query::Weekday(_)
            | Query::WeekdaySundayStart(_)
            | Query::IsoWeek(_)
            | Query::DayAfter(_)
            | Query::DayBefore(_)
            | Query::MonthStartDate(_)
            | Query::MonthEndDate(_)
            | Query::DateDuration(_)
            | Query::Interval(_)
            | Query::IntervalStartDuration(_)
            | Query::IntervalDurationEnd(_)
            | Query::IntervalRelation(_)
            | Query::IntervalRelationTyped(_)
            | Query::MonthStartDates(_)
            | Query::MonthEndDates(_)
    )
}

/// Resolve a calendar op's operand `DataValue` to an authenticated child column index, requiring it
/// to be a VARIABLE bound as a nonnullable node/value store column whose pinned datatype (if any) is
/// `required` (an UNPINNED column is allowed — the actual datatype is enforced per-row at recompute).
fn calendar_operand_column(
    operand: &DataValue,
    child: &CompiledBgp,
    required: CanonicalDatatype,
    not_bound: impl Fn(String) -> BgpPlanError,
    not_typed: impl Fn(String) -> BgpPlanError,
) -> Result<usize, BgpPlanError> {
    let variable = match operand {
        DataValue::Variable(variable) => variable,
        // A literal/list operand has no per-row store authentication.
        _ => return Err(BgpPlanError::CalendarOperandNotVariable),
    };
    let index = child
        .relation
        .schema
        .iter()
        .position(|candidate| candidate == variable)
        .ok_or_else(|| not_bound(variable.clone()))?;
    let descriptors =
        relation_result_descriptors(&child.relation).map_err(|_| not_typed(variable.clone()))?;
    if descriptors[index].nullable || descriptors[index].namespace != ResultIdNamespace::NodeValue {
        return Err(not_typed(variable.clone()));
    }
    // An UNPINNED column (`None`) is allowed (recompute enforces the datatype per row); a datatype
    // pinned to EXACTLY `required` is allowed; any other pinned datatype rejects closed.
    match child.relation.domains.get(index).copied().flatten() {
        None => {}
        Some(EqualityValueDomain::Datatype(datatype)) if datatype == required => {}
        Some(_) => return Err(not_typed(variable.clone())),
    }
    Ok(index)
}

/// A calendar op's result field must be a FRESH output VARIABLE (never a literal / validate mode).
fn calendar_result_var(result: &DataValue) -> Result<String, BgpPlanError> {
    match result {
        DataValue::Variable(variable) => Ok(variable.clone()),
        _ => Err(BgpPlanError::CalendarResultNotVariable),
    }
}

/// Build an INTEGER-result calendar [`CompiledBgp`]: append each `(result_var, function)` as a fresh
/// committed output column (schema + `None` domain), mirroring `plan_eval_form`'s integer path but
/// for one-or-more outputs. Each result variable must be fresh (not in the child schema) and DISTINCT
/// from the others (the running `schema` already holds earlier results, so a single membership check
/// enforces both).
fn build_calendar_int(
    child: CompiledBgp,
    date_column: usize,
    outputs: Vec<(String, CalendarIntFn)>,
) -> Result<CompiledBgp, BgpPlanError> {
    let mut schema = child.relation.schema.clone();
    let mut domains = child.relation.domains.clone();
    let collection = child.collection.clone();
    let mut functions = Vec::with_capacity(outputs.len());
    for (variable, function) in outputs {
        if schema.iter().any(|candidate| candidate == &variable) {
            return Err(BgpPlanError::CalendarResultAlreadyBound { var: variable });
        }
        schema.push(variable);
        // The derived column is a generated integer, not an executor value domain.
        domains.push(None);
        functions.push(function);
    }
    let relation = CompiledRelation {
        schema,
        domains,
        statement: CompiledRelationStatement::CalendarInt {
            input: Box::new(child.relation),
            date_column,
            outputs: functions,
        },
    };
    Ok(CompiledBgp {
        relation,
        projection: None,
        collection,
        count_variable: None,
        full_row_distinct: false,
    })
}

/// Build a DATE-result calendar [`CompiledBgp`]: the result is TERMINAL/revealed (NOT a committed
/// schema column), so the child schema/domains pass through unchanged and the result variable lives
/// only in the statement — mirroring `plan_concat_form`. The result must be a fresh variable.
fn build_calendar_date(
    child: CompiledBgp,
    input_column: usize,
    op: CalendarDateFn,
    result: String,
) -> Result<CompiledBgp, BgpPlanError> {
    if child
        .relation
        .schema
        .iter()
        .any(|candidate| candidate == &result)
    {
        return Err(BgpPlanError::CalendarResultAlreadyBound { var: result });
    }
    let collection = child.collection.clone();
    let relation = CompiledRelation {
        schema: child.relation.schema.clone(),
        domains: child.relation.domains.clone(),
        statement: CompiledRelationStatement::CalendarDate {
            input: Box::new(child.relation),
            op,
            input_column,
            result,
        },
    };
    Ok(CompiledBgp {
        relation,
        projection: None,
        collection,
        count_variable: None,
        full_row_distinct: false,
    })
}

/// Compile a single-input WOQL CALENDAR op conjunct into a verifier-recompute
/// [`CompiledRelationStatement::CalendarInt`] / [`CompiledRelationStatement::CalendarDate`], modelled
/// on [`plan_eval_form`]/[`plan_concat_form`].
///
/// SHIP (reject-closed everywhere else): `Weekday`/`WeekdaySundayStart`/`IsoWeek` (INTEGER outputs
/// over a bound `xsd:date`) and `DayAfter`/`DayBefore` (`xsd:date`) / `MonthStartDate`/`MonthEndDate`
/// (`xsd:gYearMonth`) (DATE output). The operand must be a variable bound as a nonnullable store
/// node/value column; every result must be a fresh, distinct output variable (validate mode and
/// already-bound results reject closed). The two-input / interval / duration / 3-arg generator ops
/// are claimed and reject-closed ([`BgpPlanError::CalendarTwoInputUnsupported`]).
fn plan_calendar_form(query: &Query) -> Result<Option<CompiledBgp>, BgpPlanError> {
    let conjuncts = match query {
        Query::And(and) => &and.and,
        _ => return Ok(None),
    };
    let positions: Vec<usize> = conjuncts
        .iter()
        .enumerate()
        .filter(|(_, conjunct)| is_calendar_conjunct(conjunct))
        .map(|(index, _)| index)
        .collect();
    if positions.is_empty() {
        // No calendar conjunct: let the other planners handle this query.
        return Ok(None);
    }
    if positions.len() > 1 {
        // Multiple calendar ops would need a dependency-ordered chain; v1 takes exactly one.
        return Err(BgpPlanError::CalendarUnsupportedShape);
    }
    let calendar = &conjuncts[positions[0]];

    // Reject-closed the two-input / interval / duration / range-generator ops UP FRONT (before even
    // building a child) — heavier follow-ups, never approximated.
    let reject_op = match calendar {
        Query::DateDuration(_) => Some("DateDuration"),
        Query::Interval(_) => Some("Interval"),
        Query::IntervalStartDuration(_) => Some("IntervalStartDuration"),
        Query::IntervalDurationEnd(_) => Some("IntervalDurationEnd"),
        Query::IntervalRelation(_) => Some("IntervalRelation"),
        Query::IntervalRelationTyped(_) => Some("IntervalRelationTyped"),
        Query::MonthStartDates(_) => Some("MonthStartDates"),
        Query::MonthEndDates(_) => Some("MonthEndDates"),
        _ => None,
    };
    if let Some(op) = reject_op {
        return Err(BgpPlanError::CalendarTwoInputUnsupported { op });
    }

    // The operand-binding child is every OTHER conjunct.
    let child_conjuncts: Vec<Query> = conjuncts
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != positions[0])
        .map(|(_, conjunct)| conjunct.clone())
        .collect();
    if child_conjuncts.is_empty() {
        // A bare calendar op with no store binding cannot authenticate its operand.
        return Err(BgpPlanError::CalendarUnsupportedShape);
    }
    let child_query = if child_conjuncts.len() == 1 {
        child_conjuncts.into_iter().next().unwrap()
    } else {
        Query::And(And {
            and: child_conjuncts,
        })
    };
    let child = plan_bgp(&child_query)?;
    if child.projection.is_some() || child.count_variable.is_some() || child.full_row_distinct {
        return Err(BgpPlanError::CalendarUnsupportedShape);
    }

    let date_operand = |operand: &DataValue, child: &CompiledBgp| {
        calendar_operand_column(
            operand,
            child,
            CanonicalDatatype::Date,
            |var| BgpPlanError::CalendarOperandNotBound { var },
            |var| BgpPlanError::CalendarOperandNotDate { var },
        )
    };
    let yearmonth_operand = |operand: &DataValue, child: &CompiledBgp| {
        calendar_operand_column(
            operand,
            child,
            CanonicalDatatype::GYearMonth,
            |var| BgpPlanError::CalendarOperandNotBound { var },
            |var| BgpPlanError::CalendarOperandNotYearMonth { var },
        )
    };

    let compiled = match calendar {
        Query::Weekday(weekday) => {
            let date_column = date_operand(&weekday.date, &child)?;
            let result = calendar_result_var(&weekday.weekday)?;
            build_calendar_int(child, date_column, vec![(result, CalendarIntFn::Weekday)])?
        }
        Query::WeekdaySundayStart(weekday) => {
            let date_column = date_operand(&weekday.date, &child)?;
            let result = calendar_result_var(&weekday.weekday)?;
            build_calendar_int(child, date_column, vec![(result, CalendarIntFn::WeekdaySundayStart)])?
        }
        Query::IsoWeek(iso) => {
            let date_column = date_operand(&iso.date, &child)?;
            // The reference binds BOTH the week-numbering year and the week number; both must be
            // fresh, distinct output variables (order [year, week] matches `definition.pl` fields).
            let year = calendar_result_var(&iso.year)?;
            let week = calendar_result_var(&iso.week)?;
            build_calendar_int(
                child,
                date_column,
                vec![
                    (year, CalendarIntFn::IsoWeekYear),
                    (week, CalendarIntFn::IsoWeekWeek),
                ],
            )?
        }
        Query::DayAfter(day) => {
            let input_column = date_operand(&day.date, &child)?;
            let result = calendar_result_var(&day.next)?;
            build_calendar_date(child, input_column, CalendarDateFn::DayAfter, result)?
        }
        Query::DayBefore(day) => {
            let input_column = date_operand(&day.date, &child)?;
            let result = calendar_result_var(&day.previous)?;
            build_calendar_date(child, input_column, CalendarDateFn::DayBefore, result)?
        }
        Query::MonthStartDate(month) => {
            let input_column = yearmonth_operand(&month.year_month, &child)?;
            let result = calendar_result_var(&month.date)?;
            build_calendar_date(child, input_column, CalendarDateFn::MonthStartDate, result)?
        }
        Query::MonthEndDate(month) => {
            let input_column = yearmonth_operand(&month.year_month, &child)?;
            let result = calendar_result_var(&month.date)?;
            build_calendar_date(child, input_column, CalendarDateFn::MonthEndDate, result)?
        }
        _ => unreachable!("is_calendar_conjunct + the reject filter cover every calendar variant"),
    };
    Ok(Some(compiled))
}

/// Compile a woql2 conjunctive BGP query into a safe [`CompiledBgp`].
///
/// Accepts nested outer [`Select`]s and supported routing/result wrappers around an
/// `And` of `>= 1` triples of
/// the form `?subj <constant IRI> ?obj`, whose atoms connect into a
/// left-deep join tree (each step shares one or two variables). Everything
/// else is rejected closed with a descriptive [`BgpPlanError`].
pub fn plan_bgp(query: &Query) -> Result<CompiledBgp, BgpPlanError> {
    // (0a) Reject the triple-SHAPED-but-not-BGP leaves (`TypeOf`/`Dot`/`IsA`) up front —
    // before any lowering or shape analysis — so a NESTED occurrence gets its specific
    // soundness rationale and the guard COMPOSES with the path lowering below (a query
    // carrying both a `Path` and one of these leaves rejects on the leaf). See
    // `reject_non_bgp_typed_leaves` for why each is unsound to lower to a store triple.
    reject_non_bgp_typed_leaves(query)?;

    // (0b) Lower fixed-length property paths (`Query::Path`) to BGP + bag union before
    // any shape analysis, so all soundness reuses the audited BGP machinery below.
    let lowered_query;
    let query = if query_contains_path(query) {
        let mut reserved = std::collections::BTreeSet::new();
        collect_reserved_path_variables(query, &mut reserved);
        let mut alloc = FreshPathVars::new(reserved);
        lowered_query = rewrite_query_paths(query, &mut alloc)?;
        &lowered_query
    } else {
        query
    };

    // (1) Unwrap proof-transparent outer controls. `Using` is not discarded: it is
    // retained as routing metadata so an integration can resolve the intended layer.
    // `From(instance)` is safe for the instance-layer proof path; schema or wildcard
    // reads reject closed. `Pin` affects optimization order, not query semantics.
    // Traversal is outer-to-inner. The bool records whether the Select is outside a
    // Count (and therefore selects the Count binding) or inside it (and therefore
    // projects the counted relation before its cardinality is taken).
    let mut selections: Vec<(&[String], bool)> = Vec::new();
    let mut collection: Option<String> = None;
    let mut count_variable: Option<String> = None;
    let mut distincts: Vec<&[String]> = Vec::new();
    let mut body = query;
    loop {
        body = match body {
            Query::Select(select) => {
                selections.push((select.variables.as_slice(), count_variable.is_none()));
                select.query.as_ref()
            }
            Query::Using(using) => {
                // Nested Using scopes replace the active collection for their inner query;
                // the innermost scope is therefore the layer-routing metadata we retain.
                collection = Some(using.collection.clone());
                using.query.as_ref()
            }
            Query::From(from) if from.graph.eq_ignore_ascii_case("instance") => from.query.as_ref(),
            Query::From(_) => return Err(BgpPlanError::SchemaGraphUnsupported),
            Query::Pin(pin) => pin.query.as_ref(),
            Query::Immediately(immediately) => immediately.query.as_ref(),
            Query::Distinct(distinct) => {
                distincts.push(distinct.variables.as_slice());
                distinct.query.as_ref()
            }
            Query::Count(count) if count_variable.is_none() => {
                count_variable = match &count.count {
                    DataValue::Variable(variable) => Some(variable.clone()),
                    _ => return Err(BgpPlanError::CountOutputNotVariable),
                };
                count.query.as_ref()
            }
            _ => break,
        };
    }

    // A finite single-predicate REPEATED-predicate path that `lower_path` DEFERRED (a plain
    // bag-join over-counts edge-simple solutions). Compile it at the plan level: the join
    // chain + a `TupleDisequality` edge-simple filter + endpoint projection. A deferred path
    // that is NOT the whole body (nested inside a larger `And`) reaches the conjunctive
    // compiler as a non-triple conjunct and rejects closed there.
    if let Query::Path(path) = body {
        // A `Star`/`Plus` closure routes to the transitive-closure planner; every other deferred path
        // (single-predicate repeat / `Times`) routes to the edge-simple TupleDisequality planner.
        if matches!(
            path.pattern,
            crate::path::PathPattern::Star(_) | crate::path::PathPattern::Plus(_)
        ) {
            return plan_transitive_closure_path(
                path,
                &selections,
                collection,
                count_variable,
                &distincts,
            );
        }
        return plan_deferred_repeated_predicate_path(
            path,
            &selections,
            collection,
            count_variable,
            &distincts,
        );
    }

    // SCHEMA-GRAPH FOUNDATION — `Subsumption child << parent` compiles to the reflexive-transitive
    // `sys:inherits` closure over the SCHEMA graph, Distinct-gated (the reference's intrinsic
    // `distinct([child,parent])`). Handled here — after outer-control unwrapping, alongside the path
    // closure — so it reuses the same audited transitive-closure machinery, only pointed at the schema
    // graph. Every unsupported subsumption shape (both endpoints free, count-gated, …) rejects closed.
    if let Query::Subsumption(sub) = body {
        return plan_subsumption(sub, &selections, collection, count_variable, &distincts);
    }

    // SCHEMA-GRAPH FOUNDATION — `IsA(?x, C)` (node regime, bound class) lowers to a CROSS-DICTIONARY
    // two-root hash join: the downward `sys:inherits` closure of `C` (schema graph) hash-joined against
    // the instance `rdf:type` relation (instance graph). Standalone only (a nested `IsA` inside a BGP
    // conjunction rejects via `reject_non_bgp_typed_leaves`). See [`plan_isa`].
    if let Query::IsA(isa) = body {
        return plan_isa(isa, &selections, collection, count_variable, &distincts);
    }

    if matches!(body, Query::Start(_) | Query::Limit(_) | Query::Once(_)) {
        if (distincts.is_empty() && (!selections.is_empty() || count_variable.is_some()))
            || (!distincts.is_empty() && count_variable.is_none())
        {
            return Err(BgpPlanError::OrderedSliceUnsupportedShape);
        }
        let mut compiled = plan_exact_ordered_slice(body)?;
        if compiled.collection.is_none() {
            compiled.collection = collection;
        }
        let (relation, full_row_distinct) = compile_distinct_relation(
            compiled.relation,
            &distincts,
            &selections,
            count_variable.as_deref(),
        )?;
        compiled.projection = compile_projection(
            &selections,
            count_variable.as_deref(),
            &relation.schema,
        )?;
        compiled.relation = relation;
        compiled.count_variable = count_variable;
        compiled.full_row_distinct = full_row_distinct;
        validate_compiled_structure(&compiled.relation)?;
        return Ok(compiled);
    }

    if let Query::OrderBy(order_by) = body {
        if (distincts.is_empty() && (!selections.is_empty() || count_variable.is_some()))
            || (!distincts.is_empty() && count_variable.is_none())
        {
            return Err(BgpPlanError::OrderByUnsupportedShape);
        }
        let mut compiled = plan_exact_scalar_order(order_by)?;
        // Prolog's nested routing scope is innermost-wins. Preserve an inner collection
        // already found while compiling the ordered child; otherwise retain this outer one.
        if compiled.collection.is_none() {
            compiled.collection = collection;
        }
        let (relation, full_row_distinct) = compile_distinct_relation(
            compiled.relation,
            &distincts,
            &selections,
            count_variable.as_deref(),
        )?;
        compiled.projection = compile_projection(
            &selections,
            count_variable.as_deref(),
            &relation.schema,
        )?;
        compiled.relation = relation;
        compiled.count_variable = count_variable;
        compiled.full_row_distinct = full_row_distinct;
        validate_compiled_structure(&compiled.relation)?;
        return Ok(compiled);
    }

    if let Some(mut compiled) = plan_ordered_triple_value_construct(body)? {
        if compiled.collection.is_none() {
            compiled.collection = collection;
        }
        let (relation, full_row_distinct) = compile_distinct_relation(
            compiled.relation,
            &distincts,
            &selections,
            count_variable.as_deref(),
        )?;
        compiled.projection = compile_projection(
            &selections,
            count_variable.as_deref(),
            &relation.schema,
        )?;
        compiled.relation = relation;
        compiled.count_variable = count_variable;
        compiled.full_row_distinct = full_row_distinct;
        validate_compiled_structure(&compiled.relation)?;
        return Ok(compiled);
    }

    if let Some(mut compiled) = plan_triple_count_comparison_body(body)? {
        let (relation, full_row_distinct) = compile_distinct_relation(
            compiled.relation,
            &distincts,
            &selections,
            count_variable.as_deref(),
        )?;
        compiled.projection = compile_projection(
            &selections,
            count_variable.as_deref(),
            &relation.schema,
        )?;
        compiled.relation = relation;
        compiled.count_variable = count_variable;
        compiled.full_row_distinct = full_row_distinct;
        validate_compiled_structure(&compiled.relation)?;
        return Ok(compiled);
    }

    if let Query::Or(disjunction) = body {
        return plan_or_relation(
            disjunction,
            &selections,
            collection,
            count_variable,
            &distincts,
        );
    }

    if let Some(mut aggregate) = plan_collection_extrema_form(body)? {
        if collection.is_some() {
            if aggregate.collection.is_some() && aggregate.collection != collection {
                return Err(BgpPlanError::OrRoutingConflict);
            }
            aggregate.collection = collection;
        }
        aggregate.count_variable = count_variable;
        aggregate.projection = compile_projection(
            &selections,
            aggregate.count_variable.as_deref(),
            &aggregate.relation.schema,
        )?;
        if !distincts.is_empty() {
            return Err(BgpPlanError::DistinctWithProjectionOrCount);
        }
        validate_compiled_structure(&aggregate.relation)?;
        return Ok(aggregate);
    }

    if let Some(mut aggregate) = plan_collection_sum_form(body)? {
        if collection.is_some() {
            if aggregate.collection.is_some() && aggregate.collection != collection {
                return Err(BgpPlanError::OrRoutingConflict);
            }
            aggregate.collection = collection;
        }
        aggregate.count_variable = count_variable;
        aggregate.projection = compile_projection(
            &selections,
            aggregate.count_variable.as_deref(),
            &aggregate.relation.schema,
        )?;
        if !distincts.is_empty() {
            return Err(BgpPlanError::DistinctWithProjectionOrCount);
        }
        validate_compiled_structure(&aggregate.relation)?;
        return Ok(aggregate);
    }

    if let Some(mut aggregate) = plan_collection_count_form(body)? {
        if collection.is_some() {
            if aggregate.collection.is_some() && aggregate.collection != collection {
                return Err(BgpPlanError::OrRoutingConflict);
            }
            aggregate.collection = collection;
        }
        aggregate.count_variable = count_variable;
        aggregate.projection = compile_projection(
            &selections,
            aggregate.count_variable.as_deref(),
            &aggregate.relation.schema,
        )?;
        if !distincts.is_empty() {
            return Err(BgpPlanError::DistinctWithProjectionOrCount);
        }
        validate_compiled_structure(&aggregate.relation)?;
        return Ok(aggregate);
    }

    if let Some(mut aggregate) = plan_group_count_form(body)? {
        if collection.is_some() {
            if aggregate.collection.is_some() && aggregate.collection != collection {
                return Err(BgpPlanError::OrRoutingConflict);
            }
            aggregate.collection = collection;
        }
        aggregate.count_variable = count_variable;
        aggregate.projection = compile_projection(
            &selections,
            aggregate.count_variable.as_deref(),
            &aggregate.relation.schema,
        )?;
        if !distincts.is_empty() {
            return Err(BgpPlanError::DistinctWithProjectionOrCount);
        }
        validate_compiled_structure(&aggregate.relation)?;
        return Ok(aggregate);
    }

    if let Some(mut aggregate) = plan_group_sum_form(body)? {
        if collection.is_some() {
            if aggregate.collection.is_some() && aggregate.collection != collection {
                return Err(BgpPlanError::OrRoutingConflict);
            }
            aggregate.collection = collection;
        }
        aggregate.count_variable = count_variable;
        aggregate.projection = compile_projection(
            &selections,
            aggregate.count_variable.as_deref(),
            &aggregate.relation.schema,
        )?;
        if !distincts.is_empty() {
            return Err(BgpPlanError::DistinctWithProjectionOrCount);
        }
        validate_compiled_structure(&aggregate.relation)?;
        return Ok(aggregate);
    }

    if let Some(mut aggregate) = plan_group_extrema_form(body)? {
        if collection.is_some() {
            if aggregate.collection.is_some() && aggregate.collection != collection {
                return Err(BgpPlanError::OrRoutingConflict);
            }
            aggregate.collection = collection;
        }
        aggregate.count_variable = count_variable;
        aggregate.projection = compile_projection(
            &selections,
            aggregate.count_variable.as_deref(),
            &aggregate.relation.schema,
        )?;
        if !distincts.is_empty() {
            return Err(BgpPlanError::DistinctWithProjectionOrCount);
        }
        validate_compiled_structure(&aggregate.relation)?;
        return Ok(aggregate);
    }

    if let Some(mut evaluated) = plan_eval_form(body)? {
        if collection.is_some() {
            if evaluated.collection.is_some() && evaluated.collection != collection {
                return Err(BgpPlanError::OrRoutingConflict);
            }
            evaluated.collection = collection;
        }
        evaluated.count_variable = count_variable;
        evaluated.projection = compile_projection(
            &selections,
            evaluated.count_variable.as_deref(),
            &evaluated.relation.schema,
        )?;
        if !distincts.is_empty() {
            return Err(BgpPlanError::DistinctWithProjectionOrCount);
        }
        validate_compiled_structure(&evaluated.relation)?;
        return Ok(evaluated);
    }

    if let Some(mut cast) = plan_typecast_form(body)? {
        if collection.is_some() {
            if cast.collection.is_some() && cast.collection != collection {
                return Err(BgpPlanError::OrRoutingConflict);
            }
            cast.collection = collection;
        }
        cast.count_variable = count_variable;
        cast.projection = compile_projection(
            &selections,
            cast.count_variable.as_deref(),
            &cast.relation.schema,
        )?;
        if !distincts.is_empty() {
            return Err(BgpPlanError::DistinctWithProjectionOrCount);
        }
        validate_compiled_structure(&cast.relation)?;
        return Ok(cast);
    }

    if let Some(mut concatenated) = plan_concat_form(body)? {
        if collection.is_some() {
            if concatenated.collection.is_some() && concatenated.collection != collection {
                return Err(BgpPlanError::OrRoutingConflict);
            }
            concatenated.collection = collection;
        }
        // The concat result is terminal/revealed (not a committed schema column), so it cannot be
        // projected or counted; a Select/Count naming it fails via `compile_projection`'s
        // SelectedVariableNotInBgp / a count over the child columns. Distinct is rejected too.
        concatenated.count_variable = count_variable;
        concatenated.projection = compile_projection(
            &selections,
            concatenated.count_variable.as_deref(),
            &concatenated.relation.schema,
        )?;
        if !distincts.is_empty() {
            return Err(BgpPlanError::DistinctWithProjectionOrCount);
        }
        validate_compiled_structure(&concatenated.relation)?;
        return Ok(concatenated);
    }

    if let Some(mut joined) = plan_join_form(body)? {
        if collection.is_some() {
            if joined.collection.is_some() && joined.collection != collection {
                return Err(BgpPlanError::OrRoutingConflict);
            }
            joined.collection = collection;
        }
        // The join result is terminal/revealed (not a committed schema column): a Select/Count
        // naming it fails via `compile_projection`; Distinct is rejected too.
        joined.count_variable = count_variable;
        joined.projection = compile_projection(
            &selections,
            joined.count_variable.as_deref(),
            &joined.relation.schema,
        )?;
        if !distincts.is_empty() {
            return Err(BgpPlanError::DistinctWithProjectionOrCount);
        }
        validate_compiled_structure(&joined.relation)?;
        return Ok(joined);
    }

    if let Some(mut substr) = plan_substring_form(body)? {
        if collection.is_some() {
            if substr.collection.is_some() && substr.collection != collection {
                return Err(BgpPlanError::OrRoutingConflict);
            }
            substr.collection = collection;
        }
        // The substring result is terminal/revealed (not a committed schema column): a
        // Select/Count naming it (or `?after`) fails via `compile_projection`; Distinct too.
        substr.count_variable = count_variable;
        substr.projection = compile_projection(
            &selections,
            substr.count_variable.as_deref(),
            &substr.relation.schema,
        )?;
        if !distincts.is_empty() {
            return Err(BgpPlanError::DistinctWithProjectionOrCount);
        }
        validate_compiled_structure(&substr.relation)?;
        return Ok(substr);
    }

    if let Some(mut padded) = plan_pad_form(body)? {
        if collection.is_some() {
            if padded.collection.is_some() && padded.collection != collection {
                return Err(BgpPlanError::OrRoutingConflict);
            }
            padded.collection = collection;
        }
        // The pad result is terminal/revealed (not a committed schema column): a Select/Count
        // naming it fails via `compile_projection`; Distinct is rejected too.
        padded.count_variable = count_variable;
        padded.projection = compile_projection(
            &selections,
            padded.count_variable.as_deref(),
            &padded.relation.schema,
        )?;
        if !distincts.is_empty() {
            return Err(BgpPlanError::DistinctWithProjectionOrCount);
        }
        validate_compiled_structure(&padded.relation)?;
        return Ok(padded);
    }

    if let Some(mut hashed) = plan_hashkey_form(body)? {
        if collection.is_some() {
            if hashed.collection.is_some() && hashed.collection != collection {
                return Err(BgpPlanError::OrRoutingConflict);
            }
            hashed.collection = collection;
        }
        // The hash result is terminal/revealed (not a committed schema column): a Select/Count
        // naming it fails via `compile_projection`; Distinct is rejected too.
        hashed.count_variable = count_variable;
        hashed.projection = compile_projection(
            &selections,
            hashed.count_variable.as_deref(),
            &hashed.relation.schema,
        )?;
        if !distincts.is_empty() {
            return Err(BgpPlanError::DistinctWithProjectionOrCount);
        }
        validate_compiled_structure(&hashed.relation)?;
        return Ok(hashed);
    }

    if let Some(mut calendar) = plan_calendar_form(body)? {
        if collection.is_some() {
            if calendar.collection.is_some() && calendar.collection != collection {
                return Err(BgpPlanError::OrRoutingConflict);
            }
            calendar.collection = collection;
        }
        // Integer-result calendar ops APPEND their result column(s) to the schema (Select/Count over
        // them works); date-result calendar ops keep the result TERMINAL/revealed (a Select/Count
        // naming it fails via `compile_projection`, exactly like Concat). Distinct is rejected too.
        calendar.count_variable = count_variable;
        calendar.projection = compile_projection(
            &selections,
            calendar.count_variable.as_deref(),
            &calendar.relation.schema,
        )?;
        if !distincts.is_empty() {
            return Err(BgpPlanError::DistinctWithProjectionOrCount);
        }
        validate_compiled_structure(&calendar.relation)?;
        return Ok(calendar);
    }

    // Deferred string-case ops (`Upper`/`Lower`) reject closed with a documented locale reason
    // (see `reject_unsupported_string_case_ops`) before the generic UnsupportedConstruct path.
    reject_unsupported_string_case_ops(body)?;

    if has_top_level_read_operator(body) {
        return plan_sequential_read_operators(
            body,
            &selections,
            collection,
            count_variable,
            &distincts,
        );
    }
    if !matches!(
        body,
        Query::And(_)
            | Query::Triple(_)
            | Query::Link(_)
            | Query::Data(_)
            | Query::AddedTriple(_)
            | Query::DeletedTriple(_)
    ) {
        return Err(BgpPlanError::UnsupportedConstruct {
            construct: query_variant_name(body),
        });
    }

    // (1b) Flatten nested Ands. Triple leaves remain kind-agnostic; Link/Data leaves
    // explicitly authenticate the node/literal half of the Store object dictionary.
    // Comparisons become object-value range filters. True and Comment contribute no
    // bindings and are proof-transparent conjunction leaves.
    let mut triples: Vec<TypedTriple> = Vec::new();
    let mut comparisons: Vec<NormalizedRange> = Vec::new();
    collect_conjunct(body, &mut triples, &mut comparisons)?;

    // (1c) CP-D: each atom carries its OWN column source (materialized head, or this layer's
    // additions/removals delta), so a CROSS-SOURCE conjunction — e.g. `AddedTriple(?x,p,?y) &
    // Triple(?y,q,?z)` (additions ⋈ materialized) or additions ⋈ removals — is now supported:
    // all sources share the layer's predicate/node-value dictionaries, so their IDs are
    // directly comparable and the audited store join gadget joins them unchanged. We only
    // compute the plan DEFAULT source here (the uniform source when every atom agrees, else
    // materialized); the per-atom sources are threaded onto each `LayerBgpAtom` below.
    let source = plan_default_column_source(&triples);

    // (2) Lift each triple to an Atom, with all per-atom safety checks.
    let mut atoms: Vec<Atom> = triples
        .iter()
        .map(atom_from_triple)
        .collect::<Result<_, _>>()?;

    let k = atoms.len();
    if k == 0 {
        return Err(BgpPlanError::TooFewAtoms { count: k });
    }
    validate_variable_namespaces(&atoms)?;

    // (2b) Phase 7.4: route each comparison conjunct to a numeric RANGE filter on the
    // object of the (unique) atom that binds the compared variable. Consumes the
    // comparisons; the ranged variable stays a kept output column.
    attach_ranges(&mut atoms, &comparisons)?;

    // The Store's mixed-arity plan is a relational prefix followed by zero-arity
    // existence bits. Stable ordering preserves query order within both groups.
    atoms.sort_by_key(|atom| atom.kept.is_empty());

    // (3) Shape guard + left-deep planner: the safety-critical core. Each ordered plan atom
    // already carries its own `source` (threaded via `atom_from_triple` and cloned through by
    // `plan_left_deep`), so cross-source joins are represented directly.
    let (mut plan, schema) = plan_left_deep(&atoms)?;
    // Stamp the plan DEFAULT source (uniform source, else materialized) — the transcript
    // baseline the store measures per-atom overrides against, and the domain of the all-ground
    // unit. The verifier reconstructs this same plan (default + every atom's source) from its
    // own trusted query, so all sources are fixed by trusted compilation; `plan_digest`/the
    // Fiat–Shamir transcript additionally bind them.
    plan.source = source;
    let domains = infer_schema_domains(&schema, &atoms, &comparisons);

    let relation = CompiledRelation {
        schema: schema.clone(),
        domains,
        statement: CompiledRelationStatement::Bgp(plan),
    };
    let (relation, full_row_distinct) = compile_distinct_relation(
        relation,
        &distincts,
        &selections,
        count_variable.as_deref(),
    )?;
    let projection = compile_projection(
        &selections,
        count_variable.as_deref(),
        &relation.schema,
    )?;

    Ok(CompiledBgp {
        relation,
        projection,
        collection,
        count_variable,
        full_row_distinct,
    })
}

fn infer_schema_domains(
    schema: &[String],
    atoms: &[Atom],
    comparisons: &[NormalizedRange],
) -> Vec<Option<EqualityValueDomain>> {
    schema
        .iter()
        .map(|variable| {
            if atoms
                .iter()
                .any(|atom| atom.pred_var.as_ref() == Some(variable))
            {
                Some(EqualityValueDomain::Predicate)
            } else if atoms
                .iter()
                .any(|atom| atom.subj_var.as_ref() == Some(variable))
            {
                Some(EqualityValueDomain::Node)
            } else {
                comparisons
                    .iter()
                    .find(|comparison| &comparison.var == variable)
                    .map(|comparison| EqualityValueDomain::Datatype(comparison.canonical_datatype))
            }
        })
        .collect()
}

fn plan_or_relation(
    disjunction: &crate::query::Or,
    selections: &[(&[String], bool)],
    collection: Option<String>,
    count_variable: Option<String>,
    distincts: &[&[String]],
) -> Result<CompiledBgp, BgpPlanError> {
    if disjunction.or.len() < 2 {
        return Err(BgpPlanError::OrTooFewBranches {
            count: disjunction.or.len(),
        });
    }
    let mut compiled = Vec::with_capacity(disjunction.or.len());
    for query in &disjunction.or {
        let branch = plan_bgp(query)?;
        let internal_ground_projection = branch.relation.schema.is_empty()
            && branch.projection.as_deref() == Some(&[]);
        if (branch.projection.is_some() && !internal_ground_projection)
            || branch.count_variable.is_some()
            || branch.full_row_distinct
        {
            return Err(BgpPlanError::OrUnsupportedBranchWrapper);
        }
        compiled.push(branch);
    }
    let schema = compiled[0].relation.schema.clone();
    let mut effective_collection = collection;
    for branch in &compiled {
        merge_collection(
            &mut effective_collection,
            branch.collection.as_ref(),
            BgpPlanError::OrRoutingConflict,
        )?;
    }

    let mut branches = Vec::new();
    let mut mappings = Vec::new();
    let mut expected_descriptors = None;
    let mut merged_domains: Option<Vec<Option<EqualityValueDomain>>> = None;
    for branch in compiled {
        let mapping = schema_mapping(&schema, &branch.relation.schema)
            .ok_or(BgpPlanError::OrIncompatibleSchema)?;
        let descriptors = relation_result_descriptors(&branch.relation)
            .map_err(|_| BgpPlanError::OrIncompatibleNamespaces)?;
        let mapped: Vec<_> = mapping.iter().map(|index| descriptors[*index]).collect();
        if expected_descriptors
            .as_ref()
            .is_some_and(|expected| expected != &mapped)
        {
            return Err(BgpPlanError::OrIncompatibleNamespaces);
        }
        expected_descriptors = Some(mapped);
        let mapped_domains = mapping
            .iter()
            .map(|index| branch.relation.domains[*index])
            .collect::<Vec<_>>();
        if let Some(prior) = &mut merged_domains {
            for (prior, current) in prior.iter_mut().zip(&mapped_domains) {
                if *prior != *current {
                    *prior = None;
                }
            }
        } else {
            merged_domains = Some(mapped_domains);
        }
        match branch.relation.statement {
            CompiledRelationStatement::BagUnion {
                branches: nested,
                mappings: nested_mappings,
            } => {
                for (nested, nested_mapping) in nested.into_iter().zip(nested_mappings) {
                    mappings.push(mapping.iter().map(|index| nested_mapping[*index]).collect());
                    branches.push(nested);
                }
            }
            statement => {
                mappings.push(if schema.is_empty() {
                    Vec::new()
                } else {
                    mapping
                });
                branches.push(CompiledRelation {
                    schema: branch.relation.schema,
                    domains: branch.relation.domains,
                    statement,
                });
            }
        }
    }
    let relation = CompiledRelation {
        schema,
        domains: merged_domains.unwrap_or_default(),
        statement: CompiledRelationStatement::BagUnion { branches, mappings },
    };
    let (relation, full_row_distinct) = compile_distinct_relation(
        relation,
        distincts,
        selections,
        count_variable.as_deref(),
    )?;
    let projection = compile_projection(
        selections,
        count_variable.as_deref(),
        &relation.schema,
    )?;
    let compiled = CompiledBgp {
        relation,
        projection,
        collection: effective_collection,
        count_variable,
        full_row_distinct,
    };
    validate_compiled_structure(&compiled.relation)?;
    Ok(compiled)
}

enum ReadOperator {
    Optional(Query),
    Not(Query),
    Equals(crate::compare::Equals),
    When(crate::control::When),
}

fn has_top_level_read_operator(query: &Query) -> bool {
    match query {
        Query::And(and) => and.and.iter().any(has_top_level_read_operator),
        Query::WoqlOptional(_) | Query::Not(_) | Query::Equals(_) | Query::If(_) => true,
        _ => false,
    }
}

fn collect_read_operator_sequence(
    query: &Query,
    positive: &mut Vec<Query>,
    operators: &mut Vec<ReadOperator>,
) -> Result<(), BgpPlanError> {
    match query {
        Query::And(and) => {
            for child in &and.and {
                collect_read_operator_sequence(child, positive, operators)?;
            }
        }
        Query::WoqlOptional(optional) => {
            operators.push(ReadOperator::Optional((*optional.query).clone()))
        }
        Query::Not(not) => operators.push(ReadOperator::Not((*not.query).clone())),
        Query::Equals(equals) => operators.push(ReadOperator::Equals(equals.clone())),
        Query::If(when) => operators.push(ReadOperator::When(when.clone())),
        other if operators.is_empty() => positive.push(other.clone()),
        other => {
            return Err(BgpPlanError::ConjunctNotTriple {
                construct: query_variant_name(other),
            })
        }
    }
    Ok(())
}

#[derive(Clone)]
enum EqualityOperand {
    Variable(String),
    Constant(CanonicalObject),
}

fn equality_operand(value: &crate::value::WoqlValue) -> Result<EqualityOperand, BgpPlanError> {
    match value {
        crate::value::WoqlValue::Variable(variable) => {
            Ok(EqualityOperand::Variable(variable.clone()))
        }
        crate::value::WoqlValue::Node(iri) => Ok(EqualityOperand::Constant(CanonicalObject::Node(
            iri.clone().into_bytes(),
        ))),
        crate::value::WoqlValue::Data(value) => {
            let term = value_constant_term(value)?;
            match term {
                AtomTerm::ValueConstant(datatype, lexical) => {
                    Ok(EqualityOperand::Constant(CanonicalObject::Value {
                        datatype,
                        lexical,
                    }))
                }
                _ => unreachable!("value_constant_term returned a non-value"),
            }
        }
        crate::value::WoqlValue::Dictionary(_) | crate::value::WoqlValue::List(_) => {
            Err(BgpPlanError::EqualsUnsupportedOperand)
        }
    }
}

fn numeric_equality_datatype(datatype: CanonicalDatatype) -> bool {
    use CanonicalDatatype::*;
    matches!(
        datatype,
        UInt8
            | Int8
            | UInt16
            | Int16
            | UInt32
            | Int32
            | Float32
            | UInt64
            | Int64
            | Float64
            | Decimal
            | BigInt
            | PositiveInteger
            | NonNegativeInteger
            | NonPositiveInteger
            | NegativeInteger
    )
}

fn bound_equality_column(
    relation: &CompiledRelation,
    variable: &str,
) -> Result<(usize, EqualityValueDomain), BgpPlanError> {
    let column = relation
        .schema
        .iter()
        .position(|candidate| candidate == variable)
        .ok_or_else(|| BgpPlanError::EqualsUnboundVariable {
            var: variable.to_string(),
        })?;
    let descriptors =
        relation_result_descriptors(relation).map_err(|_| BgpPlanError::EqualsDomainMismatch)?;
    if descriptors[column].nullable {
        return Err(BgpPlanError::EqualsNullableVariable {
            var: variable.to_string(),
        });
    }
    let domain = relation.domains[column].ok_or_else(|| BgpPlanError::EqualsUnknownDomain {
        var: variable.to_string(),
    })?;
    Ok((column, domain))
}

fn constant_matches_domain(value: &CanonicalObject, domain: EqualityValueDomain) -> bool {
    match (value, domain) {
        (CanonicalObject::Node(_), EqualityValueDomain::Node) => true,
        (CanonicalObject::Value { datatype, .. }, EqualityValueDomain::Datatype(expected)) => {
            *datatype == expected
        }
        _ => false,
    }
}

fn compile_equality_constraint(
    relation: &CompiledRelation,
    equals: &crate::compare::Equals,
) -> Result<RelationEqualityPlan, BgpPlanError> {
    let left = equality_operand(&equals.left)?;
    let right = equality_operand(&equals.right)?;
    match (left, right) {
        (EqualityOperand::Variable(left), EqualityOperand::Variable(right)) => {
            let (left_column, left_domain) = bound_equality_column(relation, &left)?;
            let (right_column, right_domain) = bound_equality_column(relation, &right)?;
            if left_domain != right_domain {
                return Err(BgpPlanError::EqualsDomainMismatch);
            }
            Ok(RelationEqualityPlan::Columns {
                left: left_column,
                right: right_column,
                domain: left_domain,
            })
        }
        (EqualityOperand::Variable(variable), EqualityOperand::Constant(value))
        | (EqualityOperand::Constant(value), EqualityOperand::Variable(variable)) => {
            let constant_on_left = matches!(
                equality_operand(&equals.left)?,
                EqualityOperand::Constant(_)
            );
            let (column, domain) = bound_equality_column(relation, &variable)?;
            if !constant_matches_domain(&value, domain) {
                return Err(BgpPlanError::EqualsDomainMismatch);
            }
            Ok(RelationEqualityPlan::ColumnConstant {
                column,
                domain,
                value,
                constant_on_left,
            })
        }
        (EqualityOperand::Constant(left), EqualityOperand::Constant(right)) => {
            if let (
                CanonicalObject::Value { datatype: left, .. },
                CanonicalObject::Value {
                    datatype: right, ..
                },
            ) = (&left, &right)
            {
                if left != right
                    && (numeric_equality_datatype(*left) || numeric_equality_datatype(*right))
                {
                    return Err(BgpPlanError::EqualsCoerciveNumeric);
                }
            }
            Ok(RelationEqualityPlan::Constants { left, right })
        }
    }
}

fn inner_join_relation(
    left: CompiledRelation,
    right: CompiledRelation,
    key_pairs: Vec<(usize, usize)>,
) -> CompiledRelation {
    let shared_right = key_pairs.iter().map(|pair| pair.1).collect::<Vec<_>>();
    let mut schema = left.schema.clone();
    let mut domains = left.domains.clone();
    schema.extend(
        right
            .schema
            .iter()
            .enumerate()
            .filter(|(index, _)| !shared_right.contains(index))
            .map(|(_, name)| name.clone()),
    );
    domains.extend(
        right
            .domains
            .iter()
            .enumerate()
            .filter(|(index, _)| !shared_right.contains(index))
            .map(|(_, domain)| *domain),
    );
    CompiledRelation {
        schema,
        domains,
        statement: CompiledRelationStatement::InnerJoin {
            left: Box::new(left),
            right: Box::new(right),
            key_pairs,
        },
    }
}

fn first_local_in_schema(schema: &[String], locals: &[String]) -> Option<String> {
    schema.iter().find(|name| locals.contains(name)).cloned()
}

fn later_operator_local(
    operator: &ReadOperator,
    locals: &[String],
) -> Result<Option<String>, BgpPlanError> {
    match operator {
        ReadOperator::Optional(query) | ReadOperator::Not(query) => {
            let compiled = plan_bgp(query)?;
            Ok(first_local_in_schema(&compiled.relation.schema, locals))
        }
        ReadOperator::Equals(equals) => {
            for value in [&equals.left, &equals.right] {
                if let Value::Variable(name) = value {
                    if locals.contains(name) {
                        return Ok(Some(name.clone()));
                    }
                }
            }
            Ok(None)
        }
        ReadOperator::When(when) => {
            let condition = plan_bgp(&when.query)?;
            if let Some(local) = first_local_in_schema(&condition.relation.schema, locals) {
                return Ok(Some(local));
            }
            let consequent = plan_bgp(&when.consequent)?;
            Ok(first_local_in_schema(&consequent.relation.schema, locals))
        }
    }
}

fn plan_sequential_read_operators(
    query: &Query,
    selections: &[(&[String], bool)],
    collection: Option<String>,
    count_variable: Option<String>,
    distincts: &[&[String]],
) -> Result<CompiledBgp, BgpPlanError> {
    let mut positive = Vec::new();
    let mut operators = Vec::new();
    collect_read_operator_sequence(query, &mut positive, &mut operators)?;
    if positive.is_empty() {
        return Err(match operators.first() {
            Some(ReadOperator::Optional(_)) => BgpPlanError::OptionalMissingOuter,
            Some(ReadOperator::Equals(_)) => BgpPlanError::EqualsMissingInput,
            Some(ReadOperator::When(_)) => BgpPlanError::WhenMissingOuter,
            _ => BgpPlanError::NotMissingOuter,
        });
    }
    let positive_query = if positive.len() == 1 {
        positive.pop().unwrap()
    } else {
        Query::And(crate::query::And { and: positive })
    };
    let left = plan_bgp(&positive_query)?;
    let mut relation = left.relation;
    let mut effective_collection = collection;
    let first_optional = matches!(operators.first(), Some(ReadOperator::Optional(_)));
    merge_collection(
        &mut effective_collection,
        left.collection.as_ref(),
        if first_optional {
            BgpPlanError::OptionalRoutingConflict
        } else {
            BgpPlanError::NotRoutingConflict
        },
    )?;

    for (operator_index, operator) in operators.iter().enumerate() {
        if let ReadOperator::When(when) = operator {
            let true_branch = relation.clone();
            let condition = plan_bgp(&when.query)?;
            reject_when_subplan(&condition)?;
            merge_collection(&mut effective_collection, condition.collection.as_ref(), BgpPlanError::WhenRoutingConflict)?;
            let condition_keys = when_key_pairs(&relation, &condition.relation)?;
            let schema = relation.schema.clone();
            let domains = relation.domains.clone();
            let matched_condition = inner_join_relation(relation, condition.relation, condition_keys);

            let consequent = plan_bgp(&when.consequent)?;
            reject_when_subplan(&consequent)?;
            merge_collection(&mut effective_collection, consequent.collection.as_ref(), BgpPlanError::WhenRoutingConflict)?;
            let consequent_keys = when_key_pairs(&matched_condition, &consequent.relation)?;
            let successful = inner_join_relation(
                matched_condition,
                consequent.relation,
                consequent_keys,
            );
            let locals = successful
                .schema
                .iter()
                .filter(|name| !schema.contains(name))
                .cloned()
                .collect::<Vec<_>>();
            if !locals.is_empty() {
                let hidden_by_result_wrapper = count_variable.is_some()
                    || (!selections.is_empty()
                        && selections.iter().all(|(variables, _)| {
                            variables.iter().all(|name| !locals.contains(name))
                        }));
                if !hidden_by_result_wrapper {
                    return Err(BgpPlanError::WhenBranchLocalsEscape { vars: locals });
                }
                for later in &operators[operator_index + 1..] {
                    if let Some(var) = later_operator_local(later, &locals)? {
                        return Err(BgpPlanError::WhenBranchLocalUsedLater { var });
                    }
                }
            }
            let successful = if locals.is_empty() {
                successful
            } else {
                let indices = schema
                    .iter()
                    .map(|name| {
                        successful
                            .schema
                            .iter()
                            .position(|candidate| candidate == name)
                            .expect("outer When column remains in the joined relation")
                    })
                    .collect();
                CompiledRelation {
                    schema: schema.clone(),
                    domains: domains.clone(),
                    statement: CompiledRelationStatement::BagProjection {
                        input: Box::new(successful),
                        indices,
                    },
                }
            };
            let identity = (0..schema.len()).collect::<Vec<_>>();
            let expanded = CompiledRelation {
                schema: schema.clone(), domains: domains.clone(),
                statement: CompiledRelationStatement::BagUnion {
                    branches: vec![successful, true_branch],
                    mappings: vec![identity.clone(), identity],
                },
            };
            relation = CompiledRelation {
                schema, domains,
                statement: CompiledRelationStatement::When { expanded: Box::new(expanded) },
            };
            continue;
        }
        if let ReadOperator::Equals(equals) = operator {
            let constraint = compile_equality_constraint(&relation, &equals)?;
            let schema = relation.schema.clone();
            let domains = relation.domains.clone();
            relation = CompiledRelation {
                schema,
                domains,
                statement: CompiledRelationStatement::Equality {
                    input: Box::new(relation),
                    constraint,
                },
            };
            continue;
        }
        let (right_query, optional) = match operator {
            ReadOperator::Optional(query) => (query, true),
            ReadOperator::Not(query) => (query, false),
            ReadOperator::Equals(_) => unreachable!(),
            ReadOperator::When(_) => unreachable!(),
        };
        let right = plan_bgp(right_query)?;
        reject_subplan_wrappers(&right, optional)?;
        merge_collection(
            &mut effective_collection,
            right.collection.as_ref(),
            if optional {
                BgpPlanError::OptionalRoutingConflict
            } else {
                BgpPlanError::NotRoutingConflict
            },
        )?;
        let key_pairs = shared_key_pairs(&relation, &right.relation, optional)?;
        if optional {
            let mut schema = relation.schema.clone();
            let mut domains = relation.domains.clone();
            domains.extend(
                right
                    .relation
                    .schema
                    .iter()
                    .zip(&right.relation.domains)
                    .filter(|(name, _)| !relation.schema.contains(name))
                    .map(|(_, domain)| *domain),
            );
            schema.extend(
                right
                    .relation
                    .schema
                    .iter()
                    .filter(|name| !relation.schema.contains(name))
                    .cloned(),
            );
            relation = CompiledRelation {
                schema,
                domains,
                statement: CompiledRelationStatement::LeftOuter {
                    left: Box::new(relation),
                    right: Box::new(right.relation),
                    key_pairs,
                },
            };
        } else {
            let schema = relation.schema.clone();
            let domains = relation.domains.clone();
            relation = CompiledRelation {
                schema,
                domains,
                statement: CompiledRelationStatement::LeftAnti {
                    left: Box::new(relation),
                    right: Box::new(right.relation),
                    key_pairs,
                },
            };
        }
    }
    let (relation, full_row_distinct) = compile_distinct_relation(
        relation,
        distincts,
        selections,
        count_variable.as_deref(),
    )?;
    let projection = compile_projection(selections, count_variable.as_deref(), &relation.schema)?;
    let compiled = CompiledBgp {
        relation,
        projection,
        collection: effective_collection,
        count_variable,
        full_row_distinct,
    };
    validate_compiled_structure(&compiled.relation)?;
    Ok(compiled)
}

fn validate_compiled_structure(relation: &CompiledRelation) -> Result<(), BgpPlanError> {
    fn visit(
        relation: &CompiledRelation,
        depth: usize,
        nodes: &mut usize,
    ) -> Result<(), BgpPlanError> {
        if depth > MAX_RELATION_PROOF_DEPTH || *nodes >= MAX_RELATION_PROOF_NODES {
            return Err(BgpPlanError::RecursiveStructureLimit);
        }
        *nodes += 1;
        match &relation.statement {
            CompiledRelationStatement::Bgp(_)
            | CompiledRelationStatement::LayerTripleCount
            // The transitive closure is a leaf composite: it carries a raw edge BGP plan, not a
            // CompiledRelation child, so there is nothing further to recurse into here.
            | CompiledRelationStatement::TransitiveClosure { .. } => Ok(()),
            CompiledRelationStatement::GeneratedU64Filter { input, .. }
            | CompiledRelationStatement::GeneratedI64Filter { input, .. } => {
                visit(input, depth + 1, nodes)
            }
            CompiledRelationStatement::BagUnion { branches, .. } => {
                for branch in branches {
                    visit(branch, depth + 1, nodes)?;
                }
                Ok(())
            }
            CompiledRelationStatement::BagProjection { input, .. } => {
                visit(input, depth + 1, nodes)
            }
            CompiledRelationStatement::Distinct { input, key_columns } => {
                if !(1..=2).contains(&key_columns.len()) {
                    return Err(BgpPlanError::DistinctUnsupportedKeyCount {
                        count: key_columns.len(),
                    });
                }
                visit(input, depth + 1, nodes)
            }
            CompiledRelationStatement::When { expanded } => visit(expanded, depth + 1, nodes),
            CompiledRelationStatement::LeftAnti { left, right, .. }
            | CompiledRelationStatement::InnerJoin { left, right, .. }
            | CompiledRelationStatement::CrossRootHashJoin { left, right, .. }
            | CompiledRelationStatement::LeftOuter { left, right, .. } => {
                visit(left, depth + 1, nodes)?;
                visit(right, depth + 1, nodes)
            }
            CompiledRelationStatement::Equality { input, .. }
            | CompiledRelationStatement::TupleDisequality { input, .. }
            | CompiledRelationStatement::ExactScalarOrder { input, .. }
            | CompiledRelationStatement::ExactMultiOrder { input, .. }
            | CompiledRelationStatement::ExactOrderedSlice { input, .. } => {
                visit(input, depth + 1, nodes)
            }
            CompiledRelationStatement::GroupExtrema { input, .. } => visit(input, depth + 1, nodes),
            CompiledRelationStatement::GroupSum { input, .. } => visit(input, depth + 1, nodes),
            CompiledRelationStatement::GroupCount { input, .. } => visit(input, depth + 1, nodes),
            CompiledRelationStatement::CollectionCount { input } => visit(input, depth + 1, nodes),
            CompiledRelationStatement::CollectionSum { input, .. } => visit(input, depth + 1, nodes),
            CompiledRelationStatement::CollectionExtrema { input, .. } => {
                visit(input, depth + 1, nodes)
            }
            CompiledRelationStatement::Eval { input, .. } => visit(input, depth + 1, nodes),
            CompiledRelationStatement::FloorEval { input, .. } => visit(input, depth + 1, nodes),
            CompiledRelationStatement::RationalArith { input, .. } => visit(input, depth + 1, nodes),
            CompiledRelationStatement::Typecast { input, .. } => visit(input, depth + 1, nodes),
            CompiledRelationStatement::Concat { input, .. } => visit(input, depth + 1, nodes),
            CompiledRelationStatement::Join { input, .. } => visit(input, depth + 1, nodes),
            CompiledRelationStatement::Substring { input, .. } => visit(input, depth + 1, nodes),
            CompiledRelationStatement::Pad { input, .. } => visit(input, depth + 1, nodes),
            CompiledRelationStatement::HashKey { input, .. } => visit(input, depth + 1, nodes),
            CompiledRelationStatement::CalendarInt { input, .. } => visit(input, depth + 1, nodes),
            CompiledRelationStatement::CalendarDate { input, .. } => visit(input, depth + 1, nodes),
            CompiledRelationStatement::TypecastNumberToString { input, .. } => {
                visit(input, depth + 1, nodes)
            }
            CompiledRelationStatement::TypecastStringToNumber { input, .. } => {
                visit(input, depth + 1, nodes)
            }
        }
    }
    let mut nodes = 0;
    visit(relation, 1, &mut nodes)
}

/// SCHEMA-GRAPH FOUNDATION — the SINGLE committed graph a compiled query reads (see [`ProofGraph`]).
///
/// Every triple-reading leaf reads one graph: a `Bgp`/`LayerTripleCount` reads the INSTANCE graph;
/// a `TransitiveClosure` reads its own `graph` selector (Instance for a WOQL path, Schema for
/// Subsumption). Wrappers (Distinct/Select/joins/aggregations/…) inherit their children's. This pass
/// only ever emits SINGLE-GRAPH queries — instance-only, or the schema-only Subsumption closure — so
/// the whole proof anchors to ONE `LayerCommitment` + `commitment_root`, and the audited recursion is
/// handed exactly that layer (it stays graph-blind).
///
/// A query that MIXES an instance leaf with a schema leaf (the deferred cross-layer `IsA` regime — a
/// schema subsumption set joined against instance `rdf:type` triples) is REJECTED CLOSED here: this
/// pass does not thread two live views through a single BGP. An empty/ground relation (no reading
/// leaf) defaults to `Instance`, byte-identical to today.
fn relation_graph(relation: &CompiledRelation) -> std::io::Result<ProofGraph> {
    fn combine(acc: &mut Option<ProofGraph>, g: ProofGraph) -> std::io::Result<()> {
        match *acc {
            None => {
                *acc = Some(g);
                Ok(())
            }
            Some(prev) if prev == g => Ok(()),
            Some(_) => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "cross-layer query mixes instance and schema graph leaves (deferred); reject closed",
            )),
        }
    }
    fn walk(relation: &CompiledRelation, acc: &mut Option<ProofGraph>) -> std::io::Result<()> {
        match &relation.statement {
            // Instance-graph reading leaves.
            CompiledRelationStatement::Bgp(_) | CompiledRelationStatement::LayerTripleCount => {
                combine(acc, ProofGraph::Instance)
            }
            // The closure is the ONE graph-parameterized leaf.
            CompiledRelationStatement::TransitiveClosure { graph, .. } => combine(acc, *graph),
            // Wrappers: inherit their children's graph (mirrors `validate_compiled_structure::visit`).
            CompiledRelationStatement::GeneratedU64Filter { input, .. }
            | CompiledRelationStatement::GeneratedI64Filter { input, .. }
            | CompiledRelationStatement::BagProjection { input, .. }
            | CompiledRelationStatement::Distinct { input, .. }
            | CompiledRelationStatement::When { expanded: input }
            | CompiledRelationStatement::Equality { input, .. }
            | CompiledRelationStatement::TupleDisequality { input, .. }
            | CompiledRelationStatement::ExactScalarOrder { input, .. }
            | CompiledRelationStatement::ExactMultiOrder { input, .. }
            | CompiledRelationStatement::ExactOrderedSlice { input, .. }
            | CompiledRelationStatement::GroupExtrema { input, .. }
            | CompiledRelationStatement::GroupSum { input, .. }
            | CompiledRelationStatement::GroupCount { input, .. }
            | CompiledRelationStatement::CollectionCount { input }
            | CompiledRelationStatement::CollectionSum { input, .. }
            | CompiledRelationStatement::CollectionExtrema { input, .. }
            | CompiledRelationStatement::Eval { input, .. }
            | CompiledRelationStatement::FloorEval { input, .. }
            | CompiledRelationStatement::RationalArith { input, .. }
            | CompiledRelationStatement::Typecast { input, .. }
            | CompiledRelationStatement::Concat { input, .. }
            | CompiledRelationStatement::Join { input, .. }
            | CompiledRelationStatement::Substring { input, .. }
            | CompiledRelationStatement::Pad { input, .. }
            | CompiledRelationStatement::HashKey { input, .. }
            | CompiledRelationStatement::CalendarInt { input, .. }
            | CompiledRelationStatement::CalendarDate { input, .. }
            | CompiledRelationStatement::TypecastNumberToString { input, .. }
            | CompiledRelationStatement::TypecastStringToNumber { input, .. } => walk(input, acc),
            CompiledRelationStatement::BagUnion { branches, .. } => {
                for branch in branches {
                    walk(branch, acc)?;
                }
                Ok(())
            }
            CompiledRelationStatement::LeftAnti { left, right, .. }
            | CompiledRelationStatement::InnerJoin { left, right, .. }
            | CompiledRelationStatement::LeftOuter { left, right, .. } => {
                walk(left, acc)?;
                walk(right, acc)
            }
            // The cross-root hash join reads TWO graphs; it is TOP-LEVEL only (handled by the two-layer
            // prove_cross_root/verify_cross_root BEFORE this single-graph routing). Nested inside a
            // single-graph relation it cannot be served — reject closed.
            CompiledRelationStatement::CrossRootHashJoin { .. } => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "cross-root hash join must be the top-level relation (two-graph); reject closed",
            )),
        }
    }
    let mut acc = None;
    walk(relation, &mut acc)?;
    Ok(acc.unwrap_or(ProofGraph::Instance))
}

fn reject_subplan_wrappers(compiled: &CompiledBgp, optional: bool) -> Result<(), BgpPlanError> {
    if compiled.projection.is_some()
        || compiled.count_variable.is_some()
        || compiled.full_row_distinct
    {
        Err(if optional {
            BgpPlanError::OptionalUnsupportedSubplan
        } else {
            BgpPlanError::NotUnsupportedSubplan
        })
    } else {
        Ok(())
    }
}

fn reject_when_subplan(compiled: &CompiledBgp) -> Result<(), BgpPlanError> {
    if compiled.projection.is_some() || compiled.count_variable.is_some() || compiled.full_row_distinct {
        Err(BgpPlanError::WhenUnsupportedSubplan)
    } else {
        Ok(())
    }
}

fn when_key_pairs(
    outer: &CompiledRelation,
    subplan: &CompiledRelation,
) -> Result<Vec<(usize, usize)>, BgpPlanError> {
    let pairs = subplan
        .schema
        .iter()
        .enumerate()
        .filter_map(|(right, name)| {
            outer
                .schema
                .iter()
                .position(|candidate| candidate == name)
                .map(|left| (left, right))
        })
        .collect::<Vec<_>>();
    if !(1..=2).contains(&pairs.len()) {
        return Err(BgpPlanError::WhenUnsupportedVariables);
    }
    let left_descriptors = relation_result_descriptors(outer)
        .map_err(|_| BgpPlanError::WhenUnsupportedSubplan)?;
    let right_descriptors = relation_result_descriptors(subplan)
        .map_err(|_| BgpPlanError::WhenUnsupportedSubplan)?;
    for (left, right) in &pairs {
        if left_descriptors[*left].namespace != right_descriptors[*right].namespace
            || left_descriptors[*left].nullable || right_descriptors[*right].nullable
        {
            return Err(BgpPlanError::WhenUnsupportedSubplan);
        }
    }
    Ok(pairs)
}

/// A Store dictionary ID namespace whose field values are guaranteed `< 2^64`. Composite
/// two-key joins pack the two keys as `col0 + 2^64·col1`, which is injective only for such
/// bounded keys; a generated-decimal namespace can reach ~2^254 and must never be packed.
fn is_store_id_namespace(namespace: ResultIdNamespace) -> bool {
    matches!(
        namespace,
        ResultIdNamespace::NodeValue | ResultIdNamespace::Predicate
    )
}

fn shared_key_pairs(
    left: &CompiledRelation,
    right: &CompiledRelation,
    optional: bool,
) -> Result<Vec<(usize, usize)>, BgpPlanError> {
    let shared: Vec<_> = left
        .schema
        .iter()
        .enumerate()
        .filter_map(|(left_index, name)| {
            right
                .schema
                .iter()
                .position(|candidate| candidate == name)
                .map(|right_index| (name, left_index, right_index))
        })
        .collect();
    if shared.len() > 2 {
        return Err(if optional {
            BgpPlanError::OptionalSharedVariableCount {
                count: shared.len(),
            }
        } else {
            BgpPlanError::NotSharedVariableCount {
                count: shared.len(),
            }
        });
    }
    let left_descriptors = relation_result_descriptors(left).map_err(|_| {
        if optional {
            BgpPlanError::OptionalUnsupportedSubplan
        } else {
            BgpPlanError::NotUnsupportedSubplan
        }
    })?;
    let right_descriptors = relation_result_descriptors(right).map_err(|_| {
        if optional {
            BgpPlanError::OptionalUnsupportedSubplan
        } else {
            BgpPlanError::NotUnsupportedSubplan
        }
    })?;
    for (name, left_index, right_index) in &shared {
        if left_descriptors[*left_index].namespace != right_descriptors[*right_index].namespace {
            return Err(BgpPlanError::VariableNamespaceConflict {
                var: (*name).clone(),
            });
        }
        // Independent authenticated child relations model equality correlation. A
        // nullable outer binding is Prolog-unbound and would instead become bindable,
        // so this composition is deliberately rejected closed.
        if left_descriptors[*left_index].nullable || right_descriptors[*right_index].nullable {
            return Err(if optional {
                BgpPlanError::OptionalUnsupportedSubplan
            } else {
                BgpPlanError::NotUnsupportedSubplan
            });
        }
    }
    // A composite (two-key) correlation packs the two keys as `col0 + 2^64·col1`, injective
    // only when both keys are Store dictionary IDs (`< 2^64`). Namespace equality alone does
    // not bound the field width, so require every packed key to be a Store-ID namespace;
    // scalar (one-key) joins use injective equality and are deliberately left unrestricted.
    if shared.len() == 2 {
        for (name, left_index, right_index) in &shared {
            if !is_store_id_namespace(left_descriptors[*left_index].namespace)
                || !is_store_id_namespace(right_descriptors[*right_index].namespace)
            {
                return Err(BgpPlanError::CompositeJoinKeyNotStoreId {
                    var: (*name).clone(),
                });
            }
        }
    }
    Ok(shared
        .into_iter()
        .map(|(_, left, right)| (left, right))
        .collect())
}

fn schema_mapping(expected: &[String], actual: &[String]) -> Option<Vec<usize>> {
    if expected.len() != actual.len() {
        return None;
    }
    expected
        .iter()
        .map(|name| actual.iter().position(|candidate| candidate == name))
        .collect()
}

fn merge_collection(
    current: &mut Option<String>,
    candidate: Option<&String>,
    error: BgpPlanError,
) -> Result<(), BgpPlanError> {
    match (&current, candidate) {
        (Some(expected), Some(actual)) if expected != actual => Err(error),
        (None, Some(actual)) => {
            *current = Some(actual.clone());
            Ok(())
        }
        _ => Ok(()),
    }
}

/// Whether a compiled relation's rows are provably free of duplicate full rows by
/// construction, so a full-schema `Distinct` over it is a sound transparent no-op. Only
/// pure natural-join relations qualify: a leaf `Bgp` assigns set-valued (unique) triples,
/// and the natural join of two duplicate-free relations stays unique because a repeated
/// joined row would require a repeated row in one side. Row-eliminating filters, orders and
/// slices preserve that uniqueness, and `Distinct`/aggregation statements themselves emit at
/// most one row per key/group. Every bag-valued composition (`Or`/`BagUnion`, `Not`/
/// `LeftAnti`, `Optional`/`LeftOuter`, `BagProjection`, `When`) can carry duplicate full
/// rows and is rejected closed. The match is exhaustive on purpose: a new statement variant
/// must consciously classify itself here rather than defaulting to a silent no-op.
fn is_provably_duplicate_free(relation: &CompiledRelation) -> bool {
    match &relation.statement {
        // Set-valued triple assignments are unique.
        CompiledRelationStatement::Bgp(_) => true,
        // A single generated count row is trivially unique.
        CompiledRelationStatement::LayerTripleCount => true,
        // Natural join of two duplicate-free relations is duplicate-free.
        CompiledRelationStatement::InnerJoin { left, right, .. } => {
            is_provably_duplicate_free(left) && is_provably_duplicate_free(right)
        }
        // The cross-root SEMI-JOIN keeps a SUBSET of the instance `rdf:type` rows. Whether the kept
        // `?x` column is duplicate-free depends on whether an instance can carry two DISTINCT subsumed
        // types (two kept rows sharing `?x`); conservatively NOT provably unique (a `Distinct` is not
        // proven redundant over it).
        CompiledRelationStatement::CrossRootHashJoin { .. } => false,
        // Row-eliminating filters/orders/slices never introduce duplicates. Tuple
        // disequality only DROPS rows (edge-reuse over-counts), so it inherits uniqueness.
        CompiledRelationStatement::GeneratedU64Filter { input, .. }
        | CompiledRelationStatement::GeneratedI64Filter { input, .. }
        | CompiledRelationStatement::Equality { input, .. }
        | CompiledRelationStatement::TupleDisequality { input, .. }
        | CompiledRelationStatement::ExactScalarOrder { input, .. }
        | CompiledRelationStatement::ExactMultiOrder { input, .. }
        | CompiledRelationStatement::ExactOrderedSlice { input, .. } => {
            is_provably_duplicate_free(input)
        }
        // The transitive closure delivers the deduped reachable SET, so it is unique by construction.
        CompiledRelationStatement::TransitiveClosure { .. } => true,
        // Distinct and the aggregations emit at most one row per key/group, hence unique.
        CompiledRelationStatement::Distinct { .. }
        | CompiledRelationStatement::GroupExtrema { .. }
        | CompiledRelationStatement::GroupSum { .. }
        | CompiledRelationStatement::GroupCount { .. }
        | CompiledRelationStatement::CollectionCount { .. }
        | CompiledRelationStatement::CollectionSum { .. }
        | CompiledRelationStatement::CollectionExtrema { .. } => true,
        // Appending a per-row derived column is an injective row map: it can only make rows
        // MORE distinguishable, never merge two distinct child rows, so uniqueness is
        // inherited from the child. (Typecast appends a deterministic function of an existing
        // child column, so the same argument holds.)
        CompiledRelationStatement::Eval { input, .. }
        | CompiledRelationStatement::FloorEval { input, .. }
        | CompiledRelationStatement::Typecast { input, .. } => is_provably_duplicate_free(input),
        // Concat/Join/Substring carry the child's committed columns through VERBATIM (their
        // synthesized result is a terminal, non-committed value), so the committed rows'
        // uniqueness is exactly the child's. Inherit it.
        CompiledRelationStatement::Concat { input, .. }
        | CompiledRelationStatement::Join { input, .. }
        | CompiledRelationStatement::Substring { input, .. }
        // Pad/HashKey likewise carry the child's committed columns through VERBATIM (their
        // synthesized result is a terminal, non-committed value), so committed-row uniqueness is
        // exactly the child's. Inherit it.
        | CompiledRelationStatement::Pad { input, .. }
        | CompiledRelationStatement::HashKey { input, .. }
        // RationalArith likewise carries the child's committed columns through VERBATIM (the
        // xsd:decimal result is terminal/non-committed), so uniqueness is exactly the child's.
        | CompiledRelationStatement::RationalArith { input, .. }
        // CalendarDate carries the child's committed columns through VERBATIM (its xsd:date result is
        // terminal/non-committed), so committed-row uniqueness is exactly the child's. Inherit it.
        | CompiledRelationStatement::CalendarDate { input, .. }
        // Number↔string Typecasts likewise carry the child's committed columns through VERBATIM (the
        // cast result is a terminal, non-committed value), so uniqueness is exactly the child's.
        | CompiledRelationStatement::TypecastNumberToString { input, .. }
        | CompiledRelationStatement::TypecastStringToNumber { input, .. } => is_provably_duplicate_free(input),
        // CalendarInt appends per-row derived integer column(s) — an injective row map (like Eval),
        // so uniqueness is inherited from the child.
        CompiledRelationStatement::CalendarInt { input, .. } => is_provably_duplicate_free(input),
        // Bag-valued compositions can carry duplicate full rows.
        CompiledRelationStatement::BagUnion { .. }
        | CompiledRelationStatement::BagProjection { .. }
        | CompiledRelationStatement::When { .. }
        | CompiledRelationStatement::LeftAnti { .. }
        | CompiledRelationStatement::LeftOuter { .. } => false,
    }
}

fn compile_distinct_relation(
    relation: CompiledRelation,
    distincts: &[&[String]],
    selections: &[(&[String], bool)],
    count_variable: Option<&str>,
) -> Result<(CompiledRelation, bool), BgpPlanError> {
    if distincts.is_empty() {
        return Ok((relation, false));
    }
    if distincts.len() != 1 {
        return Err(BgpPlanError::NestedDistinctUnsupported);
    }
    let variables = distincts[0];
    if count_variable.is_none() {
        if !selections.is_empty() {
            return Err(BgpPlanError::DistinctWithProjectionOrCount);
        }
        let covers_schema = variables.len() == relation.schema.len()
            && relation.schema.iter().all(|name| {
                variables
                    .iter()
                    .filter(|candidate| *candidate == name)
                    .count()
                    == 1
            });
        if !covers_schema {
            return Err(BgpPlanError::DistinctNotFullSchema);
        }
        // The transparent full-row no-op relies on the child's rows already being unique.
        // Over a duplicate-producing child (Or/Not/Optional/projection/When) it would
        // silently return a NON-deduplicated result marked as `Distinct`, so reject closed
        // and defer real bag dedup to a future gadget.
        if !is_provably_duplicate_free(&relation) {
            return Err(BgpPlanError::DistinctOverDuplicateProducingChild);
        }
        return Ok((relation, true));
    }
    // A Select nested inside Count/Distinct changes the generated witness stream and is
    // not merely a selection of the count binding. Keep it fail-closed. An outer Select
    // of the Count binding has `outside == true` and is validated by compile_projection.
    if selections.iter().any(|(_, outside)| !outside) {
        return Err(BgpPlanError::DistinctWithProjectionOrCount);
    }
    if !(1..=2).contains(&variables.len()) {
        return Err(BgpPlanError::DistinctUnsupportedKeyCount {
            count: variables.len(),
        });
    }
    let mut key_columns = Vec::with_capacity(variables.len());
    for variable in variables {
        let index = relation
            .schema
            .iter()
            .position(|candidate| candidate == variable)
            .ok_or_else(|| BgpPlanError::SelectedVariableNotInBgp {
                var: variable.clone(),
            })?;
        if key_columns.contains(&index)
            || relation.domains.get(index).copied().flatten().is_none()
        {
            return Err(BgpPlanError::DistinctUnsupportedDomain {
                var: variable.clone(),
            });
        }
        key_columns.push(index);
    }
    let descriptors = relation_result_descriptors(&relation)
        .map_err(|_| BgpPlanError::DistinctUnsupportedDomain {
            var: variables[0].clone(),
        })?;
    for (&index, variable) in key_columns.iter().zip(variables) {
        if descriptors[index].nullable {
            return Err(BgpPlanError::DistinctUnsupportedDomain {
                var: variable.clone(),
            });
        }
    }
    let schema = variables.to_vec();
    let domains = key_columns
        .iter()
        .map(|index| relation.domains[*index])
        .collect();
    Ok((
        CompiledRelation {
            schema,
            domains,
            statement: CompiledRelationStatement::Distinct {
                input: Box::new(relation),
                key_columns,
            },
        },
        false,
    ))
}

fn compile_projection(
    selections: &[(&[String], bool)],
    count_variable: Option<&str>,
    schema: &[String],
) -> Result<Option<Vec<usize>>, BgpPlanError> {
    if let Some(count) = count_variable {
        let outside: Vec<&[String]> = selections
            .iter()
            .filter_map(|(vars, outside)| outside.then_some(*vars))
            .collect();
        let inside: Vec<&[String]> = selections
            .iter()
            .filter_map(|(vars, outside)| (!outside).then_some(*vars))
            .collect();
        validate_selection_chain(&inside, schema)?;
        validate_selection_chain(&outside, &[count.to_owned()])?;
        Ok(Some(Vec::new()))
    } else if selections.is_empty() {
        // Ground relations use one authenticated internal UNIT column that remains
        // hidden from the public WOQL schema, including a bag union of ground branches.
        Ok(schema.is_empty().then(Vec::new))
    } else {
        let selections: Vec<&[String]> = selections.iter().map(|(vars, _)| *vars).collect();
        validate_selection_chain(&selections, schema)?;
        Ok(Some(
            selections[0]
                .iter()
                .map(|var| {
                    schema
                        .iter()
                        .position(|candidate| candidate == var)
                        .expect("validated outer projection must occur in the BGP schema")
                })
                .collect(),
        ))
    }
}

/// Validate nested Selects listed outer-to-inner. Each Select may only name variables
/// exposed by its directly nested query; the innermost Select reads from `base`.
fn validate_selection_chain(selections: &[&[String]], base: &[String]) -> Result<(), BgpPlanError> {
    let mut available = base;
    for selected in selections.iter().rev() {
        for var in *selected {
            if !available.iter().any(|candidate| candidate == var) {
                return Err(BgpPlanError::SelectedVariableNotInBgp { var: var.clone() });
            }
        }
        available = selected;
    }
    Ok(())
}

/// Flatten a query body recursively, partitioning leaves into canonical triple patterns
/// and comparisons. Any other construct rejects closed.
fn collect_conjunct<'a>(
    query: &'a Query,
    triples: &mut Vec<TypedTriple>,
    comparisons: &mut Vec<NormalizedRange>,
) -> Result<(), BgpPlanError> {
    match query {
        Query::Triple(triple) => triples.push(TypedTriple {
            triple: triple.clone(),
            object_kind: ObjectKindFilter::Any,
            source: ColumnSource::Materialized,
        }),
        // CP-D delta reads: `AddedTriple`/`DeletedTriple` have the SAME shape as `Triple`
        // (subject: NodeValue, predicate: NodeValue, object: Value, graph). The reference
        // compiles them to `triple_addition`/`triple_removal` over EXACTLY this commit's
        // layer delta file ("no deeper"), so we reuse the identical atom lowering pointed at
        // the layer's additions/removals column source. The graph field is validated (instance
        // only) by `atom_from_triple`, exactly as for `Triple`.
        Query::AddedTriple(added) => triples.push(TypedTriple {
            triple: Triple {
                subject: added.subject.clone(),
                predicate: added.predicate.clone(),
                object: added.object.clone(),
                graph: added.graph.clone(),
            },
            object_kind: ObjectKindFilter::Any,
            source: ColumnSource::Additions,
        }),
        Query::DeletedTriple(deleted) => triples.push(TypedTriple {
            triple: Triple {
                subject: deleted.subject.clone(),
                predicate: deleted.predicate.clone(),
                object: deleted.object.clone(),
                graph: deleted.graph.clone(),
            },
            object_kind: ObjectKindFilter::Any,
            source: ColumnSource::Removals,
        }),
        Query::Link(link) => triples.push(TypedTriple {
            triple: Triple {
                subject: link.subject.clone(),
                predicate: link.predicate.clone(),
                object: match &link.object {
                    NodeValue::Node(iri) => Value::Node(iri.clone()),
                    NodeValue::Variable(name) => Value::Variable(name.clone()),
                },
                graph: Some(link.graph.clone()),
            },
            object_kind: ObjectKindFilter::Node,
            source: ColumnSource::Materialized,
        }),
        Query::Data(data) => {
            let object = match &data.object {
                DataValue::Data(value) => Value::Data(value.clone()),
                DataValue::Variable(name) => Value::Variable(name.clone()),
                DataValue::List(_) => {
                    return Err(BgpPlanError::UnsupportedObjectValue { kind: "data list" })
                }
            };
            triples.push(TypedTriple {
                triple: Triple {
                    subject: data.subject.clone(),
                    predicate: data.predicate.clone(),
                    object,
                    graph: Some(data.graph.clone()),
                },
                object_kind: ObjectKindFilter::Literal,
                source: ColumnSource::Materialized,
            });
        }
        Query::And(inner) => {
            for query in &inner.and {
                collect_conjunct(query, triples, comparisons)?;
            }
        }
        Query::Greater(_) | Query::Less(_) | Query::Gte(_) | Query::Lte(_) => {
            comparisons.push(normalize_comparison(query)?)
        }
        Query::InRange(range) => comparisons.push(normalize_in_range(range)?),
        Query::Pin(pin) => collect_conjunct(pin.query.as_ref(), triples, comparisons)?,
        Query::Immediately(immediately) => {
            collect_conjunct(immediately.query.as_ref(), triples, comparisons)?
        }
        Query::From(from) if from.graph.eq_ignore_ascii_case("instance") => {
            collect_conjunct(from.query.as_ref(), triples, comparisons)?
        }
        Query::From(_) => return Err(BgpPlanError::SchemaGraphUnsupported),
        Query::True(_) | Query::Comment(_) => {}
        other => {
            return Err(BgpPlanError::ConjunctNotTriple {
                construct: query_variant_name(other),
            })
        }
    }
    Ok(())
}

/// Lift one `Triple` to an [`Atom`]: the predicate must be a constant IRI; the subject
/// and object are each either a query VARIABLE (a kept output column) or an aligned
/// CONSTANT (emitted as an [`AtomTerm`] the store authenticates against its node/value
/// dictionary); and the graph must be the instance graph.
///
/// Accepted constants:
/// * subject/object node IRIs → [`AtomTerm::NodeConstant`] (CP2);
/// * object typed literals of a round-trip-aligned xsd datatype → [`AtomTerm::ValueConstant`]
///   (CP3, via [`value_constant_term`]).
///
/// Rejected closed: cross-namespace reuse of one variable in predicate and S/O positions,
/// schema-graph triples, dictionary/list object values, and unaligned typed-literal datatypes. A
/// value-literal SUBJECT is unrepresentable — woql2's
/// subject is a [`NodeValue`] (node/variable only) — so the store's `validate_atom`
/// value-subject guard is unreachable through this compiler by construction.
fn atom_from_triple(typed: &TypedTriple) -> Result<Atom, BgpPlanError> {
    let t = &typed.triple;
    // Graph: None (defaults to instance) or explicit Instance only.
    match &t.graph {
        None | Some(GraphType::Instance) => {}
        Some(GraphType::Schema) => return Err(BgpPlanError::SchemaGraphUnsupported),
    }

    let (predicate_term, pred_var) = match &t.predicate {
        NodeValue::Node(iri) => (iri.clone().into_bytes(), None),
        NodeValue::Variable(name) => (Vec::new(), Some(name.clone())),
    };

    // Subject: a variable (kept column) or a node-IRI constant.
    let (subject_term, subj_var) = match &t.subject {
        NodeValue::Variable(name) => (AtomTerm::Variable, Some(name.clone())),
        NodeValue::Node(iri) => (AtomTerm::NodeConstant(iri.clone().into_bytes()), None),
    };

    // Object: a variable (kept column), a node-IRI constant, or an aligned typed literal.
    let (object_term, obj_var) = match &t.object {
        Value::Variable(name) => (AtomTerm::Variable, Some(name.clone())),
        Value::Node(iri) => (AtomTerm::NodeConstant(iri.clone().into_bytes()), None),
        Value::Data(xsd) => (value_constant_term(xsd)?, None),
        Value::Dictionary(_) => {
            return Err(BgpPlanError::UnsupportedObjectValue { kind: "dictionary" })
        }
        Value::List(_) => return Err(BgpPlanError::UnsupportedObjectValue { kind: "list" }),
    };

    let subject_object_equal = matches!((&subj_var, &obj_var), (Some(s), Some(o)) if s == o);

    // Kept columns are materialized in S,P,O order. Predicate IDs occupy an independent
    // Store namespace, so one WOQL variable may not alias P with S or O.
    let mut kept = Vec::with_capacity(3);
    if let Some(s) = &subj_var {
        kept.push(s.clone());
    }
    if let Some(p) = &pred_var {
        if kept.contains(p) {
            return Err(BgpPlanError::VariableNamespaceConflict { var: p.clone() });
        }
        kept.push(p.clone());
    }
    if let Some(o) = &obj_var {
        if pred_var.as_ref() == Some(o) {
            return Err(BgpPlanError::VariableNamespaceConflict { var: o.clone() });
        }
        if !kept.contains(o) {
            kept.push(o.clone());
        }
    }
    let object_kind = if matches!(object_term, AtomTerm::Variable) {
        typed.object_kind
    } else {
        // A node/data constant already pins its kind through the authenticated
        // dictionary entry; the explicit filter is needed only for variables.
        ObjectKindFilter::Any
    };
    Ok(Atom {
        layer_atom: LayerBgpAtom {
            subject: subject_term,
            predicate_term,
            predicate_variable: pred_var.is_some(),
            object: object_term,
            // CP-D Step 1 (migration): every atom starts with no object range. Step 2
            // fills this in for atoms whose object variable a comparison conjunct ranges.
            object_range: None,
            object_kind,
            subject_object_equal,
            // Thread THIS atom's committed column source (materialized head for
            // `Triple`/`Link`/`Data`, additions for `AddedTriple`, removals for
            // `DeletedTriple`). Making it per-atom is what admits a CROSS-SOURCE join; the
            // store prove/verify read each atom's columns/count from this field, and it is
            // bound by `plan_digest` + the Fiat–Shamir transcript.
            source: typed.source,
        },
        kept,
        subj_var,
        pred_var,
        obj_var,
    })
}

fn validate_variable_namespaces(atoms: &[Atom]) -> Result<(), BgpPlanError> {
    for atom in atoms {
        if let Some(predicate) = &atom.pred_var {
            if atoms.iter().any(|candidate| {
                candidate.subj_var.as_ref() == Some(predicate)
                    || candidate.obj_var.as_ref() == Some(predicate)
            }) {
                return Err(BgpPlanError::VariableNamespaceConflict {
                    var: predicate.clone(),
                });
            }
        }
    }
    Ok(())
}

/// Route each numeric comparison conjunct to an object-value RANGE filter on the atom
/// that binds the compared variable. For each comparison:
///
/// 1. [`normalize_comparison`] turns it into `(var, op, datatype, c)`, orienting the
///    operator so `c` is always on the right (`k > ?a` ⇒ `?a < k`), rejecting closed a
///    var-vs-var / const-vs-const comparison or an unsupported constant datatype.
/// 2. v1 constraints (all reject closed): `var` must be the object of EXACTLY ONE atom,
///    must NOT appear as any atom's subject, and each `var` may carry at most two bounds.
/// 3. The [`ObjectRangeSpec`] attaches to that atom's object (its object stays a kept
///    variable — the range is a filter, not a projection change).
fn attach_ranges(atoms: &mut [Atom], comparisons: &[NormalizedRange]) -> Result<(), BgpPlanError> {
    for comparison in comparisons {
        let var = comparison.var.clone();
        // The ranged variable may not occupy a subject position anywhere.
        if atoms
            .iter()
            .any(|a| a.subj_var.as_deref() == Some(var.as_str()))
        {
            return Err(BgpPlanError::RangeVariableIsSubject { var });
        }
        // It must be the object variable of exactly one atom.
        let mut object_of: Vec<usize> = Vec::new();
        for (i, a) in atoms.iter().enumerate() {
            if a.obj_var.as_deref() == Some(var.as_str()) {
                object_of.push(i);
            }
        }
        let idx = match object_of.as_slice() {
            [] => return Err(BgpPlanError::RangeVariableNotObject { var }),
            [i] => *i,
            _ => return Err(BgpPlanError::RangeVariableObjectOfMultiple { var }),
        };

        // The matched atom's object is a Variable by construction (obj_var is Some only
        // for a variable object), so a range never lands on a constant object here.
        debug_assert!(matches!(atoms[idx].layer_atom.object, AtomTerm::Variable));
        match atoms[idx].layer_atom.object_kind {
            ObjectKindFilter::Node => {
                return Err(BgpPlanError::RangeOnNodeObject { var });
            }
            ObjectKindFilter::Literal => {
                // The range proof authenticates equality to one exact literal datatype,
                // so it subsumes Data's broader literal-kind constraint.
                atoms[idx].layer_atom.object_kind = ObjectKindFilter::Any;
            }
            ObjectKindFilter::Any => {}
        }
        for bound in &comparison.bounds {
            match &mut atoms[idx].layer_atom.object_range {
                None => {
                    atoms[idx].layer_atom.object_range = Some(ObjectRangeSpec {
                        op: bound.op,
                        datatype: comparison.datatype,
                        c: bound.c,
                        b_bits: OBJECT_RANGE_B_BITS,
                        second: None,
                    });
                }
                Some(existing) => {
                    if existing.datatype != comparison.datatype {
                        return Err(BgpPlanError::RangeDatatypeMismatch { var });
                    }
                    if existing.second.is_some() {
                        return Err(BgpPlanError::DuplicateRangeOnVariable { var });
                    }
                    existing.second = Some(ObjectRangeBound {
                        op: bound.op,
                        c: bound.c,
                    });
                }
            }
        }
    }
    Ok(())
}

#[derive(Clone)]
struct NormalizedBound {
    op: RangeOp,
    c: ArkFr,
}

#[derive(Clone)]
struct NormalizedRange {
    var: String,
    datatype: ArkFr,
    canonical_datatype: CanonicalDatatype,
    bounds: Vec<NormalizedBound>,
}

/// Normalize a numeric comparison conjunct into `(var, op, datatype, c)`, where `op` is
/// oriented so the constant is on the RIGHT (`Greater(k, ?a)` ⇒ `?a < k`), and
/// `(datatype, c)` is the store-aligned encoding of the constant (see
/// [`encode_range_constant`]).
///
/// Rejects closed: both sides variables ([`BgpPlanError::RangeBothVariables`]), neither
/// side a variable ([`BgpPlanError::RangeBothConstants`]), or a constant that is not a
/// supported order-preserving numeric literal ([`BgpPlanError::UnsupportedRangeDatatype`]).
fn normalize_comparison(q: &Query) -> Result<NormalizedRange, BgpPlanError> {
    // `op_var_left` is the operator when the VARIABLE is the left operand; `op_var_right`
    // is its flip (variable on the right) — the same relation with the sides swapped.
    let (left, right, op_var_left, op_var_right) = match q {
        Query::Greater(g) => (&g.left, &g.right, RangeOp::Gt, RangeOp::Lt),
        Query::Less(l) => (&l.left, &l.right, RangeOp::Lt, RangeOp::Gt),
        Query::Gte(g) => (&g.left, &g.right, RangeOp::Ge, RangeOp::Le),
        Query::Lte(l) => (&l.left, &l.right, RangeOp::Le, RangeOp::Ge),
        // Only comparison variants are collected into `comparisons`.
        _ => unreachable!("normalize_comparison only sees Greater/Less/Gte/Lte"),
    };

    let (var, op, konst): (String, RangeOp, &DataValue) = match (left, right) {
        (DataValue::Variable(_), DataValue::Variable(_)) => {
            return Err(BgpPlanError::RangeBothVariables)
        }
        (DataValue::Variable(v), other) => (v.clone(), op_var_left, other),
        (other, DataValue::Variable(v)) => (v.clone(), op_var_right, other),
        // Neither side is a variable.
        _ => return Err(BgpPlanError::RangeBothConstants),
    };

    let xsd = match konst {
        DataValue::Data(k) => k,
        DataValue::List(_) => {
            return Err(BgpPlanError::UnsupportedRangeDatatype { datatype: "list" })
        }
        // Excluded above (the variable was the other operand).
        DataValue::Variable(_) => unreachable!(),
    };
    let (datatype, c, canonical_datatype) = encode_range_constant(xsd)?;
    Ok(NormalizedRange {
        var,
        datatype,
        canonical_datatype,
        bounds: vec![NormalizedBound { op, c }],
    })
}

fn normalize_in_range(range: &InRange) -> Result<NormalizedRange, BgpPlanError> {
    let var = match &range.value {
        DataValue::Variable(var) => var.clone(),
        _ => return Err(BgpPlanError::InRangeValueNotVariable),
    };
    let start = match &range.start {
        DataValue::Data(value) => value,
        _ => return Err(BgpPlanError::InRangeBoundNotConstant),
    };
    let end = match &range.end {
        DataValue::Data(value) => value,
        _ => return Err(BgpPlanError::InRangeBoundNotConstant),
    };
    let (start_datatype, start_c, start_canonical_datatype) = encode_range_constant(start)?;
    let (end_datatype, end_c, end_canonical_datatype) = encode_range_constant(end)?;
    if start_datatype != end_datatype || start_canonical_datatype != end_canonical_datatype {
        return Err(BgpPlanError::RangeDatatypeMismatch { var });
    }
    Ok(NormalizedRange {
        var,
        datatype: start_datatype,
        canonical_datatype: start_canonical_datatype,
        bounds: vec![
            NormalizedBound {
                op: RangeOp::Ge,
                c: start_c,
            },
            NormalizedBound {
                op: RangeOp::Lt,
                c: end_c,
            },
        ],
    })
}

/// Build the store's typed dict entry for a woql2 SCALAR literal, routing each supported
/// xsd datatype through its `terminus_store`/`tdb-succinct` typed-value constructor (the
/// [`TdbDataType::make_entry`] family) so the resulting canonicalization is byte-identical
/// to a stored value of the same literal (no hand-rolled byte layouts — the 7.5c alignment
/// rule). Returns the entry, or the xsd datatype name for a literal this compiler does not
/// support (the caller wraps that name into the right rejection error).
///
/// `xsd:string` is handled separately by the caller via
/// [`ValueTriple::new_string_value`]. Invalid scalar lexical forms reject closed.
fn xsd_scalar_entry(xsd: &XSDAnySimpleType) -> Result<TypedDictEntry, &'static str> {
    Ok(match xsd {
        XSDAnySimpleType::Boolean(b) => bool::make_entry(b),
        XSDAnySimpleType::UnsignedByte(v) => u8::make_entry(v),
        XSDAnySimpleType::Byte(v) => i8::make_entry(v),
        XSDAnySimpleType::UnsignedShort(v) => u16::make_entry(v),
        XSDAnySimpleType::Short(v) => i16::make_entry(v),
        // xsd:unsignedInt is the 32-bit unsigned type → store `UInt32`.
        XSDAnySimpleType::UnsignedInt(u) => {
            let v = u32::try_from(*u).map_err(|_| "unsignedInt (exceeds u32)")?;
            u32::make_entry(&v)
        }
        XSDAnySimpleType::Int(v) => i32::make_entry(v),
        XSDAnySimpleType::UnsignedLong(v) => u64::make_entry(v),
        XSDAnySimpleType::Long(v) => i64::make_entry(v),
        // The woql2 `Integer` variant carries an `i64` → store `Int64` (fixed-width,
        // order-preserving via offset-binary — so it also supports range).
        XSDAnySimpleType::Integer(i) => i64::make_entry(i),
        // The woql2 `Float` variant carries an `f64` → store `Float64` (8-byte IEEE
        // total-order OP encoding).
        XSDAnySimpleType::Float(f) => f64::make_entry(f),
        XSDAnySimpleType::Decimal(decimal) => {
            let value = tdb_succinct::Decimal::new(decimal.to_string()).map_err(|_| "decimal")?;
            tdb_succinct::Decimal::make_entry(&value)
        }
        XSDAnySimpleType::URI(uri) => AnyURI::make_entry(uri),
        // The WOQL AST carries a UTC DateTime; the Store's xsd:dateTime entry is its
        // timezone-free UTC representation.
        XSDAnySimpleType::DateTime(datetime) => {
            chrono::NaiveDateTime::make_entry(&datetime.naive_utc())
        }
        XSDAnySimpleType::Date(date) => Date::make_entry(&Date {
            year: i64::from(date.year()),
            month: date.month() as u8,
            day: date.day() as u8,
            offset: 0,
        }),
        XSDAnySimpleType::Time(time) => chrono::NaiveTime::make_entry(time),
        XSDAnySimpleType::HexBinary(value) => {
            let bytes = hex::decode(value).map_err(|_| "hexBinary (invalid lexical value)")?;
            HexBinary::make_entry(&HexBinary(bytes))
        }
        other => return Err(xsd_datatype_name(other)),
    })
}

/// Encode a numeric range constant to `(datatype_tag, c)` matching the store's committed
/// node/value dictionary encoding EXACTLY, by routing through the store's OWN value
/// canonicalization (7.5c alignment trick): build the typed value via [`xsd_scalar_entry`]
/// + [`ValueTriple::new_value`], read back its [`CanonicalObject::Value`] via
/// [`CanonicalObject::from_store_object`], take its [`CanonicalDatatype`] discriminant as the
/// datatype tag, and compute `c` as the big-endian unsigned integer of its order-preserving
/// `to_lexical` bytes — the SAME conversion `terminus_store`'s (private) `node_value_op_value`
/// applies to the committed dict value column.
///
/// Supported (round-trip-verified, all order-preserving fixed-width): `xsd:boolean`
/// (`Boolean`), all explicit fixed-width XSD integer types (`UInt8`/`Int8` through
/// `UInt64`/`Int64`), `xsd:integer` (`Int64`), and `xsd:float`/`double` (`Float64`).
/// Every other `XSDAnySimpleType` rejects closed: a range over a
/// non-order-preserving or variable-width datatype (string/decimal/…) would compare a
/// `0` sentinel in the store, which is unsound at the query level. Adding a datatype
/// requires a store-routed construction in [`xsd_scalar_entry`], an
/// [`op_preserving_width`] entry, PLUS a round-trip alignment test.
fn encode_range_constant(
    xsd: &XSDAnySimpleType,
) -> Result<(ArkFr, ArkFr, CanonicalDatatype), BgpPlanError> {
    match xsd {
        // Order-preserving fixed-width numerics: the store's `to_lexical` bytes ARE the OP
        // encoding, so `encode_via_store` -> `node_value_op_value` big-endian reads them.
        XSDAnySimpleType::Boolean(_)
        | XSDAnySimpleType::UnsignedByte(_)
        | XSDAnySimpleType::Byte(_)
        | XSDAnySimpleType::UnsignedShort(_)
        | XSDAnySimpleType::Short(_)
        | XSDAnySimpleType::UnsignedInt(_)
        | XSDAnySimpleType::Int(_)
        | XSDAnySimpleType::UnsignedLong(_)
        | XSDAnySimpleType::Long(_)
        | XSDAnySimpleType::Integer(_)
        | XSDAnySimpleType::Float(_) => {}
        // date/dateTime: order-preserving via the store's DECODED i64-nanoseconds key
        // (`node_value_op_value` decodes the native lexical). REJECT-CLOSE a constant at or
        // outside the representable i64-nanoseconds window here, BEFORE building it, so the
        // saturation of far DATA dates stays strictly order-correct against it (see
        // `date_constant_nanos`). The encoding itself still comes from the store routing
        // below (single source of truth) — this only gates the value.
        XSDAnySimpleType::DateTime(_) | XSDAnySimpleType::Date(_) => {
            let _ = date_constant_nanos(xsd)?;
        }
        // Everything else (string/decimal/hexBinary/anyURI/time/…) rejects closed.
        _ => {
            return Err(BgpPlanError::UnsupportedRangeDatatype {
                datatype: xsd_datatype_name(xsd),
            });
        }
    }
    let entry = xsd_scalar_entry(xsd)
        .map_err(|datatype| BgpPlanError::UnsupportedRangeDatatype { datatype })?;
    encode_via_store(entry)
}

/// The i64 nanoseconds-since-epoch key of a `date`/`dateTime` range CONSTANT, or
/// [`BgpPlanError::DateRangeConstantOutOfExactRange`] if it is not STRICTLY inside the
/// representable i64-nanoseconds window (endpoints `i64::MIN`/`i64::MAX` excluded — those
/// are exactly the saturated values a far DATA date takes, so excluding them keeps `c`
/// strictly between every far-past and far-future data date). Used only as a pre-flight
/// reject; the emitted `c` is still the store-routed value in [`encode_via_store`].
///
/// The constant carries no timezone offset (a WOQL `Date` is a bare `NaiveDate`; a
/// `DateTime` a UTC instant), matching the `offset: 0` form [`xsd_scalar_entry`] builds —
/// so the constant's key is a pure midnight (date) / instant (dateTime), and the `date`
/// offset tiebreaker in `node_value_op_value` is only ever exercised by DATA rows.
fn date_constant_nanos(xsd: &XSDAnySimpleType) -> Result<i64, BgpPlanError> {
    let nanos = match xsd {
        XSDAnySimpleType::DateTime(datetime) => datetime.timestamp_nanos_opt(),
        XSDAnySimpleType::Date(date) => date
            .and_hms_opt(0, 0, 0)
            .and_then(|ndt| ndt.and_utc().timestamp_nanos_opt()),
        // Only date/dateTime reach here.
        _ => return Err(BgpPlanError::DateRangeConstantOutOfExactRange),
    };
    match nanos {
        Some(n) if n != i64::MIN && n != i64::MAX => Ok(n),
        _ => Err(BgpPlanError::DateRangeConstantOutOfExactRange),
    }
}

/// Canonicalize a `TypedDictEntry` through the store's own value path and derive the
/// `(datatype_tag, c)` pair. Fails closed if the resulting datatype is not one of the
/// round-trip-verified order-preserving datatypes (fixed-width numerics, plus
/// `date`/`dateTime`; see [`op_preserving_width`]).
fn encode_via_store(
    entry: TypedDictEntry,
) -> Result<(ArkFr, ArkFr, CanonicalDatatype), BgpPlanError> {
    // Subject/predicate are irrelevant to the object canonicalization.
    let triple = ValueTriple::new_value("", "", entry);
    let object = CanonicalObject::from_store_object(&triple.object);
    let datatype = match &object {
        CanonicalObject::Value { datatype, .. } => *datatype,
        // `new_value` always yields a Value object; fail closed rather than emit an
        // unaligned constant.
        CanonicalObject::Node(_) => {
            return Err(BgpPlanError::UnsupportedRangeDatatype { datatype: "node" })
        }
    };
    // Guard: only order-preserving datatypes are sound to range over (the fixed-width
    // numerics, plus date/dateTime).
    if op_preserving_width(datatype).is_none() {
        return Err(BgpPlanError::UnsupportedRangeDatatype {
            datatype: "non-order-preserving datatype",
        });
    }
    // c = the store's OWN order-preserving value for this object, computed by the EXACT
    // function applied to the committed dict VALUE column (`node_value_op_value`). This is
    // the single source of truth: for fixed-width numerics it big-endian reads the lexical;
    // for date/dateTime it decodes the native lexical to the i64-nanoseconds key and
    // re-encodes as offset-binary. So `c` is byte-identical to a stored value's committed OP
    // entry BY CONSTRUCTION — no re-implemented byte layout here (the previous fixed-width
    // BE read now lives inside `node_value_op_value`). A corrupt/short numeric lexical, or a
    // date whose committed lexical is malformed, fails closed inside that function.
    let c = node_value_op_value(&object).map_err(|_| BgpPlanError::UnsupportedRangeDatatype {
        datatype: "value is not order-preserving",
    })?;
    let datatype_tag = ArkFr::from_u64(datatype as u64);
    Ok((datatype_tag, c, datatype))
}

/// Fixed byte width of a SUPPORTED order-preserving numeric datatype, or `None` for any
/// datatype whose lexical is variable-width or not order-preserving. Mirrors
/// `terminus_store`'s private `order_preserving_numeric_width` EXACTLY: for each of these
/// the store's `to_lexical` writes exactly `width` bytes whose big-endian unsigned read is
/// monotone in value (`Boolean` 0/1; `(U)Int8/16/32/64` raw-BE / offset-binary;
/// `Float32/64` IEEE total-order), so `node_value_op_value` and this compiler compute the
/// SAME `c`. A range over any other datatype rejects closed (see [`encode_range_constant`]).
fn op_preserving_width(datatype: CanonicalDatatype) -> Option<usize> {
    use CanonicalDatatype::*;
    Some(match datatype {
        Boolean => 1,
        UInt8 | Int8 => 1,
        UInt16 | Int16 => 2,
        UInt32 | Int32 | Float32 => 4,
        UInt64 | Int64 | Float64 => 8,
        // `Date`/`DateTime` are order-preserving at the `Int64` width (8), but — unlike
        // every entry above — the OP value is NOT a big-endian read of the lexical: it is
        // the store's DECODED i64-nanoseconds key (`node_value_op_value`), routed through
        // that exact function in `encode_via_store`. This mirror table only gates that a
        // range over the datatype is allowed and reports the gadget width; it does NOT
        // imply an 8-byte lexical.
        Date | DateTime => 8,
        _ => return None,
    })
}

/// The xsd datatype name of a woql2 typed literal, for descriptive rejection messages.
fn xsd_datatype_name(xsd: &XSDAnySimpleType) -> &'static str {
    match xsd {
        XSDAnySimpleType::String(_) => "string",
        XSDAnySimpleType::Decimal(_) => "decimal",
        XSDAnySimpleType::Float(_) => "float",
        XSDAnySimpleType::Boolean(_) => "boolean",
        XSDAnySimpleType::HexBinary(_) => "hexBinary",
        XSDAnySimpleType::URI(_) => "anyURI",
        XSDAnySimpleType::DateTime(_) => "dateTime",
        XSDAnySimpleType::Date(_) => "date",
        XSDAnySimpleType::Time(_) => "time",
        XSDAnySimpleType::UnsignedByte(_) => "unsignedByte",
        XSDAnySimpleType::Byte(_) => "byte",
        XSDAnySimpleType::UnsignedShort(_) => "unsignedShort",
        XSDAnySimpleType::Short(_) => "short",
        XSDAnySimpleType::UnsignedInt(_) => "unsignedInt",
        XSDAnySimpleType::Int(_) => "int",
        XSDAnySimpleType::UnsignedLong(_) => "unsignedLong",
        XSDAnySimpleType::Long(_) => "long",
        XSDAnySimpleType::Integer(_) => "integer",
    }
}

/// Emit an [`AtomTerm::ValueConstant`] for an object typed literal — but ONLY for xsd
/// datatypes whose `(CanonicalDatatype, lexical bytes)` this compiler can produce so they
/// canonicalize IDENTICALLY to the store's own encoding of the same literal. Any other
/// datatype rejects closed rather than authenticate a constant whose alignment is unproven
/// (a misaligned `(datatype, lexical)` would `hash_object` to the wrong dictionary row, so
/// membership would fail — or, worse, authenticate the wrong constant).
///
/// Equality does NOT require order-preservation (it is id-based via dictionary membership),
/// so ANY datatype the store can canonicalize is sound here. Supported (round-trip-verified,
/// each routed through the store's OWN value construction so the bytes match BY CONSTRUCTION,
/// no re-implementation of the tdb-succinct encoding):
/// * `xsd:string` via [`ValueTriple::new_string_value`] (the exact path the store uses to
///   store a string value);
/// * every other supported WOQL scalar via [`xsd_scalar_entry`] →
///   [`ValueTriple::new_value`] using its matching `tdb-succinct` typed entry.
///
/// See the `object_*_literal_*` round-trip tests. Invalid scalar lexical forms reject closed.
fn value_constant_term(xsd: &XSDAnySimpleType) -> Result<AtomTerm, BgpPlanError> {
    // Build the store object via the store's OWN construction path for each datatype. String
    // uses the dedicated string-value constructor; every other supported scalar goes through
    // its typed dict entry. Subject/predicate are irrelevant to the object canonicalization,
    // so empty placeholders are fine.
    let object = match xsd {
        XSDAnySimpleType::String(s) => ValueTriple::new_string_value("", "", s).object,
        _ => {
            let entry = xsd_scalar_entry(xsd)
                .map_err(|datatype| BgpPlanError::UnsupportedValueDatatype { datatype })?;
            ValueTriple::new_value("", "", entry).object
        }
    };
    match CanonicalObject::from_store_object(&object) {
        CanonicalObject::Value { datatype, lexical } => {
            Ok(AtomTerm::ValueConstant(datatype, lexical))
        }
        // Unreachable: a value construction always yields a `Value` object. Fail closed
        // rather than emit an unaligned constant.
        CanonicalObject::Node(_) => Err(BgpPlanError::UnsupportedValueDatatype {
            datatype: xsd_datatype_name(xsd),
        }),
    }
}

#[cfg(test)]
mod group_extrema_planner_tests {
    use super::*;
    use crate::compare::Gte;
    use crate::misc::{RangeMax, RangeMin};
    use crate::order::GroupBy;
    use crate::query::And;

    fn exact_min() -> Query {
        let child = Query::And(And {
            and: vec![
                Query::Triple(Triple {
                    subject: NodeValue::Variable("Person".into()),
                    predicate: NodeValue::Node("http://ex/age".into()),
                    object: Value::Variable("Age".into()),
                    graph: None,
                }),
                Query::Gte(Gte {
                    left: DataValue::Variable("Age".into()),
                    right: DataValue::Data(XSDAnySimpleType::UnsignedInt(0)),
                }),
            ],
        });
        Query::And(And {
            and: vec![
                Query::GroupBy(GroupBy {
                    template: Value::Variable("Age".into()),
                    group_by: vec!["Person".into()],
                    grouped_value: Value::Variable("Ages".into()),
                    query: Box::new(child),
                }),
                Query::RangeMin(RangeMin {
                    list: DataValue::Variable("Ages".into()),
                    result: DataValue::Variable("Youngest".into()),
                }),
            ],
        })
    }

    fn two_key_max() -> Query {
        let child = Query::And(And {
            and: vec![
                Query::Triple(Triple {
                    subject: NodeValue::Variable("Left".into()),
                    predicate: NodeValue::Node("http://ex/link".into()),
                    object: Value::Variable("Right".into()),
                    graph: None,
                }),
                Query::Triple(Triple {
                    subject: NodeValue::Variable("Right".into()),
                    predicate: NodeValue::Node("http://ex/score".into()),
                    object: Value::Variable("Score".into()),
                    graph: None,
                }),
                Query::Gte(Gte {
                    left: DataValue::Variable("Score".into()),
                    right: DataValue::Data(XSDAnySimpleType::Long(i64::MIN)),
                }),
            ],
        });
        Query::And(And {
            and: vec![
                Query::GroupBy(GroupBy {
                    template: Value::Variable("Score".into()),
                    group_by: vec!["Right".into(), "Left".into()],
                    grouped_value: Value::Variable("Scores".into()),
                    query: Box::new(child),
                }),
                Query::RangeMax(RangeMax {
                    list: DataValue::Variable("Scores".into()),
                    result: DataValue::Variable("Best".into()),
                }),
            ],
        })
    }

    #[test]
    fn exact_scalar_group_min_is_compiled_without_exposing_hidden_list() {
        let compiled = plan_bgp(&exact_min()).unwrap();
        assert_eq!(compiled.relation.schema, ["Person", "Youngest"]);
        match compiled.relation.statement {
            CompiledRelationStatement::GroupExtrema {
                key_columns,
                value_column,
                datatype,
                kind,
                ..
            } => {
                assert_eq!(key_columns, [0]);
                assert_ne!(key_columns[0], value_column);
                assert_eq!(datatype, CanonicalDatatype::UInt32);
                assert_eq!(kind, GroupExtremaKind::Min);
            }
            _ => panic!("expected group-extrema statement"),
        }
    }

    #[test]
    fn composite_key_or_unconsumed_hidden_list_rejects() {
        let mut composite = exact_min();
        if let Query::And(and) = &mut composite {
            if let Query::GroupBy(group) = &mut and.and[0] {
                group.group_by.push("Age".into());
            }
        }
        assert!(matches!(
            plan_bgp(&composite),
            Err(BgpPlanError::GroupExtremaUnsupportedShape)
        ));
        assert!(matches!(
            plan_bgp(&Query::GroupBy(GroupBy {
                template: Value::Variable("Age".into()),
                group_by: vec!["Person".into()],
                grouped_value: Value::Variable("Ages".into()),
                query: Box::new(exact_min()),
            })),
            Err(BgpPlanError::UnsupportedConstruct { .. })
        ));
    }


    #[test]
    fn zero_and_two_node_key_extrema_preserve_declared_priority() {
        let mut zero = exact_min();
        if let Query::And(and) = &mut zero {
            if let Query::GroupBy(group) = &mut and.and[0] {
                group.group_by.clear();
            }
        }
        let compiled = plan_bgp(&zero).unwrap();
        assert_eq!(compiled.relation.schema, ["Youngest"]);
        assert!(matches!(
            compiled.relation.statement,
            CompiledRelationStatement::GroupExtrema { ref key_columns, .. }
                if key_columns.is_empty()
        ));

        let compiled = plan_bgp(&two_key_max()).unwrap();
        assert_eq!(compiled.relation.schema, ["Right", "Left", "Best"]);
        assert!(matches!(
            compiled.relation.statement,
            CompiledRelationStatement::GroupExtrema { ref key_columns, kind: GroupExtremaKind::Max, .. }
                if key_columns == &[1, 0]
        ));
    }

    #[test]
    fn reversed_float_and_numeric_group_keys_reject_closed() {
        let mut reversed = exact_min();
        if let Query::And(and) = &mut reversed {
            and.and.swap(0, 1);
        }
        assert!(matches!(
            plan_bgp(&reversed),
            Err(BgpPlanError::GroupExtremaUnsupportedShape)
        ));

        let mut floating = exact_min();
        if let Query::And(and) = &mut floating {
            if let Query::GroupBy(group) = &mut and.and[0] {
                if let Query::And(child) = group.query.as_mut() {
                    if let Query::Gte(bound) = &mut child.and[1] {
                        bound.right = DataValue::Data(XSDAnySimpleType::Float(0.0));
                    }
                }
            }
        }
        assert!(matches!(
            plan_bgp(&floating),
            Err(BgpPlanError::GroupExtremaUnsupportedValue)
        ));

        let mut numeric_key = exact_min();
        if let Query::And(and) = &mut numeric_key {
            if let Query::GroupBy(group) = &mut and.and[0] {
                group.group_by = vec!["Weight".into()];
                if let Query::And(child) = group.query.as_mut() {
                    child.and.push(Query::Triple(Triple {
                        subject: NodeValue::Variable("Person".into()),
                        predicate: NodeValue::Node("http://ex/weight".into()),
                        object: Value::Variable("Weight".into()),
                        graph: None,
                    }));
                    child.and.push(Query::Gte(Gte {
                        left: DataValue::Variable("Weight".into()),
                        right: DataValue::Data(XSDAnySimpleType::UnsignedInt(0)),
                    }));
                }
            }
        }
        assert!(matches!(
            plan_bgp(&numeric_key),
            Err(BgpPlanError::GroupExtremaUnsupportedShape)
        ));
    }
}

#[cfg(test)]
mod collection_extrema_planner_tests {
    use super::*;
    use crate::compare::Gte;
    use crate::misc::{Collect, RangeMax, RangeMin};
    use crate::query::And;

    fn exact_extrema(max: bool) -> Query {
        let child = Query::And(And {
            and: vec![
                Query::Triple(Triple {
                    subject: NodeValue::Variable("Person".into()),
                    predicate: NodeValue::Node("http://ex/age".into()),
                    object: Value::Variable("Age".into()),
                    graph: None,
                }),
                Query::Gte(Gte {
                    left: DataValue::Variable("Age".into()),
                    right: DataValue::Data(XSDAnySimpleType::Long(i64::MIN)),
                }),
            ],
        });
        let collect = Query::Collect(Collect {
            template: Value::Variable("Age".into()),
            into: Value::Variable("Ages".into()),
            query: Box::new(child),
        });
        let range = if max {
            Query::RangeMax(RangeMax {
                list: DataValue::Variable("Ages".into()),
                result: DataValue::Variable("Extreme".into()),
            })
        } else {
            Query::RangeMin(RangeMin {
                list: DataValue::Variable("Ages".into()),
                result: DataValue::Variable("Extreme".into()),
            })
        };
        Query::And(And {
            and: vec![collect, range],
        })
    }

    #[test]
    fn immediate_collect_extrema_compiles_to_one_exact_store_id() {
        for (max, expected_kind) in [
            (false, GroupExtremaKind::Min),
            (true, GroupExtremaKind::Max),
        ] {
            let compiled = plan_bgp(&exact_extrema(max)).unwrap();
            assert_eq!(compiled.relation.schema, ["Extreme"]);
            assert_eq!(
                compiled.relation.domains,
                [Some(EqualityValueDomain::Datatype(CanonicalDatatype::Int64))]
            );
            match compiled.relation.statement {
                CompiledRelationStatement::CollectionExtrema {
                    value_column,
                    datatype,
                    kind,
                    ..
                } => {
                    assert_eq!(value_column, 1);
                    assert_eq!(datatype, CanonicalDatatype::Int64);
                    assert_eq!(kind, expected_kind);
                }
                _ => panic!("expected collection-extrema statement"),
            }
        }
    }

    #[test]
    fn reversed_mismatched_and_unbound_collection_extrema_reject_closed() {
        let mut reversed = exact_extrema(false);
        if let Query::And(and) = &mut reversed {
            and.and.swap(0, 1);
        }
        assert!(matches!(
            plan_bgp(&reversed),
            Err(BgpPlanError::CollectionExtremaUnsupportedShape)
        ));

        let mut mismatched = exact_extrema(false);
        if let Query::And(and) = &mut mismatched {
            if let Query::RangeMin(range) = &mut and.and[1] {
                range.list = DataValue::Variable("Other".into());
            }
        }
        assert!(matches!(
            plan_bgp(&mismatched),
            Err(BgpPlanError::CollectionExtremaUnsupportedShape)
        ));

        let mut unbound = exact_extrema(false);
        if let Query::And(and) = &mut unbound {
            if let Query::Collect(collect) = &mut and.and[0] {
                collect.template = Value::Variable("Missing".into());
            }
        }
        assert!(matches!(
            plan_bgp(&unbound),
            Err(BgpPlanError::CollectionExtremaUnsupportedValue)
        ));
    }

    #[test]
    fn floating_collection_extrema_rejects_until_exact_semantics_are_pinned() {
        let mut query = exact_extrema(false);
        if let Query::And(and) = &mut query {
            if let Query::Collect(collect) = &mut and.and[0] {
                if let Query::And(child) = collect.query.as_mut() {
                    if let Query::Gte(bound) = &mut child.and[1] {
                        bound.right = DataValue::Data(XSDAnySimpleType::Float(0.0));
                    }
                }
            }
        }
        assert!(matches!(
            plan_bgp(&query),
            Err(BgpPlanError::CollectionExtremaUnsupportedValue)
        ));
    }
}

#[cfg(test)]
mod composite_join_key_namespace_tests {
    use super::*;

    /// One `LayerTripleCount` leaf: a single `GeneratedDecimalU64` column (not a Store ID).
    fn generated_u64_leaf() -> CompiledRelation {
        CompiledRelation {
            schema: vec!["_".into()],
            domains: vec![None],
            statement: CompiledRelationStatement::LayerTripleCount,
        }
    }

    /// A two-column relation whose columns `a`/`b` are both `GeneratedDecimalU64` — i.e. NOT
    /// bounded Store dictionary IDs, so they must never reach the composite key packing.
    fn two_generated_columns(a: &str, b: &str) -> CompiledRelation {
        CompiledRelation {
            schema: vec![a.into(), b.into()],
            domains: vec![None, None],
            statement: CompiledRelationStatement::InnerJoin {
                left: Box::new(generated_u64_leaf()),
                right: Box::new(generated_u64_leaf()),
                key_pairs: Vec::new(),
            },
        }
    }

    #[test]
    fn composite_join_rejects_non_store_id_key_namespace() {
        let left = two_generated_columns("a", "b");
        let right = two_generated_columns("a", "b");
        // Both a Not (LeftAnti) and an Optional (LeftOuter) route composite keys here.
        for optional in [false, true] {
            let err = shared_key_pairs(&left, &right, optional).unwrap_err();
            assert!(
                matches!(err, BgpPlanError::CompositeJoinKeyNotStoreId { .. }),
                "expected CompositeJoinKeyNotStoreId, got {err:?}"
            );
        }
    }

    #[test]
    fn scalar_join_over_generated_key_stays_unrestricted() {
        // A single shared key uses injective equality, never the 2^64 packing, so a
        // generated-decimal scalar key must NOT be rejected by the composite guard.
        let left = generated_u64_leaf();
        let right = generated_u64_leaf();
        let pairs = shared_key_pairs(&left, &right, false).unwrap();
        assert_eq!(pairs, [(0, 0)]);
    }
}

/// The safety-critical left-deep planner. Tracks the accumulator schema as an ordered
/// list of distinct variable names and emits atoms (in placement order) + `k-1` join
/// steps. Each step joins the accumulator with an unplaced atom on the SINGLE shared
/// variable; the accumulator schema evolves as `[r_other.., key, s_other..]`, matching
/// `terminus_store`'s `bgp` output ordering exactly. If no remaining atom shares a
/// variable, the next input-order component starts with a verifier-known UNIT-key
/// Cartesian step whose output ordering is `[acc.., relation..]`.
///
/// Each atom relation carries ONE column per VARIABLE, in S,P,O order (`atom.kept`); a
/// constant removes its position from `kept`. `rel_key` is the shared variable's index within
/// the atom's KEPT columns, and `s_other` is the atom's remaining kept columns (possibly
/// empty for a 1-variable atom).
fn plan_left_deep(atoms: &[Atom]) -> Result<(LayerBgpPlan, Vec<String>), BgpPlanError> {
    let k = atoms.len();

    let ground_count = atoms.iter().filter(|atom| atom.kept.is_empty()).count();
    if ground_count == k {
        return Ok((
            LayerBgpPlan {
                atoms: atoms.iter().map(|atom| atom.layer_atom.clone()).collect(),
                joins: Vec::new(),
                source: ColumnSource::Materialized,
            },
            Vec::new(),
        ));
    }
    let relational_count = k - ground_count;

    // Deterministic start atom: input order 0. Its kept variables seed the accumulator.
    let mut acc_vars: Vec<String> = atoms[0].kept.clone();
    let mut placed = vec![false; k];
    placed[0] = true;

    let mut ordered_atoms: Vec<LayerBgpAtom> = vec![atoms[0].layer_atom.clone()];
    let mut joins: Vec<JoinStep> = Vec::with_capacity(relational_count - 1);

    let mut placed_count = 1usize;
    while placed_count < relational_count {
        // Find an unplaced atom sharing at least one variable with the accumulator.
        let mut chosen: Option<usize> = None;
        for (i, atom) in atoms.iter().take(relational_count).enumerate() {
            if placed[i] {
                continue;
            }
            if shared_count(&acc_vars, &atom.kept) >= 1 {
                chosen = Some(i);
                break;
            }
        }

        let i = chosen.unwrap_or_else(|| {
            placed
                .iter()
                .take(relational_count)
                .position(|is_placed| !is_placed)
                .expect("an unplaced relational atom must remain")
        });

        // Join on every shared variable. The first pair remains the scalar-key fields;
        // further pairs activate the Store chain's authenticated composite-key mode.
        let atom = &atoms[i];
        let shared: Vec<(usize, usize)> = atom
            .kept
            .iter()
            .enumerate()
            .filter_map(|(rel, variable)| {
                acc_vars
                    .iter()
                    .position(|acc| acc == variable)
                    .map(|acc| (acc, rel))
            })
            .collect();
        if shared.is_empty() {
            joins.push(JoinStep::cross_product());
            acc_vars.extend(atom.kept.iter().cloned());
            ordered_atoms.push(atom.layer_atom.clone());
            placed[i] = true;
            placed_count += 1;
            continue;
        }
        let (acc_key, rel_key) = shared[0];
        let v = atom.kept[rel_key].clone();
        let additional_keys = shared[1..].to_vec();

        if additional_keys.is_empty() {
            // Scalar path preserves the audited binary-join output ordering.
            let mut r_other = acc_vars.clone();
            r_other.remove(acc_key);
            let mut s_other = atom.kept.clone();
            s_other.remove(rel_key);
            r_other.push(v);
            r_other.extend(s_other);
            acc_vars = r_other;
        } else {
            // Composite path retains the accumulator in place and appends only variables
            // not already represented by one of the shared key columns.
            acc_vars.extend(
                atom.kept
                    .iter()
                    .filter(|variable| !acc_vars.contains(variable))
                    .cloned()
                    .collect::<Vec<_>>(),
            );
        }

        joins.push(JoinStep {
            acc_key,
            rel_key,
            additional_keys,
        });
        ordered_atoms.push(atom.layer_atom.clone());
        placed[i] = true;
        placed_count += 1;
    }

    ordered_atoms.extend(
        atoms[relational_count..]
            .iter()
            .map(|atom| atom.layer_atom.clone()),
    );

    Ok((
        LayerBgpPlan {
            atoms: ordered_atoms,
            joins,
            source: ColumnSource::Materialized,
        },
        acc_vars,
    ))
}

/// The plan DEFAULT column source for a BGP leaf. Each atom now carries its OWN source
/// ([`LayerBgpAtom::source`], threaded in [`atom_from_triple`]), so a CROSS-SOURCE conjunction
/// — a materialized `Triple` joined with a delta atom, or additions joined with removals — is
/// now SUPPORTED (the store joins over the shared dictionaries). This returns the source to
/// stamp on `plan.source`: when every atom agrees it is that uniform source (which keeps a
/// single-source plan's transcript byte-identical to the pre-per-atom encoding — the store
/// binds a per-atom source override ONLY when it diverges from `plan.source`); when they
/// diverge it is `Materialized`, the neutral default that emits no plan-level source tag. An
/// empty triple list defaults to materialized (the surrounding `k == 0` check then rejects it
/// as too few atoms).
fn plan_default_column_source(triples: &[TypedTriple]) -> ColumnSource {
    let mut sources = triples.iter().map(|triple| triple.source);
    let first = sources.next().unwrap_or(ColumnSource::Materialized);
    if sources.all(|source| source == first) {
        first
    } else {
        ColumnSource::Materialized
    }
}

/// How many of an atom's KEPT (variable) columns occur in the accumulator schema. Because
/// repeated variables within an atom are represented once after their same-row equality
/// constraint is recorded, and `acc_vars` also holds distinct names, so this is an exact
/// shared-variable count in `0..=kept.len()`.
fn shared_count(acc_vars: &[String], kept: &[String]) -> usize {
    kept.iter()
        .filter(|kv| acc_vars.iter().any(|a| a == *kv))
        .count()
}

#[cfg(test)]
mod decimal_layer_render_tests {
    use super::*;

    /// Build an exact `BigRational` from a decimal numerator/denominator pair.
    fn rat(n: &str, d: &str) -> BigRational {
        BigRational::new(
            BigInt::from_str_radix(n, 10).unwrap(),
            BigInt::from_str_radix(d, 10).unwrap(),
        )
    }

    /// EVERY row is `(numerator, denominator, expected_lexical)` captured from a REAL SWI-Prolog run
    /// of `casting:rational_to_decimal_string(N rdiv D, S, 20)` (with `prefer_rationals`). Locks the
    /// renderer byte-exact against the reference across terminating, non-terminating (rounded to 20
    /// fractional digits, round-half-up, trailing zeros trimmed), whole, negative, and
    /// large-denominator cases — including the `den = 2·10^20` boundary that rounds up at digit 20.
    #[test]
    fn rational_to_decimal_string_matches_swipl_vectors() {
        let vectors: &[(&str, &str, &str)] = &[
            ("7", "2", "3.5"),
            ("6", "2", "3"),
            ("1", "4", "0.25"),
            ("1", "3", "0.33333333333333333333"),
            ("2", "3", "0.66666666666666666667"),
            ("10", "4", "2.5"),
            ("-7", "2", "-3.5"),
            ("-1", "3", "-0.33333333333333333333"),
            ("1", "8", "0.125"),
            ("22", "7", "3.14285714285714285714"),
            ("1", "40", "0.025"),
            ("0", "5", "0"),
            ("5", "1", "5"),
            ("-5", "1", "-5"),
            ("1", "7", "0.14285714285714285714"),
            ("1", "6", "0.16666666666666666667"),
            ("1", "9", "0.11111111111111111111"),
            ("1", "11", "0.09090909090909090909"),
            ("1", "13", "0.07692307692307692308"),
            ("100", "3", "33.33333333333333333333"),
            ("999", "1000", "0.999"),
            ("1", "2", "0.5"),
            ("3", "8", "0.375"),
            ("-22", "7", "-3.14285714285714285714"),
            ("123456", "1000", "123.456"),
            ("1", "50", "0.02"),
            ("7", "20", "0.35"),
            ("1", "16", "0.0625"),
            ("355", "113", "3.14159292035398230088"),
            ("2", "7", "0.28571428571428571429"),
            ("1", "17", "0.05882352941176470588"),
            ("10", "3", "3.33333333333333333333"),
            ("-10", "3", "-3.33333333333333333333"),
            ("1", "80", "0.0125"),
            ("17", "64", "0.265625"),
            ("1", "1024", "0.0009765625"),
            ("999999", "7", "142857"),
            ("-1", "1000000", "-0.000001"),
            ("12345678901234567890", "3", "4115226300411522630"),
            ("1", "99999999999", "0.00000000001"),
            ("50000000000000000000", "3", "16666666666666666666.66666666666666666667"),
            ("1", "200000000000000000000", "0.00000000000000000001"),
        ];
        for (n, d, expected) in vectors {
            let got = rational_to_decimal_string_20(&rat(n, d)).unwrap();
            assert_eq!(&got, expected, "{n}/{d} rendered wrong");
        }
    }

    /// The decimal-string parser is the exact inverse used for stored decimal operands and decimal
    /// literals: parsing a canonical lexical then re-rendering must round-trip when it fits 20
    /// fractional digits.
    #[test]
    fn parse_decimal_round_trips_canonical_lexicals() {
        for s in ["3.5", "-3.5", "0.25", "42", "-42", "0", "0.025", "123.456", "-0.000001"] {
            let r = parse_decimal_to_rational(s).unwrap();
            assert_eq!(rational_to_decimal_string_20(&r).unwrap(), s, "round-trip {s}");
        }
        // Leading-dot and trailing-dot canonicalization.
        assert_eq!(parse_decimal_to_rational(".5").unwrap(), rat("1", "2"));
        // Non-canonical / scientific / garbage forms reject closed.
        for bad in ["1e5", "1.2.3", "abc", "", "-", "1,5", " 1"] {
            assert!(parse_decimal_to_rational(bad).is_err(), "should reject {bad:?}");
        }
    }

    /// The carry-overflow guard: a fraction that rounds up to a whole unit at 20 digits (requires a
    /// reduced denominator >= 2·10^20) FAILS CLOSED rather than reproduce the reference carry bug.
    #[test]
    fn rational_render_carry_overflow_fails_closed() {
        // (3·10^20 - 1)/(3·10^20): true value 0.9999...9(6r6...), rounds up to 1.0 at 20 digits.
        let den = "300000000000000000000";
        let num = "299999999999999999999";
        assert!(rational_to_decimal_string_20(&rat(num, den)).is_err());
    }
}
