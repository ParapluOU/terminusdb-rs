//! The predicate dispatch index: which live predicates can a change affect?
//!
//! Evaluating every live predicate against every change would make the feature
//! cost `O(predicates × changes)` per commit, which is the shape of cost that
//! killed the prototype's naive invalidation. So predicates are filed under the
//! `(class, property)` pairs they mention, and a change evaluates only the
//! predicates that mention a property which actually moved. That dispatch is
//! the point of this module; the evaluation itself is in
//! [`crate::predicate`].
//!
//! # Lifetime
//!
//! A predicate is live exactly while some registered fragment version depends
//! on it, so entries are reference counted by registration and dropped when the
//! last referrer goes. The counting happens in the single writer, in the same
//! places the term index is updated ([`crate::engine::NamespaceWriter`]), which
//! is what keeps the two from drifting: a predicate the registry has forgotten
//! but a fragment still depends on would be a fragment that never invalidates.
//!
//! # Three cases where dispatch must be wider than "the property moved"
//!
//! 1. **Creates and deletes.** A created document's `changed_props` only lists
//!    the properties it *has*, so a predicate like `_not: {rank: {eq: 1}}`
//!    — which a document with no `rank` satisfies — would never be evaluated.
//!    Both are therefore evaluated against every predicate on the class.
//! 2. **Retypes.** A changed `@type` moves a document between classes, so
//!    membership can move for predicates on classes we cannot enumerate from
//!    the change alone. Handled by going coarse for that change.
//! 3. **Undecidable values.** A document holding something the evaluator's
//!    measured rules do not cover (see [`crate::predicate::Truth::Unknown`])
//!    fires that predicate coarsely rather than guessing a membership.

use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use rayon::prelude::*;

use crate::change::ChangeSet;
use crate::ids::{ClassName, PropName};
use crate::predicate::{Predicate, PredicateId, Truth};
use crate::schema_index::SchemaIndex;
use crate::term::{Term, TermSet};

/// One live predicate and how many registered fragment versions depend on it.
#[derive(Debug)]
struct Registered {
    pred: Arc<Predicate>,
    refs: usize,
}

/// `(class, property) → predicates`, reference counted.
#[derive(Debug, Default)]
pub struct PredicateRegistry {
    by_id: HashMap<PredicateId, Registered>,
    by_class_prop: HashMap<(ClassName, PropName), BTreeSet<PredicateId>>,
    by_class: HashMap<ClassName, BTreeSet<PredicateId>>,
}

impl PredicateRegistry {
    /// An empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Take a reference to every predicate named in `deps`.
    pub fn retain(&mut self, deps: &[Term]) {
        for dep in deps {
            if let Term::Predicate(pred) = dep {
                self.retain_one(pred);
            }
        }
    }

    /// Drop a reference to every predicate named in `deps`.
    pub fn release(&mut self, deps: &[Term]) {
        for dep in deps {
            if let Term::Predicate(pred) = dep {
                self.release_one(&pred.id());
            }
        }
    }

    fn retain_one(&mut self, pred: &Arc<Predicate>) {
        let id = pred.id();
        if let Some(entry) = self.by_id.get_mut(&id) {
            entry.refs += 1;
            return;
        }
        for prop in pred.props() {
            self.by_class_prop
                .entry((pred.class.clone(), prop))
                .or_default()
                .insert(id);
        }
        self.by_class
            .entry(pred.class.clone())
            .or_default()
            .insert(id);
        self.by_id.insert(
            id,
            Registered {
                pred: Arc::clone(pred),
                refs: 1,
            },
        );
    }

    fn release_one(&mut self, id: &PredicateId) {
        let Some(entry) = self.by_id.get_mut(id) else {
            return;
        };
        entry.refs -= 1;
        if entry.refs > 0 {
            return;
        }
        let Some(entry) = self.by_id.remove(id) else {
            return;
        };
        for prop in entry.pred.props() {
            let key = (entry.pred.class.clone(), prop);
            if let Some(set) = self.by_class_prop.get_mut(&key) {
                set.remove(id);
                if set.is_empty() {
                    self.by_class_prop.remove(&key);
                }
            }
        }
        if let Some(set) = self.by_class.get_mut(&entry.pred.class) {
            set.remove(id);
            if set.is_empty() {
                self.by_class.remove(&entry.pred.class);
            }
        }
    }

