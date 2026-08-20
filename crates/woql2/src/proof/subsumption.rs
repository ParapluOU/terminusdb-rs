//! Schema-graph lowering — Subsumption and `IsA`.
//!
//! Lowers the two constructs that read the committed SCHEMA graph rather than the instance
//! graph: [`plan_subsumption`] (`class_subsumed` — the reflexive-transitive closure of
//! `sys:inherits` subclass edges) and [`plan_isa`] (an instance's `rdf:type` composed with that
//! subsumption closure). Both emit schema-graph transitive-closure plans over the authenticated
//! `sys:inherits` / `rdf:type` edges.

use super::*;

/// The `sys:inherits` subclass edge IRI (schema.pl:223-235,591). An edge `(s, sys:inherits, o)` in
/// the SCHEMA graph records that class `s` DIRECTLY inherits class `o` (subject = subclass, object =
/// superclass). `class_subsumed(A,B)` = `B ∈ {A} ∪ transitive-supers(A)` is the REFLEXIVE-TRANSITIVE
/// closure of these edges from `A` forward.
pub(crate) const SYS_INHERITS_IRI: &str = "http://terminusdb.com/schema/sys#inherits";

/// One resolved endpoint of a `Subsumption`: a bound class IRI or a free variable.
pub(crate) enum SubsumptionEndpoint {
    Class(String),
    Variable(String),
}

pub(crate) fn subsumption_endpoint(value: &crate::value::NodeValue) -> Result<SubsumptionEndpoint, BgpPlanError> {
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
pub(crate) fn plan_subsumption(
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
pub(crate) fn sys_inherits_edge() -> LayerBgpPlan {
    LayerBgpPlan {
        source: ColumnSource::Materialized,
        atoms: vec![LayerBgpAtom::all_variables(SYS_INHERITS_IRI.as_bytes().to_vec())],
        joins: Vec::new(),
    }
}

/// The `rdf:type` predicate IRI the instance `rdf:type` edge reads.
pub(crate) const RDF_TYPE_IRI: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";

/// The INSTANCE-graph `Triple(?x, rdf:type, ?class)` edge — a single-atom 2-column node BGP delivering
/// `[?x, class]` (subject var, `rdf:type` predicate, object var).
pub(crate) fn rdf_type_edge() -> LayerBgpPlan {
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
pub(crate) fn plan_isa(
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
pub(crate) fn build_subsumption_closure(
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
pub(crate) fn build_subsumption_membership_gate(
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

