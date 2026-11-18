//! Hand-rolled counters and a Prometheus text encoder.
//!
//! Deliberately not a metrics crate: the set of things worth counting here is
//! small, fixed, and known at compile time, and a `&'static str`-keyed registry
//! would cost an allocation and a hash on the request path to buy flexibility we
//! do not need. Every counter is a plain relaxed `AtomicU64`.
//!
//! What gets counted is chosen to answer the two questions an operator will
//! actually ask: *is the cache working* (hits, misses, bypasses) and *is it
//! honest* (rejected fills, raced fills, epoch bumps, purges). The second group
//! is the more important one — a cache that is quietly dropping every fill looks
//! identical to a cold one in the hit rate alone.

use std::sync::atomic::{AtomicU64, Ordering};

/// A monotonic counter.
#[derive(Debug, Default)]
pub struct Counter(AtomicU64);

impl Counter {
    /// Add one.
    pub fn inc(&self) {
        self.add(1);
    }

    /// Add `n`.
    pub fn add(&self, n: u64) {
        self.0.fetch_add(n, Ordering::Relaxed);
    }

    /// Read the current value. Relaxed: a metric read racing an increment may
    /// miss it, which is fine.
    pub fn get(&self) -> u64 {
        self.0.load(Ordering::Relaxed)
    }
}

/// A gauge: a value that goes up and down.
#[derive(Debug, Default)]
pub struct Gauge(AtomicU64);

impl Gauge {
    /// Overwrite the value.
    pub fn set(&self, v: u64) {
        self.0.store(v, Ordering::Relaxed);
    }

    /// Read the value.
    pub fn get(&self) -> u64 {
        self.0.load(Ordering::Relaxed)
    }
}

macro_rules! metrics {
    (
        counters { $($cname:ident : $chelp:literal),* $(,)? }
        gauges { $($gname:ident : $ghelp:literal),* $(,)? }
    ) => {
        /// Every counter Reactor keeps.
        #[derive(Debug, Default)]
        pub struct Metrics {
            $(
                #[doc = $chelp]
                pub $cname: Counter,
            )*
            $(
                #[doc = $ghelp]
                pub $gname: Gauge,
            )*
        }

        impl Metrics {
            /// Render in the Prometheus text exposition format.
            pub fn encode_prometheus(&self) -> String {
                let mut out = String::new();
                $(
                    out.push_str(concat!("# HELP reactor_", stringify!($cname), " ", $chelp, "\n"));
                    out.push_str(concat!("# TYPE reactor_", stringify!($cname), " counter\n"));
                    out.push_str(concat!("reactor_", stringify!($cname), " "));
                    out.push_str(&self.$cname.get().to_string());
                    out.push('\n');
                )*
                $(
                    out.push_str(concat!("# HELP reactor_", stringify!($gname), " ", $ghelp, "\n"));
                    out.push_str(concat!("# TYPE reactor_", stringify!($gname), " gauge\n"));
                    out.push_str(concat!("reactor_", stringify!($gname), " "));
                    out.push_str(&self.$gname.get().to_string());
                    out.push('\n');
                )*
                out
            }
        }
    };
}

metrics! {
    counters {
        lookup_hits: "Fragment lookups served from cache.",
        lookup_misses: "Fragment lookups with no version valid at the latched position.",
        commits_applied: "Commits applied to a namespace index.",
        versions_invalidated: "Fragment versions closed by a commit.",
        fills_stored: "Completed fills stored and registered as current.",
        fills_raced: "Completed fills stored with a bounded validity interval because a \
                      commit invalidated them between the pin and the landing.",
        fills_rejected_epoch: "Completed fills discarded because the epoch moved under them.",
        fills_rejected_stale: "Completed fills discarded because we no longer retain the \
                               fire history needed to validate them.",
        fills_backdated: "Completed fills whose content was unchanged, so the previous \
                          change position was inherited.",
        epoch_bumps: "Non-fast-forward head movements.",
        purges: "Namespaces cleared, by operator action or an undiffable history rewrite.",
        schema_refreshes_requested: "Schema re-derivations requested after a schema commit.",
        store_put_errors: "Content-addressed store writes that failed (body kept inline).",
        store_promotions: "Bodies promoted from the warm tier into the hot tier on read.",
        store_evictions: "Bodies evicted from the hot tier to stay within budget.",
    }
    gauges {
        store_bytes: "Approximate bytes held by the fragment store.",
        tracked_keys: "Fragment keys with at least one retained version.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counters_count() {
        let m = Metrics::default();
        m.lookup_hits.inc();
        m.lookup_hits.add(4);
        assert_eq!(m.lookup_hits.get(), 5);
        assert_eq!(m.lookup_misses.get(), 0);
    }

    #[test]
    fn gauges_overwrite() {
        let m = Metrics::default();
        m.store_bytes.set(10);
        m.store_bytes.set(3);
        assert_eq!(m.store_bytes.get(), 3);
    }

    #[test]
    fn prometheus_encoding_has_help_type_and_value_per_metric() {
        let m = Metrics::default();
        m.lookup_hits.add(7);
        let text = m.encode_prometheus();
        assert!(text.contains("# TYPE reactor_lookup_hits counter\n"));
        assert!(text.contains("\nreactor_lookup_hits 7\n"));
        assert!(text.contains("# TYPE reactor_store_bytes gauge\n"));
        // Every line is either a comment or `name value`.
        for line in text.lines() {
            assert!(
                line.starts_with('#') || line.split(' ').count() == 2,
                "malformed exposition line: {line}"
            );
        }
    }
}
