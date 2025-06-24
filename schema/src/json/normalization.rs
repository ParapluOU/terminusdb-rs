use rayon::prelude::*;
use serde::Serialize;
use serde_canonical_json;
use serde_canonical_json::CanonicalFormatter;
use serde_json::Serializer;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::Write;

type JSONWithRefs = serde_json::Value;
type JSONDenormalized = serde_json::Value;
type JSONFragment = serde_json::Map<String, JSONWithRefs>;
type JSONRefsAndFragments = serde_json::Value;

type JSONFragmentIndex = HashMap<String, JSONFragment>;
type JSONFragmentRef = serde_json::Map<String, serde_json::Value>;

/// https://terminusdb.com/docs/index/terminusx-db/reference-guides/document-interface#capturing-ids-while-inserting-or-replacing-documents
pub fn normalize(instance: JSONDenormalized) -> JSONRefsAndFragments {
    if is_normalized(&instance) {
        return instance;
    }

    println!("normalizing TDB instance...");

    let mut index = HashMap::new();

    // todo: skip normalizing if already normalized
    let refs = _normalize(&mut index, instance);

    serde_json::Value::Array(
        index
            .into_values()
            .map(serde_json::Value::Object)
            // .chain(vec!(refs))
            .collect(),
    )
}

// normalize tdb instance file from disk
// pub fn normalize_file(path: &str) -> JSONRefsAndFragments {
//     normalize(parture_common::json_or_zip_to_value(path).unwrap())
// }

// read denormalized tdb instance file from disk, normalize, and then write to putput
// pub fn normalize_file_to_file(input_path: &str, output_path: &str) {
//     let mut output = std::fs::File::create(output_path).expect("error creating file");
//     output.write_all(serde_json::to_string(&normalize_file(input_path)).unwrap().as_bytes());
// }

pub fn _normalize_arr(
    index: &mut JSONFragmentIndex,
    reftree: Vec<JSONWithRefs>,
) -> Vec<JSONWithRefs> {
    let mut trees = vec![];

    for f in reftree {
        trees.push(_normalize(index, f));
    }

    // todo: make paralell
    // let trees = reftree
    //     .into_par_iter()
    //     .map(|f| _normalize(index, f))
    //     .collect();

    // (index, reftree
    //     .into_iter()
    //     .map(|v|
    //         _normalize(index, v).1)
    //     .collect())

    trees
}

pub fn _normalize(fragments: &mut JSONFragmentIndex, tree: JSONWithRefs) -> JSONWithRefs {
    match tree {
        JSONWithRefs::Array(items) => {
            // todo: mutate instead?
            JSONWithRefs::Array(_normalize_arr(fragments, items))
        }
        // we replace every object with a hash reference
        JSONWithRefs::Object(data) => {
            // first normalize every sub-object
            let normalized = data
                .into_iter()
                .map(|(key, value)| (key, _normalize(fragments, value)))
                .collect();

            // then index this object
            JSONWithRefs::Object(index_fragment(fragments, normalized))
        }
        // noop
        _ => tree,
    }
}

// todo: make more robust
pub fn is_normalized(instance: &JSONDenormalized) -> bool {
    let ser = serde_json::to_string(instance).expect("could not serialize in is_normalized()");

    ser.contains("@ref") && ser.contains("@capture")
}

pub fn index_fragment(
    index: &mut JSONFragmentIndex,
    mut fragment: serde_json::Map<String, JSONWithRefs>,
) -> JSONFragmentRef {
    let hash = hash_fragment(&fragment);
    // if the fragment already exists, take the existing ID
    match index.get(&hash) {
        None => {
            let id = format!("id-{}", index.len());
            fragment.insert("@capture".to_string(), id.clone().into());
            index.insert(hash.clone(), fragment);
            new_ref(id.clone())
        }
        Some(frag) => new_ref(
            frag.get("@capture")
                .expect("unable to retrieve @capture variable")
                .as_str()
                .unwrap()
                .to_string(),
        ),
    }
}

pub fn new_ref(hash: String) -> JSONFragmentRef {
    let mut map = serde_json::Map::new();
    map.insert("@ref".to_string(), hash.into());
    map
}

pub fn hash_fragment(fragment: &JSONFragment) -> String {
    // Create a canonical serializer to ensure key order is consistent
    let mut ser = Serializer::with_formatter(Vec::new(), CanonicalFormatter::new());

    // Serialize the fragment to canonical JSON format
    let value = serde_json::Value::Object(fragment.clone());
    value
        .serialize(&mut ser)
        .expect("Failed to serialize JSON fragment");

    // Convert serialized bytes to UTF-8 string
    let canonical_json =
        String::from_utf8(ser.into_inner()).expect("Failed to convert canonical JSON to string");

    // Create hash of the canonical JSON
    let mut hasher = Sha256::new();
    hasher.update(canonical_json.as_bytes());
    let result = hasher.finalize();

    // Convert to hex string
    format!("{:x}", result)
}

#[test]
fn test_normalized() {
    let beat = serde_json::json!({
        "duration": "sixteenth",
        "notes": [
            {
                "pitch": 8
            },
            {
                "pitch": 9
            },
            {
                "pitch": 8
            }
        ]
    });

    let normalized = normalize(beat);

    dbg!(normalized);
}

#[test]
fn test_hash_fragment_order_independence() {
    // Create two fragments with the same key-value pairs but different key order
    let mut fragment1 = serde_json::Map::new();
    fragment1.insert("a".to_string(), serde_json::json!(1));
    fragment1.insert("b".to_string(), serde_json::json!(2));
    fragment1.insert("c".to_string(), serde_json::json!(3));

    let mut fragment2 = serde_json::Map::new();
    fragment2.insert("c".to_string(), serde_json::json!(3));
    fragment2.insert("a".to_string(), serde_json::json!(1));
    fragment2.insert("b".to_string(), serde_json::json!(2));

    // The hash should be the same regardless of insertion order
    let hash1 = hash_fragment(&fragment1);
    let hash2 = hash_fragment(&fragment2);

    assert_eq!(
        hash1, hash2,
        "Hashes should be equal regardless of key order"
    );

    // Verify changing a value produces a different hash
    let mut fragment3 = fragment1.clone();
    fragment3.insert("b".to_string(), serde_json::json!(4)); // Changed value

    let hash3 = hash_fragment(&fragment3);
    assert_ne!(hash1, hash3, "Hashes should differ for different values");
}
