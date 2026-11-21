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
    let mut canonical = Vec::new();
    write_canonical(&serde_json::to_value(params)?, &mut canonical);
    Ok(blake3::hash(&canonical).to_hex()[..16].to_owned())
}

/// Write a value in a form that depends on its content and nothing else.
///
/// This used to be `serde_json::to_vec`, on the assumption that `Map` sorts its
/// keys. It does — until something in the build turns on
/// `serde_json/preserve_order`, which makes `Map` an insertion-ordered
/// `IndexMap`. `crates/gql` turns it on, and cargo unifies features across a
/// build graph, so a key computed in a process that links gql ordered a map by
/// whatever the hasher felt like. `HashMap` randomises that per process, so the
/// same parameters hashed differently in two processes: not a wrong answer, but
/// a cache that never hits across processes and silently recomputes forever.
///
/// Keys are therefore sorted here rather than trusted to be sorted. Every
/// string is length-prefixed and every composite tagged, so the encoding is
/// injective — without that, `{"a":"b"}` and `{"ab":""}` could flatten to the
/// same bytes, and for a cache key a collision is not a miss, it is the wrong
/// value served under the right name.
fn write_canonical(value: &serde_json::Value, out: &mut Vec<u8>) {
    use serde_json::Value;
    match value {
        Value::Null => out.push(b'0'),
        Value::Bool(false) => out.push(b'f'),
        Value::Bool(true) => out.push(b't'),
        Value::Number(number) => {
            // `arbitrary_precision` is on workspace-wide, so a Number holds its
            // original lexical form; that is exactly what should be hashed, and
            // it keeps 1 distinct from 1.0 as the parameters distinguish them.
            out.push(b'#');
            write_bytes(number.to_string().as_bytes(), out);
        }
        Value::String(text) => {
            out.push(b'"');
            write_bytes(text.as_bytes(), out);
        }
        Value::Array(items) => {
            out.push(b'[');
            out.extend_from_slice(&(items.len() as u64).to_le_bytes());
            for item in items {
                write_canonical(item, out);
            }
        }
        Value::Object(fields) => {
            out.push(b'{');
            out.extend_from_slice(&(fields.len() as u64).to_le_bytes());
            let mut keys: Vec<&String> = fields.keys().collect();
            keys.sort_unstable();
            for key in keys {
                write_bytes(key.as_bytes(), out);
                // `keys` came from `fields`, so this cannot be absent.
                if let Some(field) = fields.get(key) {
                    write_canonical(field, out);
                }
            }
        }
    }
}

fn write_bytes(bytes: &[u8], out: &mut Vec<u8>) {
    out.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    out.extend_from_slice(bytes);
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
        // Two HashMaps with the same keys iterate identically inside one
        // process — RandomState is per process, not per map — so comparing
        // them proves nothing. What has to hold is that the ENCODING ignores
        // key order, which is testable directly by building the two orders by
        // hand. This is the test that fails when `serde_json/preserve_order`
        // is in the build and `Map` stops sorting for us.
        let mut ascending = serde_json::Map::new();
        ascending.insert("a".to_owned(), serde_json::json!(1));
        ascending.insert("b".to_owned(), serde_json::json!(2));
        let mut descending = serde_json::Map::new();
        descending.insert("b".to_owned(), serde_json::json!(2));
        descending.insert("a".to_owned(), serde_json::json!(1));
        assert_eq!(
            params_hash(&serde_json::Value::Object(ascending)).unwrap(),
            params_hash(&serde_json::Value::Object(descending)).unwrap(),
        );

        // And a map reached through an ordinary struct field, which is how a
        // derivation's parameters actually arrive.
        let mut one = HashMap::new();
        one.insert("b".to_owned(), 2);
        one.insert("a".to_owned(), 1);
        let mut other = HashMap::new();
        other.insert("a".to_owned(), 1);
        other.insert("b".to_owned(), 2);
        assert_eq!(params_hash(&one).unwrap(), params_hash(&other).unwrap());
    }

    #[test]
    fn nested_maps_are_canonical_too() {
        // Sorting only the top level would leave the bug one level down.
        let nested = |first: &str, second: &str| {
            let mut inner = serde_json::Map::new();
            inner.insert(first.to_owned(), serde_json::json!(first));
            inner.insert(second.to_owned(), serde_json::json!(second));
            let mut outer = serde_json::Map::new();
            outer.insert("inner".to_owned(), serde_json::Value::Object(inner));
            outer.insert("tail".to_owned(), serde_json::json!([1, 2]));
            serde_json::Value::Object(outer)
        };
        assert_eq!(
            params_hash(&nested("x", "y")).unwrap(),
            params_hash(&nested("y", "x")).unwrap()
        );
    }

    #[test]
    fn structurally_different_parameters_cannot_collide() {
        // Length prefixing is what stops concatenation ambiguity: without it
        // {"a":"b"} and {"ab":""} encode to the same bytes, and a cache-key
        // collision serves the wrong value rather than merely missing.
        let one = serde_json::json!({"a": "b"});
        let other = serde_json::json!({"ab": ""});
        assert_ne!(params_hash(&one).unwrap(), params_hash(&other).unwrap());

        // A string and a one-element array of it are different parameters.
        assert_ne!(
            params_hash(&serde_json::json!("a")).unwrap(),
            params_hash(&serde_json::json!(["a"])).unwrap()
        );
        // So are a number and its lexical form.
        assert_ne!(
            params_hash(&serde_json::json!(1)).unwrap(),
            params_hash(&serde_json::json!("1")).unwrap()
        );
        // And an absent field is not an explicit null.
        assert_ne!(
            params_hash(&serde_json::json!({})).unwrap(),
            params_hash(&serde_json::json!({"a": null})).unwrap()
        );
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
