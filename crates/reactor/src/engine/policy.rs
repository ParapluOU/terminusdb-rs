//! Per-route policy: what to do with a cacheable entry once we have one.
//!
//! Policy is separated from analysis because the two answer different questions.
//! `analyze` answers "*may* this be cached, and on what does it depend?" — a
//! soundness question with one right answer. Policy answers "*should* it be, and
//! how hard should we work to keep it fresh?" — a tuning question with no right
//! answer, which therefore belongs in config rather than in code.

use std::time::Duration;

/// When to recompute an invalidated fragment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Refresh {
    /// Drop it and refill on the next request. Cheap, and correct for the long
    /// tail of entries nobody asks for twice.
    #[default]
    Lazy,
    /// Recompute as soon as it is invalidated, so the next reader never waits.
    /// This is the generalization of the prototype's cache warmer, and the
    /// setting under which backdating earns its keep: an eager recomputation
    /// that produces identical bytes costs one upstream read and saves every
    /// parent fragment above it.
    Eager,
}

/// What to do when a client demands a version this node has not reached.
///
/// Design doc §5.5(5). There is no fourth option that is honest: a cache must
/// not silently pretend to be ahead of its node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FencePolicy {
    /// Wait for the watermark to reach the demanded version, then fall through.
    Wait {
        /// How long to wait before giving up and falling through.
        timeout: Duration,
    },
    /// Answer `412 Precondition Failed` so a load balancer can retry on a node
    /// that has replicated further. The right setting for read-after-write.
    Fail,
    /// Answer from what we have and label the response as stale.
    ServeStale,
}

impl Default for FencePolicy {
    fn default() -> Self {
        FencePolicy::Wait {
            timeout: Duration::from_millis(500),
        }
    }
}

/// How a response is split into fragments.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Decomposition {
    /// One fragment per response. Always correct, and the escape hatch: it is
    /// unconditionally byte-faithful because the bytes are never taken apart.
    #[default]
    Whole,
    /// One fragment per document plus a spine, so one changed document does not
    /// invalidate a whole document list. Composition is pure concatenation,
    /// which is why this is the one decomposition worth doing first. See
    /// [`crate::compose`]; a response that does not split cleanly falls back to
    /// [`Decomposition::Whole`] per response, not per route.
    NdjsonList,
}

/// Everything configurable about one cacheable route.
#[derive(Clone, Debug, PartialEq)]
pub struct Policy {
    /// Eager or lazy recomputation.
    pub refresh: Refresh,
    /// Optional time bound on an entry's life, *in addition to* term-based
    /// invalidation. A TTL is a safety net for dependencies we failed to model,
    /// not the primary mechanism: if a route needs a short TTL to be correct,
    /// its analyzer is wrong.
    pub ttl: Option<Duration>,
    /// What to do about `X-Reactor-Min-Version`.
    pub fence: FencePolicy,
    /// How to split responses.
    pub decompose: Decomposition,
    /// Bodies larger than this are forwarded and not cached, so one huge
    /// response cannot evict the whole working set.
    pub max_body_bytes: u64,
    /// Bodies at or below this size are held inline on the version instead of
    /// going through the content-addressed store, where the hash lookup would
    /// cost more than the bytes.
    pub inline_max_bytes: usize,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            refresh: Refresh::Lazy,
            ttl: None,
            fence: FencePolicy::default(),
            decompose: Decomposition::Whole,
            max_body_bytes: 8 * 1024 * 1024,
            inline_max_bytes: 16 * 1024,
        }
    }
}

impl Policy {
    /// True when a body of this size may be cached at all.
    pub fn may_cache_body(&self, len: usize) -> bool {
        len as u64 <= self.max_body_bytes
    }

    /// True when a body of this size should be held inline.
    pub fn inline(&self, len: usize) -> bool {
        len <= self.inline_max_bytes
    }

    /// True when a version filled `age` ago is still within its TTL.
    pub fn fresh_enough(&self, age: Duration) -> bool {
        self.ttl.map_or(true, |ttl| age < ttl)
    }
}

/// Policies by route prefix, with a default.
///
/// Longest-prefix wins, so `/api/document` can be configured separately from
/// `/api` without ordering mattering in the config file.
#[derive(Clone, Debug, Default)]
pub struct PolicySet {
    default: Policy,
    by_prefix: Vec<(String, Policy)>,
}

impl PolicySet {
    /// A policy set with one default and no overrides.
    pub fn new(default: Policy) -> Self {
        Self {
            default,
            by_prefix: Vec::new(),
        }
    }

    /// Add a route override. The prefix is matched against the request path.
    pub fn with_prefix(mut self, prefix: impl Into<String>, policy: Policy) -> Self {
        self.by_prefix.push((prefix.into(), policy));
        // Longest first, so the first match is the most specific one.
        self.by_prefix
            .sort_by_key(|(prefix, _)| std::cmp::Reverse(prefix.len()));
        self
    }

    /// The policy for a path.
    pub fn for_path(&self, path: &str) -> &Policy {
        self.by_prefix
            .iter()
            .find(|(prefix, _)| path.starts_with(prefix.as_str()))
            .map(|(_, policy)| policy)
            .unwrap_or(&self.default)
    }

    /// The fallback policy.
    pub fn default_policy(&self) -> &Policy {
        &self.default
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_conservative() {
        let p = Policy::default();
        assert_eq!(p.refresh, Refresh::Lazy);
        assert!(p.ttl.is_none(), "term invalidation, not time, is primary");
        assert_eq!(p.decompose, Decomposition::Whole);
        assert!(matches!(p.fence, FencePolicy::Wait { .. }));
    }

    #[test]
    fn body_size_limits_apply() {
        let p = Policy {
            max_body_bytes: 100,
            inline_max_bytes: 10,
            ..Policy::default()
        };
        assert!(p.may_cache_body(100));
        assert!(!p.may_cache_body(101));
        assert!(p.inline(10));
        assert!(!p.inline(11));
    }

    #[test]
    fn ttl_defaults_to_unbounded() {
        let p = Policy::default();
        assert!(p.fresh_enough(Duration::from_secs(86_400)));

        let ttl = Policy {
            ttl: Some(Duration::from_secs(5)),
            ..Policy::default()
        };
        assert!(ttl.fresh_enough(Duration::from_secs(4)));
        assert!(!ttl.fresh_enough(Duration::from_secs(5)));
    }

    #[test]
    fn longest_prefix_wins_regardless_of_insertion_order() {
        let specific = Policy {
            refresh: Refresh::Eager,
            ..Policy::default()
        };
        let general = Policy {
            max_body_bytes: 1,
            ..Policy::default()
        };
        let set = PolicySet::new(Policy::default())
            .with_prefix("/api", general)
            .with_prefix("/api/document", specific);

        assert_eq!(
            set.for_path("/api/document/admin/db").refresh,
            Refresh::Eager
        );
        assert_eq!(set.for_path("/api/woql/admin/db").max_body_bytes, 1);
        assert_eq!(
            set.for_path("/healthz").max_body_bytes,
            Policy::default().max_body_bytes
        );
    }
}
