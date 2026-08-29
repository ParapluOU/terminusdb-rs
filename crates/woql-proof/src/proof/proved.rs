//! Proving a compiled relation (`impl ProvedBgp`).
//!
//! This submodule carries the large `impl ProvedBgp` block. Given a [`CompiledRelation`] and
//! a prover context bound to a committed layer, it walks the recursive plan tree and asks the
//! Store to PROVE each node — base BGP scans, filters, natural / anti / outer joins,
//! aggregates, ordering, distinct, bag unions, the value / eval / string layers, and
//! transitive-closure paths — assembling the per-node proofs into one relation proof. The
//! compiler decides WHAT to prove; the Store's proof primitives do the cryptography. The
//! matching verification walks the same structure so prover and verifier stay in lock-step.
//!
//! This is the **prove** and **verify** step of compile → prove → verify. The verify entries
//! (`verify` / `verify_over_graphs` / the `verify_and_resolve*` family / `verify_executed_rows`)
//! anchor everything to a caller-supplied TRUSTED root and re-derive the plan from the caller's
//! own compiled query — so a proof for one query cannot be substituted for another
//! (anti-substitution). See [`terminus_store::proof::primer`] §10 (prove→verify) and §9 (the
//! `resolution_root` trust anchor).

use super::*;

/// β re-anchor gate: confirm the layer's `resolution_root` equals the externally-trusted
/// anchor — which binds `commitment_root` AND both id-indexed dictionary Merkle roots
/// DIRECTLY (a fabricated Merkle root that would resolve ids to arbitrary terms cannot hash
/// to the trusted anchor) — and return the now-authenticated `commitment_root` to thread into
/// the transcript-bound recursion unchanged.
fn expect_resolution_root(
    layer: &impl VerifierLayer,
    trusted_resolution_root: ProofHash,
) -> std::io::Result<ProofHash> {
    let view = layer.verifier_view();
    if view.resolution_root() != trusted_resolution_root {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "layer resolution_root does not match the externally-trusted resolution root",
        ));
    }
    Ok(view.state.commitment_root)
}

