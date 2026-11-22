//! Turning a commit id into an exact [`ChangeSet`], by pinned local reads.
//!
//! # Why `/api/changes` plus pinned reads, and not `POST /api/diff`
//!
//! `/api/diff` would hand us before/after documents in one call, but it is
//! driven by raw `TerminusDB-Data-Version` strings (`"branch:<commit>"`), and
//! synthesising one for an arbitrary historical commit is guesswork. This
//! resolver instead uses three primitives that are all verified in the client
//! and all cheap:
//!
//! * `GET /api/changes` pinned at the commit → the ids it added, changed, deleted.
//! * `GET /api/document?id=…` pinned at the commit → the *after* state.
//! * the same pinned at the **parent** → the *before* state.
//!
//! The cost is one read per changed document instead of one call per commit,
//! which is why `capture.diff_threshold` exists: above it we stop reading
//! altogether and fire coarse terms.
//!
//! # Why before-state is worth paying for
//!
//! Without it, a value change cannot fire the *vacated*
//! [`crate::term::Term::Index`] bucket and a link change cannot fire the
//! *losing* [`crate::term::Term::Backlink`]. Both are exactly the terms that
//! make filtered lists and reverse edges cacheable at all. Design doc §5.3.
//!
//! # `/api/changes` is a TerminusDB 12.1 endpoint
//!
//! On an older node it 404s, and the client's `ChangesResponse` turns that
//! error body into three empty lists (every field is `#[serde(default)]`). An
//! empty list is therefore treated as *unknown*, not as *nothing changed* — see
//! the comment at the `is_empty` check. Verified in `tests/spikes.rs`.
//!
//! # Known gap in schema-change detection
//!
//! `schema_changed` is derived from a log entry's `migration` list plus any
//! changed id that parses as a schema document. A schema commit that records no
//! migration **and** whose ids we cannot recognize as schema documents would be
//! missed, and a fragment depending on [`crate::term::Term::Schema`] would
//! survive it. The mitigation is that such a commit still fires `Doc`/`Class`
//! terms for whatever it did touch; the residual risk is limited to entries
//! whose *only* dependency is `Schema`, which in practice means introspection
//! and class-frame responses. Worth revisiting with a live server in phase E.

use std::sync::Arc;

use async_trait::async_trait;

use crate::capture::{ChangeResolver, CommitHistory, LogCommit, ReadinessGate};
use crate::change::{ChangeSet, DocChange};
use crate::ids::{ClassName, DocId, Namespace};
use terminusdb_client::{CommitId, GetOpts, LogOpts, TerminusDBHttpClient};

/// Resolves commits against a live TerminusDB node.
/// The upstream clients, one per organization.
///
/// `BranchSpec` carries no organization — the client holds it, as
/// [`Namespace::to_branch_spec`](crate::ids::Namespace::to_branch_spec) says
/// outright — so every call *about* a namespace has to go through a client
/// configured with **that namespace's** org. One Reactor fronts one node, but a
/// node has as many organizations as its application has tenants, so a single
/// client built from `[upstream].org` cannot serve them: capture used to poll
/// every namespace's head as `/api/log/<upstream org>/<db>/…` whatever the
/// namespace said, so a namespace outside that one organization never got a
/// watermark and nothing in it was ever cached (and a same-named database in the
/// configured org would have answered for it).
///
/// Kept per org rather than cloned per call because
/// [`TerminusDBHttpClient::for_org`] clears the clone's ensured-databases cache,
/// so cloning per request would throw that away every time.
#[derive(Debug)]
pub struct Clients {
    /// The client as configured, for calls that are about no namespace.
    base: Arc<TerminusDBHttpClient>,
    by_org: dashmap::DashMap<String, Arc<TerminusDBHttpClient>>,
}

impl Clients {
    /// Wrap the configured client.
    pub fn new(base: Arc<TerminusDBHttpClient>) -> Self {
        Self {
            base,
            by_org: dashmap::DashMap::new(),
        }
    }

    /// The client for calls about `ns`.
    pub fn for_ns(&self, ns: &Namespace) -> Arc<TerminusDBHttpClient> {
        if ns.org == self.base.org() {
            return Arc::clone(&self.base);
        }
        if let Some(found) = self.by_org.get(&ns.org) {
            return Arc::clone(found.value());
        }
        let scoped = Arc::new(self.base.for_org(&ns.org));
        self.by_org
            .entry(ns.org.clone())
            .or_insert(scoped)
            .value()
            .clone()
    }

