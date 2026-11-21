//! Which namespaces this node tracks, and how warm each one is.
//!
//! Until now the answer was "the ones in the configuration file", decided once
//! at startup and looked up by scanning a `Vec`. That is the right shape for a
//! handful of databases named by an operator, and the wrong one for an
//! application that *creates* databases: a room-per-tenant or
//! publication-per-database layout has thousands of namespaces, appearing while
//! the process runs, and nobody can list them in a TOML file.
//!
//! So tracking is a registry: seeded from the configuration, added to at run
//! time through `/_reactor/namespace`, and consulted on the read path by hash
//! rather than by scan.
//!
//! # Why a cap, and why opt-in
//!
//! Each tracked namespace costs a capture task, a writer, a schema index and —
//! if it subscribes — one of the node's **per-user** SSE slots, which the fork
//! caps at 20. A thousand tracked namespaces is a thousand poll loops against
//! one node. So the registry is bounded, refuses rather than grows past the
//! bound, and automatic registration is off unless an operator asks for it:
//! invariant 6 says an unknown route is forwarded rather than cached, and
//! "cache every database anybody mentions" inverts that default.
//!
//! # Warmth
//!
//! The registry also records when each namespace was last *read*, which is what
//! lets capture poll an idle namespace rarely. For a per-tenant layout that is
//! the difference between one poll loop per active room and one per room that
//! has ever existed. Staleness in a namespace nobody is reading costs nothing,
//! because invariant 4 means a lagging watermark is answered from an older
//! snapshot rather than wrongly — and the first read after a quiet spell pays a
//! head check, not a wrong answer.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;

use crate::ids::Namespace;

/// How a namespace came to be tracked.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Source {
    /// Named in `[[namespace]]`. Cannot be untracked at run time: the
    /// configuration is the operator's statement of intent, and a process that
    /// forgot it on request would come back tracking it again.
    Configured,
    /// Registered through `POST /_reactor/namespace`.
    Registered,
    /// Registered by the read path, because `[capture].auto_register` is on.
    Automatic,
}

/// What the registry knows about one namespace.
#[derive(Debug)]
struct Entry {
    source: Source,
    since: Instant,
    /// Millis since the registry was created, **plus one**, so the hot path can
    /// update it with a single relaxed store and `0` can still mean "never
    /// read". Without the offset, a namespace read in the first millisecond of
    /// the process would be indistinguishable from one never read — which the
    /// first version of this got wrong, and a test caught.
    last_read_ms: AtomicU64,
}

/// A tracked namespace, as the admin endpoint reports it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Tracked {
    /// `org/db/local/branch/<branch>`.
    pub namespace: String,
    /// How it came to be tracked.
    pub source: Source,
    /// Seconds since it was registered.
    pub tracked_for_secs: u64,
    /// Seconds since a read last touched it; `None` if it has never been read.
    pub idle_secs: Option<u64>,
}

/// The namespaces this node tracks.
pub struct Registry {
    entries: DashMap<Namespace, Entry>,
    cap: usize,
    created: Instant,
}

impl Registry {
    /// A registry holding the configured namespaces, bounded to `cap`.
    pub fn new(configured: impl IntoIterator<Item = Namespace>, cap: usize) -> Registry {
        let registry = Registry {
            entries: DashMap::new(),
            cap: cap.max(1),
            created: Instant::now(),
        };
        for ns in configured {
            // Configured namespaces are admitted whatever the cap says: an
            // operator who lists more than the cap has made a configuration
            // error, and silently ignoring half of their file would be a
            // confusing way to tell them.
            registry.insert(ns, Source::Configured);
        }
        registry
    }

    /// Whether this namespace is tracked. One hash lookup.
    pub fn tracks(&self, ns: &Namespace) -> bool {
        self.entries.contains_key(ns)
    }

    /// Start tracking a namespace.
    ///
    /// `Ok(true)` when it was newly added, `Ok(false)` when it was already
    /// tracked (so registration is idempotent and a client can call it on every
    /// request without thinking), and `Err` when the cap is reached.
    pub fn track(&self, ns: Namespace, source: Source) -> Result<bool, Full> {
        if self.entries.contains_key(&ns) {
            return Ok(false);
        }
        if self.entries.len() >= self.cap {
            return Err(Full { cap: self.cap });
        }
        self.insert(ns, source);
        Ok(true)
    }