/// True iff the relation attests a specific row ORDER that the proof cryptographically BINDS — a
/// direct exact-order/ordered-slice or ordered list/array walk, or such an order beneath a spine of
/// wrappers whose VERIFIER pins the output rows to the child rows positionally. When true, the
/// executor must deliver rows in that order (positional comparison); otherwise the result is a bag
/// (multiset comparison). Recursion bottoms out at `false` for a plain leaf, so a filtered/mapped BAG
/// (no order at the spine bottom) is still classified unordered — this only RECOGNIZES orders whose
/// order is actually bound, and never over-rejects a genuine bag.
///
/// The classification is by what the PROOF attests, NOT by honest-prover behavior:
///   - The per-row value maps (`Eval`/`FloorEval`/`RationalArith`/`Typecast`/the two number/string
///     casts/`Concat`/`Join`/`Substring`/`Pad`/`HashKey`/`CalendarInt`/`CalendarDate`) and the
///     generated-column filters (`GeneratedU64Filter`/`GeneratedI64Filter`) are recursed because
///     their VERIFIER recomputes/pins every output column positionally against the child (identical
///     length/domain; kept rows reconstructed at increasing indices). `BagProjection` index-selects
///     whole child columns with unchanged row count. So these DO bind the child's order.
///   - `Equality`/`TupleDisequality` are NOT recursed even though the honest prover compacts
///     survivors stably: the store's `filter::compact_columns` link (`verify_compaction_only`)
///     authenticates only the surviving-row MULTISET, not survivor POSITIONS (a valid proof can
///     present survivors in any order — cf. store test `atom_filter_reordered_output_verifies`), so
///     an order beneath a filter is not bound and must compare as a multiset.
///   - `When` has no store proof node (verification delegates transparently to `expanded`), so it is
///     order-transparent and recurses into `expanded`.
///   - Order-DESTROYING nodes (relational joins, bag-union, distinct, aggregates, closures) and
///     plain/scalar leaves return false.
fn relation_attests_order(statement: &CompiledRelationStatement) -> bool {
    use CompiledRelationStatement::*;
    match statement {
        // Direct order attestation. The exact-order gadgets bind the output to a proven sorted
        // order; the ordered WALKS bind an exact list/array index order in the proof itself
        // (`verify_ordered_list_walk`; `verify_density_order` with output index column [0..n)).
        ExactScalarOrder { .. } | ExactMultiOrder { .. } | ExactOrderedSlice { .. }
        | OrderedListWalk { .. } | OrderedArrayWalk { .. } => true,
        // POSITION-PRESERVING unary wrappers whose PROOF (not merely the honest prover) binds each
        // output row to the child row in the child's order — the verifier RECOMPUTES/pins every
        // output column positionally against the child, so an order beneath is faithfully preserved.
        // Per-row value maps (verifier recomputes the exact columns, inheriting result_len/order):
        Eval { input, .. } | FloorEval { input, .. } | RationalArith { input, .. }
        | Typecast { input, .. } | TypecastNumberToString { input, .. }
        | TypecastStringToNumber { input, .. } | Concat { input, .. } | Join { input, .. }
        | Substring { input, .. } | Pad { input, .. } | HashKey { input, .. }
        | CalendarInt { input, .. } | CalendarDate { input, .. }
        // Generated-column filters (verifier reconstructs the exact kept columns at increasing
        // indices) and column projection (verifier index-selects whole child columns, same length):
        | GeneratedU64Filter { input, .. } | GeneratedI64Filter { input, .. }
        | BagProjection { input, .. } => relation_attests_order(&input.statement),
        // `When` has no store proof node — planning and verification delegate transparently to
        // `expanded`, preserving that relation's order — so recurse into it. (Ordinary lowering puts
        // a `BagUnion` under `When`, which bottoms out at false; this covers direct/manual plans.)
        When { expanded, .. } => relation_attests_order(&expanded.statement),
        // Everything else is order-DESTROYING (relational joins, BagUnion, Distinct, aggregates,
        // closures) or an unordered leaf: false — the executor may deliver any permutation.
        //
        // `Equality` / `TupleDisequality` are DELIBERATELY excluded even though the honest prover
        // compacts survivors stably: their store proof (`filter::compact_columns` link) authenticates
        // only the surviving-row MULTISET, not stable survivor POSITIONS (a valid proof can present
        // survivors in any order). So an order beneath a filter is NOT cryptographically attested and
        // MUST compare as a multiset — we never claim a row order the proof does not bind.
        _ => false,
    }
}

impl ProvedBgp {
    /// The leaf BGP proof and its authenticated atom predicate ids when this is a
    /// [`ProvedQueryStatement::Single`], else `None`.
    pub fn as_single(&self) -> Option<(&LayerBgpProof, &[u64])> {
        match &self.statement {
            ProvedQueryStatement::Single {
                proof,
                predicate_ids,
            } => Some((proof, predicate_ids)),
            _ => None,
        }
    }

    /// Merkle-dict I2b S5: the TERM-LESS resolution witness shipped inside a single-BGP proof
    /// (`Single.proof.openings`), or `None` for a term-bearing proof / a non-`Single` statement
    /// (which is out of the term-less base/rollup scope and resolves via the rebuilt tree). The
    /// verify+resolve entries pass this to the store's `resolve_*_with_openings`, so a term-less
    /// base/rollup BGP (and the materialized ReadDocument subject read, which is a `Single` BGP)
    /// resolves against the shipped openings; a term-less proof WITHOUT them fails closed.
    pub fn single_openings(&self) -> Option<&ResolutionOpenings> {
        self.as_single().and_then(|(proof, _)| proof.openings.as_ref())
    }

