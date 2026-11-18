//! The authorization gate.
//!
//! Cache keys deliberately contain no user: TerminusDB's access control is per
//! *resource* (database, graph), not per document, so one cached copy is correct
//! for everyone who may read that resource. That is a large win and it comes
//! with one obligation, which this module exists to discharge: **a cache hit
//! must not become an access-control bypass.** Without this gate, anyone who
//! could reach the proxy could read any namespace another user had warmed.
//!
//! The check is a capability probe — `GET /api/db/<org>/<db>` with the
//! *client's* `Authorization` — memoized briefly.
//!
//! Two asymmetries are deliberate:
//!
//! * **Successes are cached, denials never are.** A freshly granted capability
//!   must work immediately; a freshly revoked one may still read the cache for
//!   up to the TTL. That is the documented tradeoff, and it is the right way
//!   round: the failure mode is a brief extra permission, not a brief extra
//!   denial, and the alternative hands a thundering herd of denied requests
//!   straight to the node.
//! * **A probe we cannot complete is not a denial.** No `Authorization` header,
//!   a network failure, an unexpected status — all yield
//!   [`AuthDecision::Unknown`], which means "forward to upstream and let it
//!   decide". Reactor must never invent an authorization answer in either
//!   direction.
//!
//! Assumption, flagged because it would die loudly: if TerminusDB ever grows
//! document-level ACLs, the "no user in the key" shortcut is invalid and the
//! whole scheme needs revisiting.

use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use http::HeaderValue;

use crate::ids::{ContentHash, Namespace};

/// The outcome of an authorization check.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthDecision {
    /// The caller may read this namespace: serving a hit is safe.
    Allowed,
    /// We could not establish that. Forward upstream and let it answer.
    Unknown,
}

impl AuthDecision {
    /// True only when we positively established access.
    pub fn is_allowed(self) -> bool {
        matches!(self, AuthDecision::Allowed)
    }
}

/// Memoized capability probes.
pub struct AuthGate {
    http: reqwest::Client,
    upstream: url::Url,
    ttl: Duration,
    /// `(hash of the Authorization header, namespace)` → when the probe
    /// succeeded. Only successes are ever inserted.
    allowed: DashMap<(ContentHash, Namespace), Instant>,
}

impl AuthGate {
    /// Build a gate.
    pub fn new(http: reqwest::Client, upstream: url::Url, ttl: Duration) -> Self {
        Self {
            http,
            upstream,
            ttl,
            allowed: DashMap::new(),
        }
    }

    /// How many live grants are memoized, for metrics and tests.
    pub fn cached_grants(&self) -> usize {
        self.allowed.len()
    }

    /// The memo key. The credential is hashed, never stored: this map is
    /// reachable from an admin endpoint and from a core dump.
    fn key(auth: &HeaderValue, ns: &Namespace) -> (ContentHash, Namespace) {
        (ContentHash::of(auth.as_bytes()), ns.clone())
    }

    /// A live memoized grant, if there is one.
    fn memoized(&self, auth: &HeaderValue, ns: &Namespace) -> bool {
        let key = Self::key(auth, ns);
        match self.allowed.get(&key) {
            Some(entry) => {
                if entry.value().elapsed() < self.ttl {
                    true
                } else {
                    drop(entry);
                    self.allowed.remove(&key);
                    false
                }
            }
            None => false,
        }
    }

    /// Record a success.
    fn remember(&self, auth: &HeaderValue, ns: &Namespace) {
        self.allowed.insert(Self::key(auth, ns), Instant::now());
    }

    /// May this caller read this namespace?
    pub async fn allows(&self, auth: Option<&HeaderValue>, ns: &Namespace) -> AuthDecision {
        // An anonymous request might be legitimate (a public database), but we
        // have nothing to memoize and no way to tell it apart from an attempt,
        // so upstream decides.
        let Some(auth) = auth else {
            return AuthDecision::Unknown;
        };
        if self.memoized(auth, ns) {
            return AuthDecision::Allowed;
        }
        let url = format!(
            "{}/api/db/{}/{}",
            self.upstream.as_str().trim_end_matches('/'),
            ns.org,
            ns.db
        );
        match self
            .http
            .get(url)
            .header(http::header::AUTHORIZATION, auth.clone())
            .send()
            .await
        {
            Ok(response) if response.status().is_success() => {
                self.remember(auth, ns);
                AuthDecision::Allowed
            }
            Ok(response) => {
                // A denial is a real answer, but caching it would make a
                // just-granted capability unusable for the whole TTL.
                tracing::debug!(
                    %ns,
                    status = %response.status(),
                    "authorization probe did not allow the namespace"
                );
                AuthDecision::Unknown
            }
            Err(err) => {
                tracing::debug!(%ns, error = %err, "authorization probe failed");
                AuthDecision::Unknown
            }
        }
    }

