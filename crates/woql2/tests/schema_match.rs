#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::fs::File;
    use std::io::BufReader;
    use std::path::Path;

    use serde_json::Value;
    use terminusdb_schema::{Schema, ToTDBSchema};
    use terminusdb_woql2::prelude::*; // Import all our WOQL types

    // Helper function to load and parse the woql.json spec.
    // The authoritative spec is the upstream terminus-schema woql.json mirrored
    // at docs/schemas/woql.json (refreshed alongside the docs mirror).
    fn load_woql_spec() -> HashMap<String, Value> {
        let path = format!(
            "{}/../../docs/schemas/woql.json",
            env!("CARGO_MANIFEST_DIR")
        );
        assert!(
            Path::new(&path).exists(),
            "WOQL spec not found at {path}; refresh docs/schemas/woql.json from upstream"
        );

        println!("Loading WOQL spec from: {}", path);

        let file = File::open(path).expect("Unable to open woql.json");
        let reader = BufReader::new(file);
        let stream = serde_json::Deserializer::from_reader(reader).into_iter::<Value>();

        let mut spec_map = HashMap::new();
        for value in stream {
            let obj = value.expect("Failed to parse JSON object from woql.json stream");
            if let Some(id) = obj.get("@id").and_then(|v| v.as_str()) {
                spec_map.insert(id.to_string(), obj);
            }
        }
        assert!(
            !spec_map.is_empty(),
            "Failed to load any types from woql.json"
        );
        spec_map
    }

    // Helper to extract expected properties from the spec JSON, ignoring '@' fields
    fn get_expected_properties(spec_json: &Value) -> HashMap<String, Value> {
        spec_json
            .as_object()
            .map(|obj| {
                obj.iter()
                    .filter(|(k, _)| !k.starts_with('@'))
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect()
            })
            .unwrap_or_default()
    }

    // Main assertion function (will be expanded)
    fn assert_schema_matches_spec<T: ToTDBSchema + 'static>(
        type_id: &str,
        spec_map: &HashMap<String, Value>,
    ) {
        let generated_schema = T::to_schema();
        let spec_json = spec_map
            .get(type_id)
            .unwrap_or_else(|| panic!("Type '{}' not found in woql.json spec", type_id));

        match generated_schema {
            Schema::Class { properties, .. } => {
                let expected_props = get_expected_properties(spec_json);
                assert_eq!(
                    properties.len(),
                    expected_props.len(),
                    "Mismatch in number of properties for Class '{}'. Expected: {:?}, Generated: {:?}",
                    type_id,
                    expected_props.keys().collect::<Vec<_>>(),
                    properties.iter().map(|p| &p.name).collect::<Vec<_>>(),
                );

                for prop in &properties {
                    let _expected_value = expected_props.get(&prop.name).unwrap_or_else(|| {
                        panic!(
                            "Generated property '{}' not found in spec for Class '{}'",
                            prop.name, type_id
                        )
                    });

                    // TODO: Add detailed comparison logic for property types (prop.class, prop.r#type)
                    println!("Checking property {}::{}", type_id, prop.name);
                }
            }
            Schema::Enum { id, values, .. } => {
                let expected_values = spec_json
                    .get("@value")
                    .and_then(|v| v.as_array())
                    .unwrap_or_else(|| {
                        panic!("Missing or invalid '@value' for Enum '{}' in spec", id)
                    });
                assert_eq!(
                    values.len(),
                    expected_values.len(),
                    "Mismatch in number of values for Enum '{}'",
                    id
                );
                for val in &values {
                    assert!(
                        expected_values
                            .iter()
                            .any(|ev| ev.as_str().map_or(false, |s| s == val)),
                        "Generated enum value '{}' not found in spec for Enum '{}'",
                        val,
                        id
                    );
                }
            }
            Schema::TaggedUnion { id, properties, .. } => {
                let expected_props = get_expected_properties(spec_json);
                assert_eq!(
                    properties.len(),
                    expected_props.len(),
                    "Mismatch in number of variants for TaggedUnion '{}'. Expected: {:?}, Generated: {:?}",
                    id,
                    expected_props.keys().collect::<Vec<_>>(),
                    properties.iter().map(|p| &p.name).collect::<Vec<_>>(),
                );
                for prop in &properties {
                    let _expected_value = expected_props.get(&prop.name).unwrap_or_else(|| {
                        panic!(
                            "Generated variant '{}' not found in spec for TaggedUnion '{}'",
                            prop.name, id
                        )
                    });
                    // TODO: Add detailed comparison logic for variant types (prop.class)
                    println!("Checking variant {}::{}", id, prop.name);
                }
            }
            Schema::OneOfClass { id, classes, .. } => {
                // Similar logic to TaggedUnion, but we check against possible classes
                // The spec might need a different structure to represent OneOf
                let spec_classes = spec_json
                    .get("@oneOf") // Assuming spec uses @oneOf for this
                    .and_then(|v| v.as_array())
                    .unwrap_or_else(|| {
                        panic!(
                            "Missing or invalid '@oneOf' property for OneOfClass '{}' in spec",
                            id
                        )
                    });

                assert_eq!(
                    classes.len(),
                    spec_classes.len(),
                    "Mismatch in number of classes for OneOfClass '{}'",
                    id
                );
                // TODO: Add more detailed check if needed, comparing class names/types
                println!("Checked OneOfClass '{}'", id);
            }
        }
    }

    // Example Test - will add more
    /*
    #[test]
    fn test_schema_generation_matches_spec() {
        let spec_map = load_woql_spec();
        assert_schema_matches_spec::<AddTriple>("AddTriple", &spec_map);
        assert_schema_matches_spec::<And>("And", &spec_map);
        // ... Add all other relevant WOQL types defined in prelude.rs
        // Example: Test a class
        // assert_schema_matches_spec::<Triple>("Triple", &spec_map);
        // Example: Test an enum
        // assert_schema_matches_spec::<Order>("Order", &spec_map);
        // Example: Test a tagged union
        // assert_schema_matches_spec::<ArithmeticExpression>("ArithmeticExpression", &spec_map);
    }
    */
    #[test]
    fn test_basic_types() {
        let spec_map = load_woql_spec();
        // assert_schema_matches_spec::<Value>("Value", &spec_map); // Commented out - serde_json::Value doesn't implement ToTDBSchema
        assert_schema_matches_spec::<NodeValue>("NodeValue", &spec_map);
        assert_schema_matches_spec::<DataValue>("DataValue", &spec_map);
        assert_schema_matches_spec::<Order>("Order", &spec_map); // Enum example
        assert_schema_matches_spec::<Triple>("Triple", &spec_map); // Class example
    }

    /// Two-way coverage: every non-abstract class in the upstream woql.json
    /// spec that inherits (directly) from `Query` must have a corresponding
    /// `Query` enum variant, and vice versa. Upstream additions fail the
    /// missing-variant direction; local inventions fail the extra-variant one.
    #[test]
    fn test_query_enum_covers_spec() {
        let spec_map = load_woql_spec();

        // Spec-side: classes whose @inherits mentions "Query".
        let mut spec_classes: Vec<String> = spec_map
            .iter()
            .filter(|(id, v)| {
                *id != "Query"
                    && match v.get("@inherits") {
                        Some(Value::String(s)) => s == "Query",
                        Some(Value::Array(a)) => {
                            a.iter().any(|x| x.as_str() == Some("Query"))
                        }
                        _ => false,
                    }
            })
            .map(|(id, _)| id.clone())
            .collect();
        spec_classes.sort();

        // Rust-side: the Query tagged-union's variant target classes.
        let schema = Query::to_schema();
        let mut variant_classes: Vec<String> = match &schema {
            Schema::TaggedUnion { properties, .. } => {
                properties.iter().map(|p| p.class.clone()).collect()
            }
            other => panic!("Query schema is not a TaggedUnion: {:?}", other),
        };
        variant_classes.sort();

        let missing: Vec<_> = spec_classes
            .iter()
            .filter(|c| !variant_classes.contains(c))
            .collect();
        let extra: Vec<_> = variant_classes
            .iter()
            .filter(|c| !spec_classes.contains(c))
            .collect();

        assert!(
            missing.is_empty() && extra.is_empty(),
            "Query enum out of sync with woql.json spec.\n   spec classes missing a variant: {:?}\n   variants with no spec class: {:?}",
            missing,
            extra
        );
    }
}