    /// The proven branches and the union proof when this is a [`ProvedQueryStatement::BagUnion`],
    /// else `None`.
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
        trusted_resolution_root: ProofHash,
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
        // β re-anchor: the EXTERNAL trust anchor is the layer's `resolution_root` (binds
        // `commitment_root` AND the two dictionary Merkle roots directly). Gate on it, then thread
        // the now-authenticated `commitment_root` into the transcript-bound recursion unchanged.
        let commitment_root = expect_resolution_root(layer, trusted_resolution_root)?;
        self.verify_relation(
            &compiled.relation,
            layer,
            commitment_root,
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
    ///      (`relation_graph`); the graph selector is bound into the query digest, so a prover cannot
    ///      make a schema query verify against the instance graph (or vice versa).
    ///   3. Anchor the audited recursion to the matching commitment + its (now-trusted) root.
    ///
    /// An instance-only query verifies identically to `verify`, with the added guarantee that its
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
                    envelope.verify_cross_root(instance, schema, database_root, &plan)?;
                    // BIND the outer proof node's delivered result to the now-authenticated store
                    // envelope (mirrors the wholesale-envelope arm at proved.rs ~1785). Without this
                    // the OUTER `self.result_columns` the WOQL layer trusts are unbound to the proven
                    // semi-join output, so a prover could substitute authenticated ids after the fact.
                    if self.result_commitments != envelope.result_commitments
                        || self.result_columns != envelope.result_columns
                        || self.result_len != envelope.result_len
                        || self.result_log_domain != envelope.result_log_domain
                    {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "WOQL cross-root delivered result does not match the proven store envelope",
                        ));
                    }
                    Ok(())
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

    /// `verify_over_graphs` that also RESOLVES the delivered result objects (rendered against the
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
                    // BIND the outer `self.result_columns` (resolved below via
                    // `resolve_typed_result_objects`) to the now-authenticated store envelope
                    // (mirrors the wholesale-envelope arm at proved.rs ~1785). Otherwise a prover
                    // could keep a genuine envelope yet deliver substituted authenticated ids.
                    if self.result_commitments != envelope.result_commitments
                        || self.result_columns != envelope.result_columns
                        || self.result_len != envelope.result_len
                        || self.result_log_domain != envelope.result_log_domain
                    {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "WOQL cross-root delivered result does not match the proven store envelope",
                        ));
                    }
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
                instance, instance.resolution_root(), &self.result_columns, &namespaces,
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
        // CROSS-PATH GUARD (E1): a TERM-LESS DELTA view (`parent_state_root == Some` and no
        // clear catalogs) is now constructible/decodable, but a delta is an INCREMENT — not the
        // full resolved state — so it must never verify on this single-layer envelope path (which
        // reads/authenticates ONE layer's own `commitment_root`). This is the woql2-side choke that
        // every single-layer `ProvedBgp` verify entry funnels through (verify / verify_over_graphs'
        // single-graph branch / verify_and_resolve* / verify_objects / verify_executed_*), including
        // the woql2-level-only reads (e.g. `LayerTripleCount` reads the view directly, never touching
        // the store's `require_base_layer_for_query`). A term-BEARING delta is out of scope here and
        // keeps its existing behavior. Delta querying is opened by the reconciled/delta-read path (E2).
        let view = layer.verifier_view();
        if view.state.parent_state_root.is_some() && !view.terms_present {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "term-less delta layer cannot be verified on the single-layer base/rollup envelope path \
                 (a delta is an increment, not the full state); verify it through the reconciled/delta-read path",
            ));
        }
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
                    layer.resolution_root(),
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
            (
                ProvedQueryStatement::OrderedListWalk { envelope },
                CompiledRelationStatement::OrderedListWalk { .. },
            ) => {
                // DELEGATE to the audited store verify. The store envelope carries the WHOLE
                // OrderedListWalk composite proof (its internals are store-private), so — like the
                // `CrossRootHashJoin` leaf — the woql2 layer carries it wholesale rather than
                // decomposing it. Rebuild the plan from the TRUSTED compiled statement: its
                // `query_digest` folds the bound (subject, predicate) (see `relation_query_digest`),
                // and the store's own `verify` rejects any envelope whose root or query digest does
                // not match this plan — so the proof is BOUND to this exact (subject, predicate) list
                // and cannot be re-aimed at another.
                let plan = relation_proof_plan(compiled, query_digest)?;
                envelope.verify(layer, expected_root, &plan)?;
                // BIND the wholesale envelope's delivered result to THIS proof node's delivered
                // columns. The envelope verified its OWN result columns against the root above;
                // requiring `self == envelope` here means the columns the outer `verify_columns`
                // (and any downstream caller) reads are exactly the ones the store proved — a
                // mismatched outer copy cannot slip a different answer past verification.
                if self.result_commitments != envelope.result_commitments
                    || self.result_columns != envelope.result_columns
                    || self.result_len != envelope.result_len
                    || self.result_log_domain != envelope.result_log_domain
                {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "WOQL ordered-list-walk delivered result does not match the proven store envelope",
                    ));
                }
            }
            (
                ProvedQueryStatement::OrderedArrayWalk { envelope },
                CompiledRelationStatement::OrderedArrayWalk { .. },
            ) => {
                // DELEGATE to the audited store verify. The store envelope carries the WHOLE
                // OrderedArrayWalk composite proof (its internals are store-private), so — like the
                // `OrderedListWalk` / `CrossRootHashJoin` leaf — the woql2 layer carries it wholesale
                // rather than decomposing it. Rebuild the plan from the TRUSTED compiled statement: its
                // `query_digest` folds the bound (subject, predicate) (see `relation_query_digest`),
                // and the store's own `verify` rejects any envelope whose root or query digest does not
                // match this plan — so the proof is BOUND to this exact (subject, predicate) array and
                // cannot be re-aimed at another.
                let plan = relation_proof_plan(compiled, query_digest)?;
                envelope.verify(layer, expected_root, &plan)?;
                // BIND the wholesale envelope's delivered result to THIS proof node's delivered columns
                // (identical to the list-walk arm above): a mismatched outer copy cannot slip a
                // different answer past verification.
                if self.result_commitments != envelope.result_commitments
                    || self.result_columns != envelope.result_columns
                    || self.result_len != envelope.result_len
                    || self.result_log_domain != envelope.result_log_domain
                {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "WOQL ordered-array-walk delivered result does not match the proven store envelope",
                    ));
                }
            }
            (
                ProvedQueryStatement::TransitiveClosurePairs { envelope },
                CompiledRelationStatement::TransitiveClosurePairs { .. }
                | CompiledRelationStatement::TransitiveClosurePairsCompound { .. },
            ) => {
                // DELEGATE to the audited store verify. The store envelope carries the WHOLE 2-column
                // closure proof (edge / seed / rounds / empty-delta fixpoint), so — like the ordered
                // walks — the woql2 layer carries it wholesale rather than decomposing it. This ONE arm
                // covers BOTH the single-predicate raw-edge all-pairs closure and the DERIVED-edge
                // (all-pairs wildcard / compound; W1) closure — they share the store
                // `RecursiveRelationProof::TransitiveClosurePairs` envelope, so the wholesale carry is
                // identical; only the reconstructed plan's edge differs. Rebuild the plan from the TRUSTED
                // compiled statement: its `query_digest` folds the edge structure, direction and the
                // DISTINCT `/transitive-closure-pairs`(`-compound`) tag, and the store's own `verify`
                // rejects any envelope whose root or query digest does not match — so an all-pairs proof
                // cannot be re-aimed at another edge/direction, nor cross-replayed as a single-seed
                // closure, nor a derived-edge proof replayed as a single-predicate one.
                let plan = relation_proof_plan(compiled, query_digest)?;
                envelope.verify(layer, expected_root, &plan)?;
                // BIND the wholesale envelope's delivered result to THIS proof node's delivered columns
                // (identical to the ordered-walk arms): a mismatched outer copy of EITHER column cannot
                // slip a different pairs answer past verification.
                if self.result_commitments != envelope.result_commitments
                    || self.result_columns != envelope.result_columns
                    || self.result_len != envelope.result_len
                    || self.result_log_domain != envelope.result_log_domain
                {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "WOQL all-pairs closure delivered result does not match the proven store envelope",
                    ));
                }
            }
            (
                ProvedQueryStatement::TransitiveClosureCompound { envelope },
                CompiledRelationStatement::TransitiveClosureCompound { .. },
            ) => {
                // DELEGATE to the audited store verify. The compound closure carries the WHOLE store
                // envelope (edge / seed / rounds / empty-delta fixpoint over the derived edge), so — like
                // the all-pairs closure and the ordered walks — the woql2 layer carries it wholesale
                // rather than decomposing it. Rebuild the plan from the TRUSTED compiled statement: its
                // `query_digest` folds the compound EDGE structure, seed, reflexive and direction under a
                // DISTINCT `/transitive-closure-compound` tag, and the store's own `verify` rejects any
                // envelope whose root or query digest does not match — so a compound proof cannot be
                // re-aimed at another edge/seed/direction, nor cross-replayed as a single-predicate or
                // all-pairs closure.
                let plan = relation_proof_plan(compiled, query_digest)?;
                envelope.verify(layer, expected_root, &plan)?;
                // BIND the wholesale envelope's delivered result to THIS proof node's delivered columns
                // (identical to the all-pairs / ordered-walk arms): a mismatched outer copy cannot slip a
                // different reachable set past verification.
                if self.result_commitments != envelope.result_commitments
                    || self.result_columns != envelope.result_columns
                    || self.result_len != envelope.result_len
                    || self.result_log_domain != envelope.result_log_domain
                {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "WOQL compound closure delivered result does not match the proven store envelope",
                    ));
                }
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

    /// Merkle-dict I2b S5 (PRODUCER): mint + attach the TERM-LESS resolution witness to a freshly
    /// proven single-BGP `ProvedBgp`, so the proof can ship alongside a term-less artifact
    /// (`verifier_commitment_termless`) and still verify + resolve. `layer` is the TERM-BEARING
    /// prover layer (it still holds the catalog to open against). The openings are UNCOMMITTED (the
    /// verifier re-checks each against the trusted root). No-op for a non-single-BGP statement
    /// (outside the term-less base/rollup scope). Base/rollup only — the store builder rejects a
    /// delta layer. This is the woql2/ReadDocument analogue of the store's
    /// `attach_termless_bgp_openings`.
    pub fn attach_termless_openings(
        &mut self,
        compiled: &CompiledBgp,
        layer: &impl VerifierLayer,
    ) -> std::io::Result<()> {
        let plan = match &compiled.relation.statement {
            CompiledRelationStatement::Bgp(plan) => plan.clone(),
            _ => return Ok(()),
        };
        let columns = self.result_columns.clone();
        if let ProvedQueryStatement::Single {
            proof,
            predicate_ids,
        } = &mut self.statement
        {
            let openings =
                build_termless_bgp_openings(layer, &plan, proof, predicate_ids, &columns)?;
            proof.openings = Some(openings);
        }
        Ok(())
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
        trusted_resolution_root: ProofHash,
    ) -> std::io::Result<Vec<Vec<CanonicalObject>>> {
        // SCHEMA-GRAPH GUARD (mirror of `prove_on_layer`): a schema-reading query must NOT resolve
        // against a single caller-supplied root — route it through `verify_over_graphs_and_resolve`.
        if relation_graph(&compiled.relation)? != ProofGraph::Instance {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "query reads the schema graph; verify with `verify_over_graphs` (instance + schema commitments under the trusted database_root)",
            ));
        }
        // β re-anchor gate: bind the trusted `resolution_root`, then thread the authenticated
        // `commitment_root` into the transcript-bound recursion.
        let commitment_root = expect_resolution_root(layer, trusted_resolution_root)?;
        self.verify_and_resolve_inner(compiled, layer, commitment_root)
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
            // Merkle-dict I2b S5: consume the single-BGP proof's term-less openings (`None` on a
            // term-bearing proof — resolution rebuilds the tree). This is the resolution primitive
            // the materialized ReadDocument subject read also flows through (`resolve_subject_rows`).
            resolve_typed_result_objects_with_openings(
                layer,
                layer.resolution_root(),
                &self.result_columns,
                &namespaces,
                self.single_openings(),
            )
        }
    }

    /// LOW-LEVEL, INSTANCE-ONLY (mirror of [`CompiledBgp::prove_on_layer`]): a schema-reading query
    /// rejects closed here — no combined-root values entry routes through it, so this is a hard reject.
    pub fn verify_and_resolve_values(
        &self,
        compiled: &CompiledBgp,
        layer: &impl VerifierLayer,
        trusted_resolution_root: ProofHash,
    ) -> std::io::Result<Vec<Vec<CanonicalResultValue>>> {
        // SCHEMA-GRAPH GUARD (mirror of `prove_on_layer`): schema-reading queries must not resolve
        // against a single caller-supplied root. Instance queries are unaffected (byte-identical).
        if relation_graph(&compiled.relation)? != ProofGraph::Instance {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "query reads the schema graph; verify with `verify_over_graphs` (instance + schema commitments under the trusted database_root)",
            ));
        }
        // β re-anchor: `verify` gates on the trusted `resolution_root`; the resolve below opens
        // against the same authenticated layer.
        self.verify(compiled, layer, trusted_resolution_root)?;
        if compiled.relation.schema.is_empty() {
            Ok(Vec::new())
        } else {
            // Merkle-dict I2b S5: route term-less resolution through the single-BGP openings.
            resolve_result_values_with_openings(
                layer,
                layer.resolution_root(),
                &self.result_columns,
                &compiled_result_descriptors(compiled)?,
                self.single_openings(),
            )
        }
    }

    /// Verify caller-delivered canonical node IRIs/typed literals against the query
    /// proof, its committed result IDs, and the root-authenticated Store dictionary.
    pub fn verify_objects(
        &self,
        compiled: &CompiledBgp,
        layer: &impl VerifierLayer,
        trusted_resolution_root: ProofHash,
        objects: &[Vec<CanonicalObject>],
    ) -> std::io::Result<()> {
        self.verify(compiled, layer, trusted_resolution_root)?;
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
                layer.resolution_root(),
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
    /// executor supplies Store dictionary IDs in its public variable order, resolved
    /// through the authenticated dictionary. For an UNORDERED (bag/set) relation the
    /// comparison is an exact row-multiset comparison. For an EXACT-ORDER relation
    /// (`OrderBy` / ordered-slice) the proof fixes the exact row order, so the executor's
    /// rows are compared POSITIONALLY (in the proven order, no sort).
    pub fn verify_executed_rows(
        &self,
        compiled: &CompiledBgp,
        layer: &impl VerifierLayer,
        trusted_resolution_root: ProofHash,
        variables: &[String],
        rows: &[Vec<u64>],
    ) -> std::io::Result<()> {
        if let Some(count_variable) = compiled.count_variable.as_deref() {
            self.verify(compiled, layer, trusted_resolution_root)?;
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
            self.verify(compiled, layer, trusted_resolution_root)?;
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

        let proven_objects = self.verify_and_resolve(compiled, layer, trusted_resolution_root)?;
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
            layer.resolution_root(),
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

        if relation_attests_order(&compiled.relation.statement) {
            // ORDER-ATTESTING: the proof fixes the exact row order; the executor must deliver it in
            // that order. Compare positionally (lengths already constrained above by result_len/rows.len()).
            if expected_rows != delivered_rows {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "ordinary WOQL ordered result rows do not match the proven result order",
                ));
            }
        } else {
            // UNORDERED (bag/set) relation: order is not attested, so compare as a multiset.
            expected_rows.sort();
            delivered_rows.sort();
            if expected_rows != delivered_rows {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "ordinary WOQL result rows do not match the proven result multiset",
                ));
            }
        }
        Ok(())
    }

    /// Like [`Self::verify_executed_rows`], but for a relation with NULLABLE or GENERATED-decimal
    /// columns: the executor supplies typed [`ExecutedResultValue`] cells (Store id / signed or
    /// unsigned generated decimal / null) rather than bare ids. Checks the delivered variable list
    /// and row width against the compiled projection, then confirms the rows against the proof
    /// (multiset, or positional for an exact-order relation). `Count` bindings use the scalar
    /// boundary instead and are rejected here.
    pub fn verify_executed_values(
        &self,
        compiled: &CompiledBgp,
        layer: &impl VerifierLayer,
        trusted_resolution_root: ProofHash,
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
        let proven = self.verify_and_resolve_values(compiled, layer, trusted_resolution_root)?;
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
            layer.resolution_root(),
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
        if relation_attests_order(&compiled.relation.statement) {
            // ORDER-ATTESTING: the proof fixes the exact row order; the executor must deliver it in
            // that order. Compare positionally (no sort). There is no explicit equal-length check on
            // this nullable path, so guard the row count before the positional element comparison.
            if expected_rows.len() != delivered_rows.len() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "ordinary WOQL ordered nullable rows do not match the proven result row count",
                ));
            }
            if expected_rows != delivered_rows {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "ordinary WOQL ordered nullable rows do not match the proven result order",
                ));
            }
        } else {
            // UNORDERED (bag/set) relation: order is not attested, so compare as a multiset.
            expected_rows.sort();
            delivered_rows.sort();
            if expected_rows != delivered_rows {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "ordinary WOQL nullable rows do not match the proven result multiset",
                ));
            }
        }
        Ok(())
    }

    /// Encode a self-contained result envelope only after checking the proof, result IDs,
    /// and canonical lexical values against the caller-selected root.
    pub fn encode_envelope(
        &self,
        compiled: &CompiledBgp,
        layer: &LayerCommitment,
        trusted_resolution_root: ProofHash,
    ) -> std::io::Result<Vec<u8>> {
        let digest = compiled_query_digest(compiled)?;
        // β re-anchor: gate on the trusted `resolution_root`, then materialize the envelope against
        // the now-authenticated `commitment_root` (the store envelope's transcript/identity anchor,
        // unchanged) so encode and decode share one external anchor.
        let commitment_root = expect_resolution_root(layer, trusted_resolution_root)?;
        // `verify_relation` recursively checks the complete proof tree.  Keep that single
        // public trust boundary, then only materialize the already-checked tree below;
        // verifying every child again (and the finished envelope once more) made checked
        // encoding quadratic in the depth/number of recursive operators.
        self.verify_relation(&compiled.relation, layer, commitment_root, digest)?;
        let envelope = self.recursive_envelope_after_verification(
            &compiled.relation,
            layer,
            commitment_root,
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
            layer.resolution_root(),
            &edge.result_columns,
            &result_id_namespaces(edge_plan)?,
        )?;
        let edge_result_values = resolve_result_values(
            layer,
            layer.resolution_root(),
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
            layer.resolution_root(),
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
        // The ordered-list walk carries its whole store envelope WHOLESALE (its composite proof is
        // store-private and cannot be decomposed into the `ProvedQueryStatement` tree the way a
        // closure is), so — after `verify_relation` has already checked it against the trusted root
        // and statement digest — reconstruction just returns the carried envelope. Mirrors the
        // transitive closure's one-shot early return above.
        if let (
            ProvedQueryStatement::OrderedListWalk { envelope },
            CompiledRelationStatement::OrderedListWalk { .. },
        ) = (&self.statement, &compiled.statement)
        {
            return Ok((**envelope).clone());
        }
        // The ordered-array walk carries its whole store envelope WHOLESALE (its composite proof is
        // store-private and cannot be decomposed into the `ProvedQueryStatement` tree), so — after
        // `verify_relation` has already checked it against the trusted root and statement digest —
        // reconstruction just returns the carried envelope. Mirrors the ordered-list walk above.
        if let (
            ProvedQueryStatement::OrderedArrayWalk { envelope },
            CompiledRelationStatement::OrderedArrayWalk { .. },
        ) = (&self.statement, &compiled.statement)
        {
            return Ok((**envelope).clone());
        }
        // The all-pairs closure carries its whole store envelope WHOLESALE (its 2-column round/fixpoint
        // proof is carried opaquely, exactly like the ordered walks), so — after `verify_relation` has
        // already checked it against the trusted root and statement digest — reconstruction just returns
        // the carried envelope.
        if let (
            ProvedQueryStatement::TransitiveClosurePairs { envelope },
            CompiledRelationStatement::TransitiveClosurePairs { .. }
            | CompiledRelationStatement::TransitiveClosurePairsCompound { .. },
        ) = (&self.statement, &compiled.statement)
        {
            return Ok((**envelope).clone());
        }
        // The compound single-seed closure carries its whole store envelope WHOLESALE (its derived-edge
        // round/fixpoint proof is carried opaquely, exactly like the all-pairs closure), so — after
        // `verify_relation` has already checked it against the trusted root and statement digest —
        // reconstruction just returns the carried envelope.
        if let (
            ProvedQueryStatement::TransitiveClosureCompound { envelope },
            CompiledRelationStatement::TransitiveClosureCompound { .. },
        ) = (&self.statement, &compiled.statement)
        {
            return Ok((**envelope).clone());
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
                        layer.resolution_root(),
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
                layer.resolution_root(),
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

