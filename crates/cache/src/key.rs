//! What a derived value is keyed by.
//!
//! Reactor cannot derive a key for a computation it never saw — a closure has no
//! normal form to hash — so the key is composed here, out of three parts, each
//! of which is a different kind of "this is a different question":
//!
//! ```text
//! <name> / <params-hash> / <epoch>
//!   │          │             └─ which build of the code produced it
//!   │          └─ which arguments it was called with
//!   └─ which computation it is
//! ```
//!
//! Get any of the three wrong and two different values share one slot, which is
//! the one failure this design cannot detect for you: Reactor will faithfully
//! invalidate the entry when its *dependencies* change, and faithfully serve the
//! wrong computation's answer in the meantime.

use serde::Serialize;

/// FNV-1a over text, evaluated at compile time.
///
/// Used by the `derivation!` macro on `stringify!` of the computation's body,
/// which is as close to "key by the transformation itself" as Rust gets: a
/// closure is a pointer and some captures at run time, but a macro can hash its
/// own token stream. Not cryptographic and does not need to be — it
/// distinguishes versions of a function, it does not authenticate them.
pub const fn fnv1a(text: &str) -> u64 {
    let bytes = text.as_bytes();
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut i = 0;
    while i < bytes.len() {
        hash ^= bytes[i] as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        i += 1;
    }
    hash
}

/// How the epoch part of a key is chosen.
///
/// This is the knob for "my code changed, so my cached values are wrong" — the
/// one thing the dependency graph cannot see, because the graph tracks data and
/// this is a change to the function.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EpochPolicy {
    /// One epoch for the whole build: set it to your build id.
    ///
    /// Safe and the default. Every deploy starts with a cold cache, and a value
    /// produced by code that no longer exists can never be served, whatever
    /// changed and wherever it changed.
    Deploy(String),
    /// Per-derivation, from the hash of its own source text.
    ///
    /// Warm across deploys, which matters when a derivation is expensive and
    /// most releases do not touch it. The honest limit: the hash covers only the
    /// tokens inside the `derivation!` body, so if the body calls `summarise()`
    /// and *that* function changes, the key does not move and values computed by
    /// the old code keep being served. Choose this when you are willing to own
    /// that — or keep `Deploy` and pay a cold start.
    CodeHash,
}

impl Default for EpochPolicy {
    /// `Deploy`, from `TERMINUSDB_CACHE_EPOCH` if it is set.
    ///
    /// The fallback is `dev`, which is deliberately not a version: a process
    /// that forgot to say which build it is gets a stable key, so local
    /// development keeps its cache across restarts, and a deployment that wants
    /// correctness across releases has to say so.
    fn default() -> Self {
        EpochPolicy::Deploy(
            std::env::var("TERMINUSDB_CACHE_EPOCH").unwrap_or_else(|_| "dev".to_owned()),
        )
    }
}

/// A stable hash of a derivation's parameters.
///
/// Through `serde_json::Value` on purpose: `serde_json`'s map is a `BTreeMap`,
/// so a `HashMap` field — whose iteration order differs run to run — comes out
/// sorted and the key stops depending on the mood of the hasher. (`Debug`, which
/// the prototype this replaces used for keys, is explicitly not a stable format:
/// a field reorder or a tightened `Debug` impl silently re-keys every entry.)
pub fn params_hash<T: Serialize + ?Sized>(params: &T) -> anyhow::Result<String> {
    let canonical = serde_json::to_vec(&serde_json::to_value(params)?)?;
    Ok(blake3::hash(&canonical).to_hex()[..16].to_owned())
}

/// Compose `name/params/epoch`.
pub fn compose(name: &str, params: &str, epoch: &str) -> String {
    format!("{name}/{params}/{epoch}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;
    use std::collections::HashMap;

    #[derive(Serialize)]
    struct Params {
        quarter: u32,
        limit: usize,
    }

    #[test]
    fn the_same_parameters_hash_the_same_way() {
        let a = params_hash(&Params {
            quarter: 4,
            limit: 20,
        })
        .unwrap();
        let b = params_hash(&Params {
            quarter: 4,
            limit: 20,
        })
        .unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn different_parameters_do_not() {
        let a = params_hash(&Params {
            quarter: 4,
            limit: 20,
        })
        .unwrap();
        let b = params_hash(&Params {
            quarter: 3,
            limit: 20,
        })
        .unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn a_map_field_does_not_depend_on_iteration_order() {
        // The bug this guards: a `HashMap` serialized directly comes out in
        // whatever order the hasher felt like, so the key would differ between
        // two processes computing the same thing.
        let mut one = HashMap::new();
        one.insert("b".to_owned(), 2);
        one.insert("a".to_owned(), 1);
        let mut other = HashMap::new();
        other.insert("a".to_owned(), 1);
        other.insert("b".to_owned(), 2);
        assert_eq!(params_hash(&one).unwrap(), params_hash(&other).unwrap());
    }

    #[test]
    fn the_code_hash_is_a_compile_time_constant() {
        const ONE: u64 = fnv1a("let x = 1;");
        const OTHER: u64 = fnv1a("let x = 2;");
        assert_ne!(ONE, OTHER);
        assert_eq!(ONE, fnv1a("let x = 1;"));
        // The empty body is still a body, and still hashes.
        assert_ne!(fnv1a(""), 0);
    }

    #[test]
    fn the_default_epoch_is_a_deploy_epoch() {
        // Not asserting the value: the environment decides it, and that is the
        // point. Asserting the *kind* is what matters — the safe policy is the
        // one you get without choosing.
        assert!(matches!(EpochPolicy::default(), EpochPolicy::Deploy(_)));
    }

    #[test]
    fn a_key_has_three_parts() {
        let key = compose("overview", "abc123", "dev");
        assert_eq!(key, "overview/abc123/dev");
        assert_eq!(key.split('/').count(), 3);
    }
}
