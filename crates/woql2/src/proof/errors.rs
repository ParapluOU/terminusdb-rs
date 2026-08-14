//! The rejection taxonomy — every reason the compiler refuses a query.
//!
//! [`plan_bgp`] and its helpers are "reject-closed": anything the authenticated plan cannot
//! encode faithfully becomes a descriptive [`BgpPlanError`] rather than being silently
//! mis-proven. This submodule defines that error enum (one variant per rejection reason) with
//! its `Display` text, plus small helpers ([`query_variant_name`] and the per-construct reject
//! reasons) used to build precise messages. Rejecting soundly is a safety feature: the plan is
//! the sole line between the provable fragment and the unsupported.

use super::*;

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

    // ----- WOQL `ReadDocument` (increment 2b: frame-authenticated flat + list documents) -----
    /// The `ReadDocument` identifier is a VARIABLE, not a bound node IRI. A document proof binds
    /// exactly ONE stored subject, so v1 needs a ground id; a variable id (enumerate all documents)
    /// is a deferred follow-up.
    #[error("ReadDocument identifier must be a bound node IRI, not a variable")]
    ReadDocumentUnboundIdentifier,

    /// RESERVED (no longer produced). `ReadDocument` now proves INHERITING document classes: the
    /// effective frame is the union of directly-declared properties across the AUTHENTICATED reflexive
    /// `sys:inherits` closure `{C} ∪ ancestors(C)` (see `proof/document.rs`). A `sys:inherits` edge is
    /// therefore accepted, not rejected. Retained as a public variant for API stability; the residual
    /// inherited-frame rejects are now specific shapes (`conflicting-inherited-range`, an inherited
    /// subdocument via [`Self::ReadDocumentSubdocumentDeferred`], an inherited tagged-union/enum via
    /// [`Self::ReadDocumentUnsupportedPropertyShape`]).
    #[error("ReadDocument class has a sys:inherits edge; the inherited-frame closure is deferred (v1 reads direct frames only)")]
    ReadDocumentInheritedFrame,

    /// A property's frame shape is one v1 cannot render faithfully (e.g. a cardinality-bearing `Set`,
    /// `Optional`, `Table`, `sys:oneOf` tagged union, enum/foreign/unit range, an inheriting link/element
    /// target, an undefined or non-node range, or a MULTI-DIMENSIONAL `sys:Array` — `shape =
    /// "array-multidim"` — the store composite reads only the level-1 `sys:index`). Rendering it as a
    /// plain property could omit or misclassify data, so it reject-closes. `shape` names the specific
    /// unsupported form. (A 1-D DENSE array of scalar/plain-link elements is now SUPPORTED — increment 4;
    /// a SPARSE/gapped array is rejected downstream by the store density certificate, not here.)
    #[error("ReadDocument property shape `{shape}` is unsupported in v1 (flat scalar/link/set + non-subdocument list + 1-D dense array); reject closed")]
    ReadDocumentUnsupportedPropertyShape { shape: &'static str },

    /// The complete, root-authenticated direct-triple set of the document carries a predicate `P` that
    /// the class frame does NOT define (schema non-conformance / injection). The delivered document
    /// could not be rendered faithfully without this property being frame-classified, so it
    /// reject-closes — the no-injection guarantee.
    #[error("ReadDocument instance property `{predicate}` is absent from the authenticated class frame (no-injection violation)")]
    ReadDocumentUnknownInstanceProperty { predicate: String },

    /// A `List`-typed property does not have EXACTLY ONE head edge `t(id, P, head)` in the
    /// authenticated direct-triple set (0 = absent is fine, but a PRESENT list must be single-headed;
    /// >1 head is malformed), or its ordered-walk sub-proof is missing / mismatched.
    #[error("ReadDocument list property `{predicate}` is malformed: not exactly one head edge, or its ordered-walk sub-proof is missing/extra")]
    ReadDocumentMalformedListHead { predicate: String },

    /// The TOP-LEVEL document id's OWN class is a subdocument (`sys:subdocument`, inheritance-closed).
    /// A subdocument is a child inlined INTO a parent document; it is not itself a top-level document,
    /// so reading one directly by id is out of scope — reject closed. (Increment 3 INLINES subdocument
    /// CHILDREN of a normal document, keyed off each child's authenticated type; only a subdocument as
    /// the outermost `id` reject-closes here. Repurposed from the old subdocument-range reject, which
    /// increment 3 removed: a subdocument RANGE is now inlined via the per-child recursion.)
    #[error("ReadDocument top-level id class is a subdocument; a subdocument is inlined into a parent, not read directly by id; reject closed")]
    ReadDocumentSubdocumentDeferred,

    /// A target the reference would UNFOLD inline that v1 renders as a bare id. Raised for a single-valued
    /// class LINK child whose authenticated type (or an ancestor) is `@unfoldable` (`sys:unfoldable`) OR
    /// whose property is declared `@unfold` (`sys:unfold` on the frame range node); AND (increment-3 audit
    /// fix) for a `Set`/`List` collection that would unfold — an `@unfold` collection property, or an
    /// element whose ACTUAL authenticated type (reflexive `sys:inherits` closure) bears
    /// `sys:subdocument` / `sys:unfoldable`. `get_document(unfold(true))` inlines such content; v1 inlines
    /// only true single-valued SUBDOCUMENTS and never collection elements, so silently rendering a bare id
    /// would OMIT the inlined content the reference includes. That is a SOUNDNESS gap (a smaller-than-
    /// reference document), so we reject closed rather than under-deliver.
    #[error("ReadDocument link/collection target is `@unfoldable`/`@unfold` or has a subdocument/unfoldable actual type (reference unfolds it inline); v1 inlines only single-valued subdocuments and never collection elements, so reject closed rather than omit content")]
    ReadDocumentUnfoldableDeferred,

    /// A subdocument INLINE recursion reached a child IRI already on the inline path (a cycle). Genuine
    /// subdocuments have parent-nested generated IRIs and cannot form a cycle in valid data; we
    /// reject-close rather than loop.
    #[error("ReadDocument subdocument inlining reached an already-inlined child IRI (cycle); reject closed rather than loop")]
    ReadDocumentSubdocumentCycle,

    /// A subdocument INLINE recursion exceeded the depth or node budget. Enforced IDENTICALLY in prove
    /// and verify so a prover can never make verify accept a TRUNCATED subdocument tree; over-budget
    /// reject-closes rather than truncate.
    #[error("ReadDocument subdocument inlining exceeded the depth/node budget; reject closed rather than truncate")]
    ReadDocumentSubdocumentBudgetExceeded,
}

/// The name of a `Query` variant, for descriptive rejection messages. Kept tiny and
/// covering the constructs a caller is most likely to hit.
pub(crate) fn query_variant_name(q: &Query) -> &'static str {
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