    /// The client as configured.
    pub fn base(&self) -> &Arc<TerminusDBHttpClient> {
        &self.base
    }
}

pub struct TerminusResolver {
    clients: Arc<Clients>,
    /// Above this many touched documents, stop reading and fire coarse terms.
    diff_threshold: usize,
}

impl TerminusResolver {
    /// Build a resolver.
    pub fn new(clients: Arc<Clients>, diff_threshold: usize) -> Self {
        Self {
            clients,
            diff_threshold,
        }
    }

    /// Read one document pinned at a commit.
    ///
    /// `unfold` is deliberately off: we want the stored shape, where links are
    /// id references. An unfolded read would embed whole documents and the link
    /// targets would be harder, not easier, to extract.
    // Deliberately untyped: capture reads documents of whatever class a commit
    // touched, to extract their link targets. There is no single `Target` to
    // deserialize into, and we want the stored shape rather than a model's.
    #[allow(deprecated)]
    async fn read_at(
        &self,
        ns: &Namespace,
        commit: &CommitId,
        id: &str,
    ) -> anyhow::Result<Option<serde_json::Value>> {
        let spec = ns.to_commit_spec(commit);
        self.clients
            .for_ns(ns)
            .get_document_if_exists(id, &spec, GetOpts::default())
            .await
    }

    /// Did this commit touch the schema graph?
    ///
    /// Two signals, both cheap: a migration record on the log entry, and a
    /// touched id that looks like a schema document. See the module docs for
    /// the gap this leaves.
    async fn schema_changed(&self, ns: &Namespace, commit: &CommitId, touched: &[&String]) -> bool {
        let migration = match self
            .clients
            .for_ns(ns)
            .log(
                &ns.to_commit_spec(commit),
                LogOpts {
                    count: Some(1),
                    ..LogOpts::default()
                },
            )
            .await
        {
            Ok(entries) => entries
                .first()
                .map(|entry| !entry.migration.is_empty())
                .unwrap_or(false),
            Err(err) => {
                tracing::debug!(%ns, %commit, error = %err, "could not read the log entry");
                false
            }
        };
        migration || touched.iter().any(|id| looks_like_schema_document(id))
    }
}

/// Heuristic: ids in the schema graph are class or `sys:` documents rather than
/// instance documents.
fn looks_like_schema_document(id: &str) -> bool {
    id.contains("/schema#") || id.starts_with("sys:") || id.starts_with("@schema:")
}

/// The class a change should be attributed to.
///
/// Prefer the document's own `@type`, because an id's prefix can be absent (a
/// lexical key with no class prefix) while `@type` is always present on a
/// document TerminusDB returned.
fn class_of(
    id: &DocId,
    before: Option<&serde_json::Value>,
    after: Option<&serde_json::Value>,
) -> Option<ClassName> {
    for side in [after, before].into_iter().flatten() {
        if let Some(ty) = side.get("@type").and_then(|t| t.as_str()) {
            let ty = ty.rsplit(['#', ':']).next().unwrap_or(ty);
            if !ty.is_empty() {
                return Some(ClassName::new(ty));
            }
        }
    }
    id.class()
}

