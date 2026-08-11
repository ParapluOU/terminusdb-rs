//! SCHEMA-GRAPH FOUNDATION + WOQL `Subsumption` — end-to-end.
//!
//! Proves the two-graph foundation (a schema `LayerCommitment` committed by the SAME per-layer
//! machinery, combined with the instance root into a single `database_root`) and the `sys:inherits`
//! subsumption closure (`Subsumption child << parent` = reflexive-transitive `class_subsumed`).
//!
//! Every e2e path runs on a 128 MiB stack (deep envelope/closure recursion) and each layer holds
//! >= 2 real triples, per the harness requirements.

mod proof_common;
use proof_common::iri;

use std::collections::BTreeSet;
use std::sync::Arc;

use terminus_store::proof::commitment::{database_root, LayerCommitment};
use terminus_store::proof::CanonicalObject;
use terminus_store::{open_sync_memory_store, ValueTriple};

use terminusdb_woql2::compare::Subsumption;
use terminusdb_woql2::proof::{plan_bgp, BgpPlanError};
use terminusdb_woql2::query::Query;
use terminusdb_woql2::value::NodeValue;

/// The `sys:inherits` subclass edge IRI the compiler emits for `Subsumption`.
const SYS_INHERITS: &str = "http://terminusdb.com/schema/sys#inherits";

/// Build a SCHEMA-graph layer from `(subclass, superclass)` `sys:inherits` edges. This is an ordinary
/// terminus-store layer committed by the SAME `proof_commitment()` machinery as any instance layer —
/// the whole point of the foundation is that no new committed structure is needed for the schema graph.
fn build_schema_layer(edges: &[(&str, &str)]) -> Arc<LayerCommitment> {
    let store = open_sync_memory_store();
    let builder = store.create_base_layer().unwrap();
    for (sub, sup) in edges {
        builder
            .add_value_triple(ValueTriple::new_node(&iri(sub), SYS_INHERITS, &iri(sup)))
            .unwrap();
    }
    builder.commit().unwrap().proof_commitment().unwrap()
}

/// A plain INSTANCE-graph layer (unrelated triples) — present only so the combined `database_root`
/// has a real instance sub-root to bind. >= 2 triples.
fn build_instance_layer() -> Arc<LayerCommitment> {
    let store = open_sync_memory_store();
    let builder = store.create_base_layer().unwrap();
    builder
        .add_value_triple(ValueTriple::new_node(&iri("i1"), &iri("p"), &iri("i2")))
        .unwrap();
    builder
        .add_value_triple(ValueTriple::new_node(&iri("i2"), &iri("p"), &iri("i3")))
        .unwrap();
    builder.commit().unwrap().proof_commitment().unwrap()
}

fn subsumption(child: NodeValue, parent: NodeValue) -> Query {
    Query::Subsumption(Subsumption { child, parent })
}

fn class(name: &str) -> NodeValue {
    NodeValue::Node(iri(name))
}

fn var(name: &str) -> NodeValue {
    NodeValue::Variable(name.to_string())
}

/// The set of class IRIs (short names) a resolved closure delivered.
fn resolved_classes(objects: &[Vec<CanonicalObject>]) -> BTreeSet<String> {
    objects[0]
        .iter()
        .map(|object| match object {
            CanonicalObject::Node(bytes) => {
                let full = String::from_utf8(bytes.clone()).unwrap();
                full.strip_prefix("http://ex/").unwrap().to_string()
            }
            other => panic!("unexpected subsumption result object {other:?}"),
        })
        .collect()
}

fn names(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|s| s.to_string()).collect()
}

/// Run `body` on a 128 MiB stack (deep envelope/closure recursion).
fn on_big_stack(name: &str, body: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .name(name.into())
        .stack_size(128 * 1024 * 1024)
        .spawn(body)
        .unwrap()
        .join()
        .unwrap();
}

// ---------------------------------------------------------------------------------------------------
// PART A — the foundation.
// ---------------------------------------------------------------------------------------------------

