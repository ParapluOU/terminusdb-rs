use serde_json::{Map, Value};
use std::collections::HashMap;

pub fn flatten_json(nested: &Value) -> (String, HashMap<String, Value>) {
    let mut flat_map = HashMap::new();

    // unpack single-item array
    match nested {
        Value::Array(arr) => {
            if arr.len() == 1 {
                return flatten_json(&arr[0]);
            }
        }
        _ => {}
    }

    let err = format!("no root @id returned for top-level object: {:#?}", &nested);
    let root_id = flatten_object(&mut nested.clone(), &mut flat_map);
    (
        root_id
            .expect(&err)
            .as_str()
            .expect("unable to return root @id as String")
            .to_string(),
        flat_map,
    )
}

fn flatten_object(obj: &mut Value, flat_map: &mut HashMap<String, Value>) -> Option<Value> {
    match obj {
        Value::Object(map) => {
            if let (Some(id), Some(ty)) = (map.get("@id"), map.get("@type")) {
                if let (Some(id), Some(ty)) = (id.as_str(), ty.as_str()) {
                    // Extract the object with only @id and @type
                    let id_string = id.to_string();
                    let ty_string = ty.to_string();

                    let rust_hash = id_string.split("/").collect::<Vec<&str>>()[1].to_string();

                    // create reference to this objec that the caller will have to start using
                    let mut extracted = Map::new();
                    extracted.insert("@id".to_string(), Value::String(id_string.clone()));
                    extracted.insert("@type".to_string(), Value::String(ty_string.clone()));

                    // Replace nested objects with their @id and @type
                    for (key, value) in map.iter_mut() {
                        if !key.starts_with('@') {
                            *value = flatten_object(value, flat_map).unwrap_or(value.clone());
                        }
                    }

                    flat_map.insert(rust_hash.clone(), Value::Object(map.clone()));

                    // Return only the reference to the object
                    // return Some(Value::Object(extracted));
                    return Some(Value::String(rust_hash));
                }
            } else {
                // Traverse nested dictionaries
                for (_, value) in map.iter_mut() {
                    *value = flatten_object(value, flat_map).unwrap_or(value.clone());
                }
            }
        }
        Value::Array(arr) => {
            // Traverse lists
            for i in 0..arr.len() {
                arr[i] = flatten_object(&mut arr[i], flat_map).unwrap_or(arr[i].clone());
            }
        }
        _ => {}
    }

    None
}