    /// Forget everything. Used when a history rewrite leaves us unable to say
    /// what is still valid; the fragments that referred to these predicates are
    /// dropped in the same breath.
    pub fn clear(&mut self) {
        self.by_id.clear();
        self.by_class_prop.clear();
        self.by_class.clear();
    }

    /// How many distinct predicates are live.
    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    /// True when nothing is registered.
    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }

    /// How many registrations refer to `pred`, for tests and `/explain`.
    pub fn refs(&self, id: &PredicateId) -> usize {
        self.by_id.get(id).map(|entry| entry.refs).unwrap_or(0)
    }

    /// Every live predicate on any of `classes`.
    pub fn all_on(&self, classes: &[ClassName]) -> Vec<Arc<Predicate>> {
        let mut ids: BTreeSet<PredicateId> = BTreeSet::new();
        for class in classes {
            if let Some(set) = self.by_class.get(class) {
                ids.extend(set.iter().copied());
            }
        }
        self.resolve(ids)
    }

    /// Live predicates on any of `classes` that mention any of `props`.
    pub fn mentioning(&self, classes: &[ClassName], props: &[PropName]) -> Vec<Arc<Predicate>> {
        let mut ids: BTreeSet<PredicateId> = BTreeSet::new();
        for class in classes {
            for prop in props {
                if let Some(set) = self.by_class_prop.get(&(class.clone(), prop.clone())) {
                    ids.extend(set.iter().copied());
                }
            }
        }
        self.resolve(ids)
    }

    fn resolve(&self, ids: BTreeSet<PredicateId>) -> Vec<Arc<Predicate>> {
        ids.into_iter()
            .filter_map(|id| self.by_id.get(&id).map(|entry| Arc::clone(&entry.pred)))
            .collect()
    }
}

/// Bounds and knobs for one commit's predicate evaluation.
#[derive(Clone, Copy, Debug)]
pub struct EvalLimits {
    /// Most `(change, predicate)` pairs we will evaluate for one commit. Over
    /// this, the commit invalidates every predicate on the classes it touched
    /// instead — coarse, but bounded.
    pub max_evaluations: usize,
    /// Most predicates we will keep a dispatch index for. Over this we stop
    /// trusting the index to be cheap and go coarse.
    pub max_predicates: usize,
    /// Threads to evaluate on.
    pub workers: usize,
    /// Fault injection: sleep this long inside each evaluation. Used by the
    /// watermark-gating test to make the lag observable; `0` in production.
    pub delay_ms: u64,
}

impl Default for EvalLimits {
    fn default() -> Self {
        Self {
            max_evaluations: 50_000,
            max_predicates: 5_000,
            workers: 0,
            delay_ms: 0,
        }
    }
}

/// What one commit's predicate evaluation produced.
#[derive(Debug, Default)]
pub struct Firing {
    /// The `Predicate` terms to union into the commit's fired set.
    pub terms: TermSet,
    /// `(change, predicate)` pairs actually evaluated.
    pub evaluated: u64,
    /// Pairs we declined to evaluate, having gone coarse instead. A rising
    /// count is the signal that precision is being traded for latency — which
    /// is the right trade, but the operator should know it is happening.
    pub skipped: u64,
    /// True when some part of this commit fell back to coarse firing.
    pub coarse: bool,
}

/// The shared evaluation pool.
///
/// Process-wide rather than per-namespace: it is a CPU pool, and sizing it per
/// namespace would oversubscribe a node tracking several. Built once, on first
/// use, from the first namespace's configured worker count.
fn pool(workers: usize) -> Option<&'static rayon::ThreadPool> {
    static POOL: std::sync::OnceLock<Option<rayon::ThreadPool>> = std::sync::OnceLock::new();
    POOL.get_or_init(|| {
        let mut builder = rayon::ThreadPoolBuilder::new().thread_name(|i| format!("rx-pred-{i}"));
        if workers > 0 {
            builder = builder.num_threads(workers);
        }
        match builder.build() {
            Ok(pool) => Some(pool),
            Err(err) => {
                // A node that cannot spawn threads still has to be correct, so
                // evaluation falls back to the calling thread rather than the
                // commit failing.
                tracing::warn!(%err, "no predicate evaluation pool: evaluating inline");
                None
            }
        }
    })
    .as_ref()
}