    /// Drop every memoized grant. Used by `/_reactor/invalidate` so an operator
    /// revoking access does not have to wait out the TTL.
    pub fn clear(&self) {
        self.allowed.clear();
    }
}

/// A gate wrapper that is already satisfied, for tests and for a deployment that
/// has decided the proxy is not reachable by untrusted callers.
pub struct AlwaysAllow;

/// Either a real gate or a bypass.
///
/// An enum rather than a trait object because there are exactly two cases and
/// the choice is made once at startup.
pub enum Gate {
    /// Probe upstream.
    Probe(Arc<AuthGate>),
    /// Allow everything. Only safe where the listener itself is trusted.
    Open(AlwaysAllow),
}

impl Gate {
    /// Check access.
    pub async fn allows(&self, auth: Option<&HeaderValue>, ns: &Namespace) -> AuthDecision {
        match self {
            Gate::Probe(gate) => gate.allows(auth, ns).await,
            Gate::Open(_) => AuthDecision::Allowed,
        }
    }

    /// Drop memoized grants.
    pub fn clear(&self) {
        if let Gate::Probe(gate) = self {
            gate.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ns() -> Namespace {
        Namespace::new("admin", "mydb", "main")
    }

    fn gate(ttl: Duration) -> AuthGate {
        AuthGate::new(
            reqwest::Client::new(),
            // Nothing listens here, so every probe fails: that is what the
            // "cannot establish it" path is supposed to handle.
            url::Url::parse("http://127.0.0.1:1").unwrap(),
            ttl,
        )
    }

    #[tokio::test]
    async fn no_credentials_means_upstream_decides() {
        let gate = gate(Duration::from_secs(30));
        assert_eq!(gate.allows(None, &ns()).await, AuthDecision::Unknown);
        assert_eq!(gate.cached_grants(), 0);
    }

    #[tokio::test]
    async fn a_failed_probe_is_not_a_denial_and_is_not_cached() {
        let gate = gate(Duration::from_secs(30));
        let auth = HeaderValue::from_static("Basic nope");
        assert_eq!(
            gate.allows(Some(&auth), &ns()).await,
            AuthDecision::Unknown,
            "an unreachable node must not produce an authorization answer"
        );
        assert_eq!(
            gate.cached_grants(),
            0,
            "denials and failures are never memoized"
        );
    }

    #[tokio::test]
    async fn a_remembered_grant_is_reused_until_it_expires() {
        let gate = gate(Duration::from_millis(40));
        let auth = HeaderValue::from_static("Basic abc");
        // Simulate a successful probe without a server.
        gate.remember(&auth, &ns());
        assert_eq!(gate.allows(Some(&auth), &ns()).await, AuthDecision::Allowed);
        assert_eq!(gate.cached_grants(), 1);

        tokio::time::sleep(Duration::from_millis(60)).await;
        assert_eq!(
            gate.allows(Some(&auth), &ns()).await,
            AuthDecision::Unknown,
            "an expired grant must be re-probed, not reused"
        );
        assert_eq!(gate.cached_grants(), 0, "and the stale entry is dropped");
    }

    #[tokio::test]
    async fn grants_are_per_credential_and_per_namespace() {
        let gate = gate(Duration::from_secs(30));
        let alice = HeaderValue::from_static("Basic alice");
        let bob = HeaderValue::from_static("Basic bob");
        gate.remember(&alice, &ns());

        assert_eq!(
            gate.allows(Some(&alice), &ns()).await,
            AuthDecision::Allowed
        );
        assert_eq!(
            gate.allows(Some(&bob), &ns()).await,
            AuthDecision::Unknown,
            "one user's grant must never serve another user"
        );
        let other = Namespace::new("admin", "other", "main");
        assert_eq!(
            gate.allows(Some(&alice), &other).await,
            AuthDecision::Unknown,
            "a grant is per namespace"
        );
    }

    #[tokio::test]
    async fn clearing_drops_every_grant() {
        let gate = gate(Duration::from_secs(30));
        let auth = HeaderValue::from_static("Basic abc");
        gate.remember(&auth, &ns());
        gate.clear();
        assert_eq!(gate.cached_grants(), 0);
    }

    #[tokio::test]
    async fn the_open_gate_allows_everything() {
        let gate = Gate::Open(AlwaysAllow);
        assert!(gate.allows(None, &ns()).await.is_allowed());
    }

    #[test]
    fn the_memo_key_hashes_the_credential() {
        let auth = HeaderValue::from_static("Basic verysecret");
        let (hash, _) = AuthGate::key(&auth, &ns());
        assert_eq!(hash, ContentHash::of(b"Basic verysecret"));
        assert!(
            !format!("{hash:?}").contains("verysecret"),
            "the credential must not be recoverable from the key's debug output"
        );
    }
}