#[test]
fn schema_layer_commits_and_database_root_combines_and_validates() {
    // The schema graph is committed as its OWN LayerCommitment (reused machinery) with its own root.
    let schema = build_schema_layer(&[("A", "B"), ("B", "C")]);
    let instance = build_instance_layer();
    let root_i = instance.state.commitment_root;
    let root_s = schema.state.commitment_root;

    // The two sub-roots are distinct and the schema layer round-trips through its own validation.
    assert_ne!(root_i, root_s, "instance and schema commitments must differ");
    schema.verifier_commitment().unwrap().validate().unwrap();
    instance.verifier_commitment().unwrap().validate().unwrap();

    // The combined database_root is the domain-separated two-hash of the sub-roots, order-fixed.
    let db_root = database_root(&root_i, &root_s);
    assert_eq!(db_root, database_root(&root_i, &root_s), "deterministic");
    assert_ne!(
        db_root,
        database_root(&root_s, &root_i),
        "instance/schema ordering is fixed and must not be transposable"
    );

    // The verifier validates BOTH commitments AND that they combine to the trusted database_root.
    terminus_store::proof::commitment::validate_database_root(&*instance, &*schema, &db_root).unwrap();
    // A wrong database_root is rejected.
    let mut tampered = db_root;
    tampered[0] ^= 1;
    assert!(
        terminus_store::proof::commitment::validate_database_root(&*instance, &*schema, &tampered)
            .is_err(),
        "a database_root that does not combine the two sub-roots must be rejected"
    );
    // Swapping which layer is the "schema" side is rejected against the true database_root.
    assert!(
        terminus_store::proof::commitment::validate_database_root(&*schema, &*instance, &db_root)
            .is_err(),
        "swapping instance/schema sub-commitments must be rejected"
    );
}

#[test]
fn instance_only_proof_is_byte_identical_regression() {
    // REGRESSION: an existing instance-only query must prove/verify EXACTLY as before the foundation.
    // Same query, proved twice, must yield byte-identical envelopes, and verify against the instance
    // root unchanged. (`plan_bgp`/`prove`/`verify` for an instance query touch no schema machinery.)
    on_big_stack("subsumption-instance-regression", || {
        let instance = build_instance_layer();
        let root = instance.state.commitment_root;

        // `t(?s, <p>, ?o)` — a bare instance BGP, the canonical pre-existing query shape.
        let query = Query::Triple(terminusdb_woql2::triple::Triple {
            subject: NodeValue::Variable("s".into()),
            predicate: NodeValue::Node(iri("p")),
            object: terminusdb_woql2::value::Value::Variable("o".into()),
            graph: None,
        });
        let compiled = plan_bgp(&query).unwrap();
        let proved_a = compiled.prove(&*instance).unwrap();
        let proved_b = compiled.prove(&*instance).unwrap();
        let bytes_a = proved_a.encode_envelope(&compiled, &*instance, root).unwrap();
        let bytes_b = proved_b.encode_envelope(&compiled, &*instance, root).unwrap();
        assert_eq!(bytes_a, bytes_b, "instance proof must be byte-identical across runs");
        // Verifies against the instance root exactly as today.
        proved_a.verify(&compiled, &*instance, root).unwrap();

        // And the combined-graph entry, with a schema layer added, still anchors an INSTANCE query to
        // the instance root — the schema commitment is purely additive.
        let schema = build_schema_layer(&[("A", "B"), ("B", "C")]);
        let db_root = database_root(&root, &schema.state.commitment_root);
        proved_a
            .verify_over_graphs(&compiled, &*instance, &*schema, db_root)
            .unwrap();
    });
}

// ---------------------------------------------------------------------------------------------------
// PART B — Subsumption.
// ---------------------------------------------------------------------------------------------------