#[async_trait]
impl ChangeResolver for TerminusResolver {
    async fn resolve(
        &self,
        ns: &Namespace,
        commit: &CommitId,
        parent: Option<&CommitId>,
    ) -> anyhow::Result<ChangeSet> {
        let changes = self
            .clients
            .for_ns(ns)
            .get_changes(&ns.to_commit_spec(commit), None)
            .await?;

        let touched: Vec<&String> = changes
            .added
            .iter()
            .chain(changes.changed.iter())
            .chain(changes.deleted.iter())
            .collect();
        let schema_changed = self.schema_changed(ns, commit, &touched).await;

        let mut changeset = ChangeSet {
            ns: ns.clone(),
            commit: commit.clone(),
            parent: parent.cloned(),
            changes: Vec::new(),
            schema_changed,
            complete: true,
        };

        // An empty change list is NOT evidence that the commit changed nothing.
        //
        // Found the hard way in `tests/spikes.rs`: on a server without
        // `/api/changes` (it is a 12.1 addition — 12.0.6 answers 404) the
        // client's `ChangesResponse` deserializes the *error body* into
        // `added/changed/deleted = []`, because all three fields are
        // `#[serde(default)]`. The resolver then produced an empty, **complete**
        // changeset, which fires no terms at all and silently under-invalidates
        // every cached fragment — exactly the failure this design exists to
        // prevent, and invisible from the outside.
        //
        // So an empty list is treated as "we do not know". A commit that
        // genuinely touched no instance documents (a schema-only commit) then
        // fires `Everything`, costing a namespace-wide refill. That is the right
        // trade: over-invalidation is a performance bug, under-invalidation is a
        // correctness bug.
        if changes.is_empty() {
            tracing::debug!(
                %ns, %commit,
                "/api/changes reported no documents: treating the commit as opaque \
                 rather than as empty"
            );
            changeset.complete = false;
            return Ok(changeset);
        }

        // Too big to read document by document. Fire coarse terms instead:
        // over-invalidation is a performance bug, under-invalidation is a
        // correctness bug.
        if changes.len() > self.diff_threshold {
            tracing::info!(
                %ns, %commit, touched = changes.len(), threshold = self.diff_threshold,
                "commit exceeds the diff threshold: firing coarse terms"
            );
            changeset.complete = false;
            return Ok(changeset);
        }

        // SOUNDNESS: every path that cannot produce exact before/after state
        // sets `complete = false`. `Term::subsumes` matches `Backlink` only
        // against a fired `Backlink`, and a fired `Backlink` can only be
        // derived from the *old* targets of a changed link. So a changeset that
        // reports itself complete without real before-state produces a
        // reverse-edge fragment that is never invalidated again — stale
        // forever, with no recovery path. When in doubt, incomplete.
        let mut incomplete = false;

        for id in &changes.added {
            match self.read_at(ns, commit, id).await {
                Ok(after) => changeset
                    .changes
                    .push(doc_change(id, None, after, &mut incomplete)),
                Err(err) => {
                    tracing::warn!(%ns, %commit, id = %id, error = %err, "could not read an added document");
                    incomplete = true;
                }
            }
        }

        for id in &changes.changed {
            let after = self.read_at(ns, commit, id).await;
            // The before state needs the parent. Without one we cannot get it,
            // and guessing is exactly what must not happen here.
            let before = match parent {
                Some(parent) => self.read_at(ns, parent, id).await,
                None => Ok(None),
            };
            match (before, after) {
                (Ok(before), Ok(after)) => {
                    if parent.is_none() {
                        // A changed document in a commit with no parent is a
                        // contradiction; treat it as unknown rather than as a
                        // creation.
                        incomplete = true;
                    }
                    changeset
                        .changes
                        .push(doc_change(id, before, after, &mut incomplete));
                }
                (before, after) => {
                    if let Err(err) = before {
                        tracing::warn!(%ns, %commit, id = %id, error = %err, "could not read before-state");
                    }
                    if let Err(err) = after {
                        tracing::warn!(%ns, %commit, id = %id, error = %err, "could not read after-state");
                    }
                    incomplete = true;
                }
            }
        }

        for id in &changes.deleted {
            let before = match parent {
                Some(parent) => self.read_at(ns, parent, id).await,
                None => {
                    // Nothing can be deleted by the first commit on a branch.
                    incomplete = true;
                    Ok(None)
                }
            };
            match before {
                Ok(before) => changeset
                    .changes
                    .push(doc_change(id, before, None, &mut incomplete)),
                Err(err) => {
                    tracing::warn!(%ns, %commit, id = %id, error = %err, "could not read a deleted document");
                    incomplete = true;
                }
            }
        }

        if incomplete {
            changeset.complete = false;
        }
        Ok(changeset)
    }
}

/// Build one [`DocChange`], marking the changeset incomplete if the change
/// cannot be attributed to a class (without one we cannot fire `Class` or
/// `Members` terms, so the picture is not complete).
fn doc_change(
    id: &str,
    before: Option<serde_json::Value>,
    after: Option<serde_json::Value>,
    incomplete: &mut bool,
) -> DocChange {
    let id = DocId::parse(id);
    let class = match class_of(&id, before.as_ref(), after.as_ref()) {
        Some(class) => class,
        None => {
            *incomplete = true;
            ClassName::new("")
        }
    };
    DocChange {
        id,
        class,
        before,
        after,
    }
}