/// Decide which predicates this commit moved documents into or out of.
///
/// Returns the `Predicate` terms to fire. The caller unions them into the
/// commit's fired set **before** publishing the watermark: evaluation may be
/// slow, and a watermark that advanced past an unapplied invalidation would tell
/// a reader it is current while we serve entries that commit should have killed.
/// Slow indexing must mean the watermark *lags*, never that it is wrong.
pub fn fire_predicates(
    cs: &ChangeSet,
    schema: &SchemaIndex,
    registry: &PredicateRegistry,
    limits: EvalLimits,
) -> Firing {
    let mut out = Firing::default();
    if registry.is_empty() {
        return out;
    }

    // Per change: the classes it could match predicates on, and whether we
    // have to consider every predicate on them rather than only the ones
    // mentioning a moved property.
    struct Work<'a> {
        change: &'a crate::change::DocChange,
        classes: Vec<ClassName>,
        preds: Vec<Arc<Predicate>>,
    }

    let mut work: Vec<Work<'_>> = Vec::with_capacity(cs.changes.len());
    let mut pairs = 0usize;
    let mut coarse_classes: Vec<ClassName> = Vec::new();

    for change in &cs.changes {
        let classes = schema.ancestors(&change.class);
        let changed = change.changed_props();
        // A retype cannot be attributed: the document may have left classes we
        // cannot name from this change. Coarse, and rare.
        let retyped = changed.iter().any(|prop| prop.as_str() == "@type");
        let structural = change.is_create() || change.is_delete();

        if retyped {
            coarse_classes.extend(classes.iter().cloned());
            continue;
        }
        let preds = if structural {
            registry.all_on(&classes)
        } else {
            registry.mentioning(&classes, &changed)
        };
        if preds.is_empty() {
            continue;
        }
        pairs += preds.len();
        work.push(Work {
            change,
            classes,
            preds,
        });
    }

    // Backpressure degrades *precision*, not latency: an unbounded evaluation
    // would stall the watermark, which stalls every read. Over the bound we
    // invalidate every predicate on the touched classes — the same blast radius
    // the old `Class(C)` fallback had — and count it.
    let over_bound = pairs > limits.max_evaluations || registry.len() > limits.max_predicates;
    if over_bound {
        for item in &work {
            out.skipped += item.preds.len() as u64;
            coarse_classes.extend(item.classes.iter().cloned());
        }
        work.clear();
    }

    if !coarse_classes.is_empty() {
        coarse_classes.sort();
        coarse_classes.dedup();
        for pred in registry.all_on(&coarse_classes) {
            out.terms.insert(Term::Predicate(pred));
        }
        out.coarse = true;
    }

    if work.is_empty() {
        return out;
    }

    // Flatten to one task per (change, predicate) pair so the pool can balance
    // across them: one document with a thousand live predicates on its class is
    // as plausible as a thousand documents with one.
    let tasks: Vec<(&Work<'_>, &Arc<Predicate>)> = work
        .iter()
        .flat_map(|item| item.preds.iter().map(move |pred| (item, pred)))
        .collect();
    out.evaluated = tasks.len() as u64;

    let decide = |(item, pred): &(&Work<'_>, &Arc<Predicate>)| -> Option<Decision> {
        if limits.delay_ms > 0 {
            std::thread::sleep(std::time::Duration::from_millis(limits.delay_ms));
        }
        let before = pred.member(item.change.before.as_ref());
        let after = pred.member(item.change.after.as_ref());
        match (before, after) {
            // The honest answer when the document holds something our measured
            // rules do not cover. Fire it: an over-fire costs a refill, and the
            // alternative is a guess at membership.
            (Truth::Unknown, _) | (_, Truth::Unknown) => {
                Some(Decision::Undecidable(Arc::clone(pred)))
            }
            (a, b) if a != b => Some(Decision::Moved(Arc::clone(pred))),
            _ => None,
        }
    };

    let decisions: Vec<Decision> = match pool(limits.workers) {
        Some(pool) => pool.install(|| tasks.par_iter().filter_map(decide).collect()),
        None => tasks.iter().filter_map(decide).collect(),
    };

    for decision in decisions {
        match decision {
            Decision::Moved(pred) => {
                out.terms.insert(Term::Predicate(pred));
            }
            Decision::Undecidable(pred) => {
                out.coarse = true;
                out.terms.insert(Term::Predicate(pred));
            }
        }
    }
    out
}