#[test]
fn subsumption_hierarchy_diamond_and_directions_e2e() {
    on_big_stack("subsumption-hierarchy", || {
        // A -> B -> C plus a diamond A -> {B, C} -> D.
        //   supers(A) (reflexive) = {A, B, C, D}; supers(B) = {B, D}; supers(C) = {C, D}.
        //   subs(D)   (reflexive) = {A, B, C, D}; subs(B) = {A, B}.
        let schema =
            build_schema_layer(&[("A", "B"), ("B", "C"), ("A", "Cc"), ("Cc", "D"), ("C", "D")]);
        let instance = build_instance_layer();
        let root_s = schema.state.commitment_root;
        let db_root = database_root(&instance.state.commitment_root, &root_s);

        // Helper: prove a Subsumption query over the SCHEMA commitment, verify through the combined
        // database_root path, and return the delivered class set.
        let run = |q: &Query| -> BTreeSet<String> {
            let compiled = plan_bgp(q).unwrap();
            let proved = compiled.prove(&*schema).unwrap();
            let objects = proved
                .verify_over_graphs_and_resolve(&compiled, &*instance, &*schema, db_root)
                .unwrap();
            resolved_classes(&objects)
        };

        // `A << ?y` — supers of A (reflexive-transitive), incl. the diamond join D.
        assert_eq!(
            run(&subsumption(class("A"), var("y"))),
            names(&["A", "B", "C", "Cc", "D"]),
            "A << ?y delivers the reflexive-transitive supers of A"
        );
        // `?x << D` — subclasses of D (inverted closure), the whole diamond below D.
        assert_eq!(
            run(&subsumption(var("x"), class("D"))),
            names(&["A", "B", "C", "Cc", "D"]),
            "?x << D delivers the subclasses of D (reflexive)"
        );
        // `B << ?y` — supers of B only.
        assert_eq!(
            run(&subsumption(class("B"), var("y"))),
            names(&["B", "C", "D"]),
            "B << ?y delivers the supers of B"
        );
        // `?x << B` — subclasses of B only.
        assert_eq!(
            run(&subsumption(var("x"), class("B"))),
            names(&["A", "B"]),
            "?x << B delivers the subclasses of B"
        );
    });
}

#[test]
fn subsumption_both_bound_membership_gate_e2e() {
    on_big_stack("subsumption-both-bound", || {
        let schema = build_schema_layer(&[("A", "B"), ("B", "C")]);
        let instance = build_instance_layer();
        let db_root = database_root(&instance.state.commitment_root, &schema.state.commitment_root);

        let holds = |child: &str, parent: &str| -> bool {
            let q = subsumption(class(child), class(parent));
            let compiled = plan_bgp(&q).unwrap();
            let proved = compiled.prove(&*schema).unwrap();
            // Both endpoints are constants → a ground existence: verify succeeds either way; the
            // delivered row COUNT is the truth value (>=1 = holds, 0 = does not).
            proved
                .verify_over_graphs(&compiled, &*instance, &*schema, db_root)
                .unwrap();
            proved.result_len >= 1
        };

        assert!(holds("A", "C"), "A << C is true (A -> B -> C)");
        assert!(holds("A", "B"), "A << B is true");
        assert!(holds("A", "A"), "A << A is true (reflexive)");
        assert!(!holds("C", "A"), "C << A is false (not in the closure of A)");
        assert!(!holds("B", "A"), "B << A is false");
    });
}

// ---------------------------------------------------------------------------------------------------
// SOUNDNESS NEGATIVES.
// ---------------------------------------------------------------------------------------------------