    /// Stop tracking a namespace.
    ///
    /// Refuses a configured one: see [`Source::Configured`]. The caller is
    /// responsible for purging whatever was cached — untracking alone would
    /// leave entries nothing is watching.
    pub fn untrack(&self, ns: &Namespace) -> Result<bool, Configured> {
        match self.entries.get(ns).map(|entry| entry.source) {
            None => Ok(false),
            Some(Source::Configured) => Err(Configured),
            Some(_) => {
                self.entries.remove(ns);
                Ok(true)
            }
        }
    }

    /// Note that a read touched this namespace. One relaxed store.
    pub fn note_read(&self, ns: &Namespace) {
        if let Some(entry) = self.entries.get(ns) {
            entry
                .last_read_ms
                .store(self.now_ms() + 1, Ordering::Relaxed);
        }
    }

    /// How long since a read touched this namespace.
    ///
    /// `None` for an untracked namespace. A namespace that has never been read
    /// reports the time since it was registered, which is what makes a
    /// never-read namespace back off like an idle one rather than like a hot
    /// one.
    pub fn idle_for(&self, ns: &Namespace) -> Option<Duration> {
        let entry = self.entries.get(ns)?;
        match entry.last_read_ms.load(Ordering::Relaxed) {
            NEVER_READ => Some(entry.since.elapsed()),
            last => Some(Duration::from_millis(
                self.now_ms().saturating_sub(last - 1),
            )),
        }
    }

    /// Every tracked namespace.
    pub fn namespaces(&self) -> Vec<Namespace> {
        self.entries
            .iter()
            .map(|entry| entry.key().clone())
            .collect()
    }

    /// Every tracked namespace, with its warmth, sorted by name so the admin
    /// endpoint is diffable.
    pub fn report(&self) -> Vec<Tracked> {
        let now = self.now_ms();
        let mut out: Vec<Tracked> = self
            .entries
            .iter()
            .map(|entry| {
                let last = entry.last_read_ms.load(Ordering::Relaxed);
                Tracked {
                    namespace: entry.key().path(),
                    source: entry.source,
                    tracked_for_secs: entry.since.elapsed().as_secs(),
                    idle_secs: (last != NEVER_READ)
                        .then(|| Duration::from_millis(now.saturating_sub(last - 1)).as_secs()),
                }
            })
            .collect();
        out.sort_by(|a, b| a.namespace.cmp(&b.namespace));
        out
    }

    /// How many namespaces are tracked.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether nothing is tracked, in which case Reactor caches nothing.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The bound on how many namespaces may be tracked.
    pub fn cap(&self) -> usize {
        self.cap
    }

    fn insert(&self, ns: Namespace, source: Source) {
        self.entries.insert(
            ns,
            Entry {
                source,
                since: Instant::now(),
                last_read_ms: AtomicU64::new(NEVER_READ),
            },
        );
    }

    fn now_ms(&self) -> u64 {
        self.created.elapsed().as_millis() as u64
    }
}

/// `last_read_ms` for a namespace no read has touched.
///
/// Zero is free as a sentinel because stored values are offset by one; "never
/// read" then reports idleness since *registration*, which is what makes a
/// namespace nobody has asked for back off like an idle one rather than like a
/// hot one.
const NEVER_READ: u64 = 0;

/// The registry is full.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("the namespace registry is full ({cap} tracked)")]
pub struct Full {
    /// The cap that was reached.
    pub cap: usize,
}

/// A configured namespace cannot be untracked at run time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("this namespace is named in the configuration file, so it cannot be untracked at run time")]
pub struct Configured;

/// Shared handle.
pub type SharedRegistry = Arc<Registry>;

#[cfg(test)]
mod tests {
    use super::*;

    fn ns(db: &str) -> Namespace {
        Namespace::new("admin", db, "main")
    }