enum Decision {
    /// Membership genuinely moved: the precise case.
    Moved(Arc<Predicate>),
    /// We could not decide, so we fire anyway.
    Undecidable(Arc<Predicate>),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::change::DocChange;
    use crate::predicate::{CmpOp, Lit, Order, Pred};
    use crate::testutil::{cls, doc, prop, schema_fixture};
    use serde_json::json;
    use terminusdb_client::CommitId;

    fn age_at_least(n: &str) -> Arc<Predicate> {
        Arc::new(Predicate::new(
            cls("Person"),
            Pred::Cmp {
                prop: prop("age"),
                op: CmpOp::Ge,
                value: Lit::Num(n.to_owned()),
                order: Order::Integer,
            },
        ))
    }

    fn named(name: &str) -> Arc<Predicate> {
        Arc::new(Predicate::new(
            cls("Person"),
            Pred::Cmp {
                prop: prop("name"),
                op: CmpOp::Eq,
                value: Lit::Text(name.to_owned()),
                order: Order::Text,
            },
        ))
    }

    fn changeset(changes: Vec<DocChange>) -> ChangeSet {
        ChangeSet {
            ns: crate::ids::Namespace::new("admin", "db", "main"),
            commit: CommitId::new("c1"),
            parent: None,
            changes,
            schema_changed: false,
            complete: true,
        }
    }

    fn registry_with(preds: &[Arc<Predicate>]) -> PredicateRegistry {
        let mut registry = PredicateRegistry::new();
        let deps: Vec<Term> = preds
            .iter()
            .map(|p| Term::Predicate(Arc::clone(p)))
            .collect();
        registry.retain(&deps);
        registry
    }

    // ------------------------------------------------------------- refcounting

    #[test]
    fn a_predicate_lives_while_a_fragment_depends_on_it() {
        let pred = age_at_least("30");
        let deps = vec![Term::Predicate(Arc::clone(&pred))];
        let mut registry = PredicateRegistry::new();

        registry.retain(&deps);
        registry.retain(&deps);
        assert_eq!(registry.len(), 1, "one predicate, two referrers");
        assert_eq!(registry.refs(&pred.id()), 2);

        registry.release(&deps);
        assert_eq!(registry.len(), 1, "still referred to");
        registry.release(&deps);
        assert!(registry.is_empty(), "the last referrer went");
        // And the dispatch index went with it, or a later commit would look up
        // a predicate nothing depends on.
        assert!(registry
            .mentioning(&[cls("Person")], &[prop("age")])
            .is_empty());
    }

    #[test]
    fn releasing_something_never_registered_is_harmless() {
        let mut registry = PredicateRegistry::new();
        registry.release(&[Term::Predicate(age_at_least("30"))]);
        assert!(registry.is_empty());
    }

    // ---------------------------------------------------------------- dispatch

    #[test]
    fn dispatch_is_by_class_and_property() {
        let registry = registry_with(&[age_at_least("30"), named("Ann")]);
        assert_eq!(
            registry.mentioning(&[cls("Person")], &[prop("age")]).len(),
            1,
            "only the predicate mentioning `age`"
        );
        assert_eq!(
            registry
                .mentioning(&[cls("Person")], &[prop("height")])
                .len(),
            0,
            "a property no predicate mentions dispatches to nothing"
        );
        assert_eq!(registry.all_on(&[cls("Person")]).len(), 2);
        assert_eq!(registry.all_on(&[cls("Company")]).len(), 0);
    }

    // -------------------------------------------------------------- the fires

    #[test]
    fn a_field_change_that_does_not_cross_the_boundary_fires_nothing() {
        let schema = schema_fixture();
        let registry = registry_with(&[age_at_least("30")]);
        let cs = changeset(vec![DocChange::updated(
            doc("Person/alice"),
            cls("Person"),
            json!({"name": "Ann", "age": 40}),
            json!({"name": "Ann", "age": 50}),
        )]);
        let firing = fire_predicates(&cs, &schema, &registry, EvalLimits::default());
        assert!(
            firing.terms.is_empty(),
            "40 and 50 are both >= 30: {:?}",
            firing.terms
        );
        assert_eq!(firing.evaluated, 1, "but it was considered");
        assert!(!firing.coarse);
    }