#[test]
fn subsumption_wrong_graph_and_tamper_reject() {
    on_big_stack("subsumption-soundness", || {
        let schema = build_schema_layer(&[("A", "B"), ("B", "C")]);
        let instance = build_instance_layer();
        let root_s = schema.state.commitment_root;
        let db_root = database_root(&instance.state.commitment_root, &root_s);

        let query = subsumption(class("A"), var("y"));
        let compiled = plan_bgp(&query).unwrap();

        // Honest schema proof verifies through the combined path.
        let proved = compiled.prove(&*schema).unwrap();
        proved
            .verify_over_graphs(&compiled, &*instance, &*schema, db_root)
            .unwrap();

        // WRONG GRAPH (prover side): a proof built over the INSTANCE layer for a SCHEMA query must not
        // verify. Two sound outcomes: proving fails closed (the seed class / sys:inherits predicate is
        // absent from the instance dictionary), OR a proof is produced but the verifier — which
        // reconstructs graph=Schema from the trusted query and anchors to the schema root — rejects it.
        match compiled.prove(&*instance) {
            Err(_) => { /* proving over the wrong graph already fails closed */ }
            Ok(wrong) => assert!(
                wrong
                    .verify_over_graphs(&compiled, &*instance, &*schema, db_root)
                    .is_err(),
                "a schema query proved over the instance layer must be rejected by the verifier"
            ),
        }

        // WRONG GRAPH (verifier anchor): even the low-level `verify` against the INSTANCE view + root
        // must reject an honest schema proof (its columns are anchored to the schema root).
        assert!(
            proved
                .verify(&compiled, &*instance, instance.state.commitment_root)
                .is_err(),
            "a schema proof must not verify against the instance commitment"
        );

        // TAMPERED database_root: the combined anchor no longer matches the two sub-roots.
        let mut bad_db = db_root;
        bad_db[0] ^= 1;
        assert!(
            proved
                .verify_over_graphs(&compiled, &*instance, &*schema, bad_db)
                .is_err(),
            "a tampered database_root must be rejected"
        );

        // SPURIOUS SUPER: a schema with an injected extra edge has a DIFFERENT commitment root, so it
        // fails to combine into the trusted database_root — the tamper cannot slip a forged superclass
        // past the anchor. (And an honest proof over the injected schema delivers the wrong set only
        // against its own, untrusted root.)
        let spurious = build_schema_layer(&[("A", "B"), ("B", "C"), ("A", "Evil")]);
        assert_ne!(
            spurious.state.commitment_root, root_s,
            "injecting a spurious super changes the schema commitment root"
        );
        assert!(
            terminus_store::proof::commitment::validate_database_root(
                &*instance,
                &*spurious,
                &db_root
            )
            .is_err(),
            "a schema with a spurious super cannot combine into the trusted database_root"
        );
    });
}

/// LOW-LEVEL VERIFY GUARD (audit follow-up — the coverage the schema-graph audit found missing).
///
/// The prove side already guards this (`prove_on_layer` rejects a schema query, forcing schema proofs
/// through `prove_on_layers`). The single-root VERIFY entries — `verify`, `verify_and_resolve`,
/// `verify_and_resolve_values` — must carry the MIRROR guard: they trust a caller-supplied
/// `expected_root` with no tie to the trusted combined `database_root`, so a schema query on any of
/// them must reject closed and be routed to `verify_over_graphs`.
///
/// Concrete attack this blocks: a malicious prover FABRICATES a schema (the real `A -> B -> C` plus a
/// forged `A sys:inherits Evil` edge), commits it → `schema'_root`, honestly proves the closure over
/// that fabricated view, and hands a verifier `(compiled = "A << ?y", proof over schema',
/// schema' view, schema'_root)`. Without the guard the low-level `verify` ACCEPTS and the fabricated
/// `Evil ∈ supers(A)` is never tied to the trusted `database_root`. With the guard, all three
/// low-level entries reject with the schema-query error — the query MUST go through the combined-root
/// `verify_over_graphs`, which cannot bind the fabricated schema to a trusted out-of-band anchor.
#[test]
fn low_level_verify_entries_reject_schema_query() {
    on_big_stack("subsumption-low-level-guard", || {
        // FABRICATED schema: the real hierarchy PLUS a forged `A << Evil` superclass edge.
        let fabricated = build_schema_layer(&[("A", "B"), ("B", "C"), ("A", "Evil")]);
        let fab_root = fabricated.state.commitment_root;

        // `A << ?y` — supers of A; over the fabricated schema this closure INCLUDES the forged `Evil`.
        let query = subsumption(class("A"), var("y"));
        let compiled = plan_bgp(&query).unwrap();

        // The prover honestly proves the closure over its OWN fabricated schema view.
        let proved = compiled.prove(&*fabricated).unwrap();

        // The error is the GUARD (its message names the schema graph), not a downstream root/commitment
        // mismatch — the guard fires at the TOP, before any verification work touches the root.
        let is_schema_guard = |err: &std::io::Error| {
            assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput, "guard kind");
            assert!(
                err.to_string().contains("schema graph")
                    && err.to_string().contains("verify_over_graphs"),
                "guard message must route schema queries to verify_over_graphs, got: {err}"
            );
        };

        // (1) `verify` against the fabricated view + its matching root: guard rejects.
        let e = proved
            .verify(&compiled, &*fabricated, fab_root)
            .expect_err("low-level verify must reject a schema query");
        is_schema_guard(&e);

        // (2) `verify_and_resolve` (object resolution) against the same fabricated view/root: guard.
        let e = proved
            .verify_and_resolve(&compiled, &*fabricated, fab_root)
            .expect_err("low-level verify_and_resolve must reject a schema query");
        is_schema_guard(&e);

        // (3) `verify_and_resolve_values` (value resolution): guard.
        let e = proved
            .verify_and_resolve_values(&compiled, &*fabricated, fab_root)
            .expect_err("low-level verify_and_resolve_values must reject a schema query");
        is_schema_guard(&e);

        // The guard is ROOT-AGNOSTIC: it fires before the root is consulted, so it also rejects the
        // fabricated view under an UNRELATED honest root with the SAME guard error (not a root
        // mismatch) — the whole point is that no single caller-supplied root can gate a schema query.
        let honest = build_schema_layer(&[("A", "B"), ("B", "C")]);
        let e = proved
            .verify(&compiled, &*fabricated, honest.state.commitment_root)
            .expect_err("guard must reject regardless of which single root is supplied");
        is_schema_guard(&e);
    });
}