    #[test]
    fn the_configured_namespaces_are_tracked_from_the_start() {
        let registry = Registry::new([ns("one"), ns("two")], 10);
        assert!(registry.tracks(&ns("one")));
        assert!(registry.tracks(&ns("two")));
        assert!(!registry.tracks(&ns("three")));
        assert_eq!(registry.len(), 2);
    }

    #[test]
    fn registration_is_idempotent() {
        let registry = Registry::new([], 10);
        assert_eq!(registry.track(ns("room"), Source::Registered), Ok(true));
        assert_eq!(
            registry.track(ns("room"), Source::Registered),
            Ok(false),
            "a client may register on every request without thinking about it"
        );
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn the_cap_refuses_rather_than_grows() {
        let registry = Registry::new([], 2);
        assert_eq!(registry.track(ns("a"), Source::Registered), Ok(true));
        assert_eq!(registry.track(ns("b"), Source::Automatic), Ok(true));
        assert_eq!(
            registry.track(ns("c"), Source::Registered),
            Err(Full { cap: 2 })
        );
        assert!(!registry.tracks(&ns("c")));
    }

    #[test]
    fn a_configured_namespace_cannot_be_untracked() {
        let registry = Registry::new([ns("configured")], 10);
        assert_eq!(registry.untrack(&ns("configured")), Err(Configured));
        assert!(registry.tracks(&ns("configured")));
    }

    #[test]
    fn a_registered_namespace_can_be_untracked_and_an_unknown_one_is_not_an_error() {
        let registry = Registry::new([], 10);
        registry.track(ns("room"), Source::Registered).unwrap();
        assert_eq!(registry.untrack(&ns("room")), Ok(true));
        assert!(!registry.tracks(&ns("room")));
        assert_eq!(
            registry.untrack(&ns("never")),
            Ok(false),
            "untracking something untracked is the state the caller asked for"
        );
    }

    #[test]
    fn a_configured_namespace_is_admitted_over_the_cap() {
        // An operator who lists more namespaces than the cap has made a
        // configuration error; dropping half the file quietly would be a poor
        // way to tell them.
        let registry = Registry::new([ns("a"), ns("b"), ns("c")], 1);
        assert_eq!(registry.len(), 3);
        assert!(registry.tracks(&ns("c")));
        // But nothing *new* is admitted.
        assert!(registry.track(ns("d"), Source::Automatic).is_err());
    }

    #[test]
    fn warmth_is_recorded_and_an_unread_namespace_looks_idle() {
        let registry = Registry::new([ns("hot")], 10);
        // Never read: idle since registration, not "idle for zero".
        let idle = registry.idle_for(&ns("hot")).expect("tracked");
        assert!(idle < Duration::from_secs(1), "the clock starts at zero");
        assert_eq!(
            registry.report()[0].idle_secs,
            None,
            "never read is reported as never read, not as just-read"
        );

        registry.note_read(&ns("hot"));
        assert_eq!(registry.report()[0].idle_secs, Some(0));
        assert_eq!(
            registry.idle_for(&ns("cold")),
            None,
            "untracked has no warmth"
        );
    }

    #[test]
    fn noting_a_read_for_an_untracked_namespace_does_nothing() {
        let registry = Registry::new([], 10);
        registry.note_read(&ns("unknown"));
        assert!(
            registry.is_empty(),
            "a read does not track a namespace by itself"
        );
    }

    #[test]
    fn the_report_names_the_source_and_sorts() {
        let registry = Registry::new([ns("zeta")], 10);
        registry.track(ns("alpha"), Source::Registered).unwrap();
        registry.track(ns("mid"), Source::Automatic).unwrap();
        let report = registry.report();
        let names: Vec<&str> = report.iter().map(|t| t.namespace.as_str()).collect();
        assert_eq!(
            names,
            vec![
                "admin/alpha/local/branch/main",
                "admin/mid/local/branch/main",
                "admin/zeta/local/branch/main"
            ]
        );
        assert_eq!(report[0].source, Source::Registered);
        assert_eq!(report[1].source, Source::Automatic);
        assert_eq!(report[2].source, Source::Configured);
    }
}