    #[test]
    fn a_field_change_that_crosses_the_boundary_fires() {
        let schema = schema_fixture();
        let pred = age_at_least("30");
        let registry = registry_with(&[Arc::clone(&pred)]);
        for (before, after) in [(29, 31), (31, 29)] {
            let cs = changeset(vec![DocChange::updated(
                doc("Person/alice"),
                cls("Person"),
                json!({"age": before}),
                json!({"age": after}),
            )]);
            let firing = fire_predicates(&cs, &schema, &registry, EvalLimits::default());
            assert_eq!(
                firing.terms.iter().collect::<Vec<_>>(),
                vec![&Term::Predicate(Arc::clone(&pred))],
                "{before} -> {after} crosses 30"
            );
        }
    }

    #[test]
    fn a_change_to_an_unmentioned_property_is_not_even_evaluated() {
        let schema = schema_fixture();
        let registry = registry_with(&[age_at_least("30")]);
        let cs = changeset(vec![DocChange::updated(
            doc("Person/alice"),
            cls("Person"),
            json!({"name": "Ann", "age": 40}),
            json!({"name": "Anne", "age": 40}),
        )]);
        let firing = fire_predicates(&cs, &schema, &registry, EvalLimits::default());
        assert_eq!(firing.evaluated, 0, "dispatch skipped it entirely");
        assert!(firing.terms.is_empty());
    }

    #[test]
    fn a_conjunction_fires_only_when_the_conjunction_flips() {
        let schema = schema_fixture();
        let pred = Arc::new(Predicate::new(
            cls("Person"),
            Pred::All(vec![
                Pred::Cmp {
                    prop: prop("age"),
                    op: CmpOp::Ge,
                    value: Lit::Num("30".to_owned()),
                    order: Order::Integer,
                },
                Pred::Cmp {
                    prop: prop("name"),
                    op: CmpOp::Eq,
                    value: Lit::Text("Ann".to_owned()),
                    order: Order::Text,
                },
            ]),
        ));
        let registry = registry_with(&[Arc::clone(&pred)]);

        // One conjunct moves but the conjunction stays false: nothing.
        let cs = changeset(vec![DocChange::updated(
            doc("Person/alice"),
            cls("Person"),
            json!({"age": 20, "name": "Bob"}),
            json!({"age": 40, "name": "Bob"}),
        )]);
        assert!(
            fire_predicates(&cs, &schema, &registry, EvalLimits::default())
                .terms
                .is_empty(),
            "still not named Ann, so still not a member"
        );

        // Now the same conjunct moves and the conjunction flips.
        let cs = changeset(vec![DocChange::updated(
            doc("Person/alice"),
            cls("Person"),
            json!({"age": 20, "name": "Ann"}),
            json!({"age": 40, "name": "Ann"}),
        )]);
        assert_eq!(
            fire_predicates(&cs, &schema, &registry, EvalLimits::default())
                .terms
                .len(),
            1
        );
    }

    #[test]
    fn a_create_and_a_delete_evaluate_every_predicate_on_the_class() {
        let schema = schema_fixture();
        // A predicate satisfied by a document with no `age` at all, which is
        // exactly what `changed_props`-driven dispatch would miss on a create.
        let pred = Arc::new(Predicate::new(
            cls("Person"),
            Pred::Not(Box::new(Pred::Cmp {
                prop: prop("age"),
                op: CmpOp::Ge,
                value: Lit::Num("30".to_owned()),
                order: Order::Integer,
            })),
        ));
        let registry = registry_with(&[Arc::clone(&pred)]);

        let created = changeset(vec![DocChange::created(
            doc("Person/alice"),
            cls("Person"),
            json!({"name": "Ann"}),
        )]);
        assert_eq!(
            fire_predicates(&created, &schema, &registry, EvalLimits::default())
                .terms
                .len(),
            1,
            "a document with no `age` entered the `_not(age >= 30)` set"
        );

        let deleted = changeset(vec![DocChange::deleted(
            doc("Person/alice"),
            cls("Person"),
            json!({"name": "Ann"}),
        )]);
        assert_eq!(
            fire_predicates(&deleted, &schema, &registry, EvalLimits::default())
                .terms
                .len(),
            1,
            "and left it again"
        );
    }

    #[test]
    fn a_create_outside_the_set_fires_nothing() {
        let schema = schema_fixture();
        let registry = registry_with(&[age_at_least("30")]);
        let cs = changeset(vec![DocChange::created(
            doc("Person/alice"),
            cls("Person"),
            json!({"age": 20}),
        )]);
        assert!(
            fire_predicates(&cs, &schema, &registry, EvalLimits::default())
                .terms
                .is_empty(),
            "it was never a member and still is not"
        );
    }