// ---------------------------------------------------------------------------------------------------
// PLAN-LEVEL REJECT-CLOSED discipline.
// ---------------------------------------------------------------------------------------------------

#[test]
fn subsumption_both_free_rejects_closed() {
    // Two free class endpoints = a whole-schema pairs enumeration (deferred).
    let q = subsumption(var("x"), var("y"));
    assert!(plan_bgp(&q).is_err(), "both-free subsumption must reject closed");
}

#[test]
fn isa_bound_class_lowers_variable_class_and_schema_from_reject() {
    // BOUND class (node regime): a standalone `?x isa A` NOW LOWERS to a cross-dictionary two-root
    // hash join (the downward sys:inherits closure of A over the schema graph, hash-joined against the
    // instance rdf:type relation via a full-field LogUp semi-join). So it PLANS successfully.
    let isa = Query::IsA(terminusdb_woql2::compare::IsA {
        element: NodeValue::Variable("x".into()),
        type_of: class("A"),
    });
    plan_bgp(&isa).expect("standalone bound-class IsA lowers to a cross-root plan");

    // VARIABLE class: rejects on the free class with its own distinct reason (the downward closure
    // needs the class bound to seed it).
    let isa_free_class = Query::IsA(terminusdb_woql2::compare::IsA {
        element: NodeValue::Variable("x".into()),
        type_of: NodeValue::Variable("C".into()),
    });
    assert_eq!(
        plan_bgp(&isa_free_class).unwrap_err(),
        BgpPlanError::IsAVariableClass,
        "variable-class IsA rejects because the closure has no bound seed class"
    );

    let schema_from = Query::From(terminusdb_woql2::control::From {
        graph: "schema".to_string(),
        query: Box::new(Query::Triple(terminusdb_woql2::triple::Triple {
            subject: NodeValue::Variable("s".into()),
            predicate: NodeValue::Node(iri("p")),
            object: terminusdb_woql2::value::Value::Variable("o".into()),
            graph: None,
        })),
    });
    assert!(
        plan_bgp(&schema_from).is_err(),
        "a bare schema-graph From (non-subsumption read) still rejects closed"
    );
}

// ---------------------------------------------------------------------------------------------------
// PART C — IsA end-to-end (cross-dictionary two-root hash join via the full-field LogUp semi-join).
// ---------------------------------------------------------------------------------------------------

const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";

/// An INSTANCE-graph layer of `(subject, class)` `rdf:type` triples.
fn build_typed_instance_layer(types: &[(&str, &str)]) -> Arc<LayerCommitment> {
    let store = open_sync_memory_store();
    let builder = store.create_base_layer().unwrap();
    for (subject, class) in types {
        builder
            .add_value_triple(ValueTriple::new_node(&iri(subject), RDF_TYPE, &iri(class)))
            .unwrap();
    }
    builder.commit().unwrap().proof_commitment().unwrap()
}