/// The readiness gate: a pinned read that must succeed before we publish.
///
/// Uses `/api/changes` with `count=0`, which returns no ids but still requires
/// the node to resolve and open the commit — the cheapest honest proof that this
/// node really has it. The resolver needs the same call anyway, so the gate adds
/// one tiny request rather than a whole read path.
pub struct PinnedReadGate {
    clients: Arc<Clients>,
}

impl PinnedReadGate {
    /// Build a gate.
    pub fn new(clients: Arc<Clients>) -> Self {
        Self { clients }
    }
}

#[async_trait]
impl ReadinessGate for PinnedReadGate {
    async fn await_readable(&self, ns: &Namespace, commit: &CommitId) -> anyhow::Result<()> {
        self.clients
            .for_ns(ns)
            .get_changes(&ns.to_commit_spec(commit), Some(0))
            .await?;
        Ok(())
    }
}

/// Commit history from `/api/log`.
pub struct TerminusHistory {
    clients: Arc<Clients>,
}

impl TerminusHistory {
    /// Build a history reader.
    pub fn new(clients: Arc<Clients>) -> Self {
        Self { clients }
    }
}

#[async_trait]
impl CommitHistory for TerminusHistory {
    async fn recent(&self, ns: &Namespace, count: usize) -> anyhow::Result<Vec<LogCommit>> {
        let entries = self
            .clients
            .for_ns(ns)
            .log(
                &ns.to_branch_spec(),
                LogOpts {
                    count: Some(count),
                    ..LogOpts::default()
                },
            )
            .await?;
        Ok(entries
            .into_iter()
            .map(|entry| LogCommit {
                // Both fields go through the shared normalizer: `identifier` is
                // already bare, but `parent` carries a `ValidCommit/` prefix that
                // a pinned resource path rejects.
                commit: super::commit_id_from_any(&entry.identifier)
                    .unwrap_or_else(|| CommitId::new(entry.identifier)),
                parent: entry.parent.as_deref().and_then(super::commit_id_from_any),
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_class_comes_from_the_document_type_in_preference_to_the_id() {
        let id = DocId::parse("Thing/abc");
        let after = json!({"@type": "Person", "name": "Ann"});
        assert_eq!(
            class_of(&id, None, Some(&after)),
            Some(ClassName::new("Person"))
        );
    }

    #[test]
    fn a_prefixed_type_is_unprefixed() {
        let id = DocId::parse("x/1");
        for ty in ["terminusdb:///schema#Person", "@schema:Person", "Person"] {
            let doc = json!({"@type": ty});
            assert_eq!(
                class_of(&id, None, Some(&doc)),
                Some(ClassName::new("Person")),
                "{ty}"
            );
        }
    }

    #[test]
    fn a_class_falls_back_to_the_id_prefix_then_to_nothing() {
        assert_eq!(
            class_of(&DocId::parse("Person/alice"), None, None),
            Some(ClassName::new("Person"))
        );
        assert_eq!(class_of(&DocId::parse("bare"), None, None), None);
    }

    #[test]
    fn the_before_side_is_used_for_a_delete() {
        let id = DocId::parse("x/1");
        let before = json!({"@type": "Company"});
        assert_eq!(
            class_of(&id, Some(&before), None),
            Some(ClassName::new("Company"))
        );
    }

    #[test]
    fn an_unattributable_change_makes_the_changeset_incomplete() {
        // Without a class we cannot fire `Class`/`Members`, so the picture is
        // not complete and must not claim to be.
        let mut incomplete = false;
        let change = doc_change("bare", None, Some(json!({"name": "x"})), &mut incomplete);
        assert!(incomplete);
        assert_eq!(change.id, DocId::parse("bare"));
    }

    #[test]
    fn an_attributable_change_leaves_the_changeset_complete() {
        let mut incomplete = false;
        let change = doc_change(
            "Person/alice",
            Some(json!({"@type": "Person", "name": "a"})),
            Some(json!({"@type": "Person", "name": "b"})),
            &mut incomplete,
        );
        assert!(!incomplete);
        assert_eq!(change.class, ClassName::new("Person"));
        assert!(!change.is_create() && !change.is_delete());
    }

    #[test]
    fn schema_document_ids_are_recognized() {
        assert!(looks_like_schema_document("terminusdb:///schema#Person"));
        assert!(looks_like_schema_document("sys:JSON"));
        assert!(looks_like_schema_document("@schema:Person"));
        assert!(!looks_like_schema_document("Person/alice"));
    }
}