    #[test]
    fn a_subclass_instance_is_evaluated_against_a_superclass_predicate() {
        let schema = schema_fixture();
        let pred = age_at_least("30");
        let registry = registry_with(&[Arc::clone(&pred)]);
        let cs = changeset(vec![DocChange::updated(
            doc("Employee/e1"),
            cls("Employee"),
            json!({"age": 20}),
            json!({"age": 40}),
        )]);
        assert_eq!(
            fire_predicates(&cs, &schema, &registry, EvalLimits::default())
                .terms
                .len(),
            1,
            "an Employee is a Person"
        );
    }

    #[test]
    fn an_undecidable_value_fires_coarsely_rather_than_guessing() {
        let schema = schema_fixture();
        let registry = registry_with(&[age_at_least("30")]);
        let cs = changeset(vec![DocChange::updated(
            doc("Person/alice"),
            cls("Person"),
            json!({"age": 40}),
            // A string where an integer was declared.
            json!({"age": "forty"}),
        )]);
        let firing = fire_predicates(&cs, &schema, &registry, EvalLimits::default());
        assert_eq!(firing.terms.len(), 1);
        assert!(
            firing.coarse,
            "and it says so, rather than claiming precision"
        );
    }

    #[test]
    fn a_retype_goes_coarse() {
        let schema = schema_fixture();
        let registry = registry_with(&[age_at_least("30")]);
        let cs = changeset(vec![DocChange::updated(
            doc("Person/alice"),
            cls("Person"),
            json!({"@type": "Person", "age": 40}),
            json!({"@type": "Employee", "age": 40}),
        )]);
        let firing = fire_predicates(&cs, &schema, &registry, EvalLimits::default());
        assert!(firing.coarse);
        assert_eq!(firing.terms.len(), 1, "every predicate on the class goes");
        assert_eq!(firing.evaluated, 0);
    }

    // ----------------------------------------------------------- backpressure

    #[test]
    fn over_the_evaluation_bound_invalidation_goes_coarse_and_is_counted() {
        let schema = schema_fixture();
        let preds: Vec<Arc<Predicate>> = (0..10).map(|i| age_at_least(&i.to_string())).collect();
        let registry = registry_with(&preds);
        let cs = changeset(vec![DocChange::updated(
            doc("Person/alice"),
            cls("Person"),
            json!({"age": 40}),
            json!({"age": 41}),
        )]);

        // Within the bound: precise, and 40 -> 41 crosses none of 0..10.
        let precise = fire_predicates(&cs, &schema, &registry, EvalLimits::default());
        assert!(precise.terms.is_empty());
        assert_eq!(precise.skipped, 0);

        let limits = EvalLimits {
            max_evaluations: 4,
            ..EvalLimits::default()
        };
        let coarse = fire_predicates(&cs, &schema, &registry, limits);
        assert!(coarse.coarse);
        assert_eq!(coarse.terms.len(), 10, "every live predicate on the class");
        assert_eq!(coarse.skipped, 10);
        assert_eq!(coarse.evaluated, 0);
    }

    #[test]
    fn over_the_predicate_count_bound_invalidation_goes_coarse() {
        let schema = schema_fixture();
        let preds: Vec<Arc<Predicate>> = (0..10).map(|i| age_at_least(&i.to_string())).collect();
        let registry = registry_with(&preds);
        let cs = changeset(vec![DocChange::updated(
            doc("Person/alice"),
            cls("Person"),
            json!({"age": 40}),
            json!({"age": 41}),
        )]);
        let limits = EvalLimits {
            max_predicates: 3,
            ..EvalLimits::default()
        };
        let firing = fire_predicates(&cs, &schema, &registry, limits);
        assert!(firing.coarse);
        assert_eq!(firing.skipped, 10);
    }

    #[test]
    fn an_empty_registry_costs_nothing() {
        let schema = schema_fixture();
        let registry = PredicateRegistry::new();
        let cs = changeset(vec![DocChange::updated(
            doc("Person/alice"),
            cls("Person"),
            json!({"age": 40}),
            json!({"age": 41}),
        )]);
        let firing = fire_predicates(&cs, &schema, &registry, EvalLimits::default());
        assert_eq!(firing.evaluated, 0);
        assert!(firing.terms.is_empty());
    }
}