fn isa(element: &str, class_name: &str) -> Query {
    Query::IsA(terminusdb_woql2::compare::IsA {
        element: NodeValue::Variable(element.into()),
        type_of: class(class_name),
    })
}

/// The set of instance `?x` short-names an IsA proof delivered. `resolve_typed_result_objects` is
/// COLUMN-major (`objects[col]` is a column of per-row values); IsA delivers a single `?x` column.
fn resolved_instances(objects: &[Vec<CanonicalObject>]) -> BTreeSet<String> {
    objects
        .first()
        .map(|column| {
            column
                .iter()
                .map(|object| match object {
                    CanonicalObject::Node(bytes) => String::from_utf8(bytes.clone())
                        .unwrap()
                        .strip_prefix("http://ex/")
                        .unwrap()
                        .to_string(),
                    other => panic!("unexpected IsA result object {other:?}"),
                })
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn isa_node_regime_cross_root_e2e() {
    on_big_stack("isa-cross-root", || {
        // Schema: A -inherits-> B (A is a subclass of B); C -inherits-> D is unrelated filler.
        //   downward closure of B = {B, A}; downward closure of A = {A}.
        let schema = build_schema_layer(&[("A", "B"), ("C", "D")]);
        // Instance: x1:A, x2:B, x3:Other (a class OUTSIDE B's closure).
        let instance = build_typed_instance_layer(&[("x1", "A"), ("x2", "B"), ("x3", "Other")]);
        let db_root = database_root(&instance.state.commitment_root, &schema.state.commitment_root);

        let run = |q: &Query| -> BTreeSet<String> {
            let compiled = plan_bgp(q).unwrap();
            let proved = compiled.prove_cross_root_layers(&*instance, &*schema).unwrap();
            let objects = proved
                .verify_over_graphs_and_resolve(&compiled, &*instance, &*schema, db_root)
                .unwrap();
            resolved_instances(&objects)
        };

        // `?x isa B` = every x whose direct type is subsumed by B = {x1 (A<:B), x2 (B)}; EXCLUDES x3
        // (Other is not in B's downward closure). This is the crux: x3's class "Other" is a DIFFERENT
        // IRI (hence a different hash), so the full-field semi-join excludes it.
        assert_eq!(
            run(&isa("x", "B")),
            names(&["x1", "x2"]),
            "?x isa B = {{x1, x2}}; x3 (class Other) is excluded"
        );
        // `?x isa A` = {x1} only (A has no subclasses; x2:B and x3:Other are excluded).
        assert_eq!(run(&isa("x", "A")), names(&["x1"]), "?x isa A = {{x1}}");
        // A class with NO instances subsumed: `?x isa D` — closure {D, C}, but no instance has type
        // C or D, so the result is empty.
        assert!(run(&isa("x", "D")).is_empty(), "?x isa D has no matching instances");
    });
}

#[test]
fn isa_cross_root_rejects_wrong_root_and_swapped_graphs() {
    on_big_stack("isa-cross-root-negatives", || {
        let schema = build_schema_layer(&[("A", "B"), ("C", "D")]);
        let instance = build_typed_instance_layer(&[("x1", "A"), ("x2", "B")]);
        let db_root = database_root(&instance.state.commitment_root, &schema.state.commitment_root);
        let compiled = plan_bgp(&isa("x", "B")).unwrap();
        let proved = compiled.prove_cross_root_layers(&*instance, &*schema).unwrap();

        // Correct verification succeeds.
        proved
            .verify_over_graphs_and_resolve(&compiled, &*instance, &*schema, db_root)
            .unwrap();
        // A tampered database_root rejects.
        let mut bad = db_root;
        bad[0] ^= 1;
        assert!(
            proved
                .verify_over_graphs_and_resolve(&compiled, &*instance, &*schema, bad)
                .is_err(),
            "a tampered database_root must reject"
        );
        // Swapping the instance and schema graphs rejects (the ordered two-hash database_root no longer
        // matches, and each child is anchored to the wrong root).
        assert!(
            proved
                .verify_over_graphs_and_resolve(&compiled, &*schema, &*instance, db_root)
                .is_err(),
            "swapping the instance/schema graphs must reject"
        );
    });
}
