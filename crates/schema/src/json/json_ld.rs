//! JSON-LD helpers for working with raw TerminusDB JSON documents.
//!
//! These utilities extract standard JSON-LD fields (`@type`, `@id`) and
//! handle TerminusDB conventions (TaggedUnion child wrappers, document
//! references) without requiring fully-typed schema models.

use crate::TdbIRI;
use serde_json::Value;

/// Extract the `@type` field from a JSON-LD object.
///
/// Returns an error if the value is not an object or `@type` is missing/non-string.
pub fn get_type(json: &Value) -> Result<&str, String> {
    let obj = json.as_object()
        .ok_or_else(|| format!("Expected JSON object, got {}", json))?;
    obj.get("@type")
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!(
            "Missing @type on JSON-LD object with keys: {:?}",
            obj.keys().collect::<Vec<_>>()
        ))
}

/// Extract the `@id` field from a JSON-LD object.
///
/// Returns an error if the value is not an object or `@id` is missing/non-string.
pub fn get_id(json: &Value) -> Result<&str, String> {
    let obj = json.as_object()
        .ok_or_else(|| format!("Expected JSON object, got {}", json))?;
    obj.get("@id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!(
            "Missing @id on JSON-LD object with keys: {:?}",
            obj.keys().collect::<Vec<_>>()
        ))
}

/// Check if a TDB type name represents a TaggedUnion child wrapper.
///
/// TerminusDB convention: ordered children use TaggedUnion types whose
/// names end with `Child` (e.g. `SecChild`, `ParagraphChild`).
pub fn is_tagged_union_child(type_name: &str) -> bool {
    type_name.ends_with("Child")
}

/// Extract the active variant from a TaggedUnion JSON object.
///
/// TaggedUnion objects have exactly one non-`@`-prefixed field — the active
/// variant. Returns `(variant_key, variant_value)` or `None` if no variant found.
pub fn active_variant<'a>(obj: &'a serde_json::Map<String, Value>) -> Option<(&'a str, &'a Value)> {
    obj.iter()
        .find(|(key, _)| !key.starts_with('@'))
        .map(|(k, v)| (k.as_str(), v))
}

/// Check if a string value is a TerminusDB document reference.
///
/// Uses `TdbIRI::parse` for proper validation rather than heuristic pattern
/// matching. A document reference has the form `Type/id` where `Type` starts
/// with an uppercase letter.
///
/// # Examples
/// ```
/// use terminusdb_schema::json_ld::is_document_ref;
///
/// assert!(is_document_ref("Sec/abc123"));
/// assert!(is_document_ref("Standard/1870093e790d23e22af2822be943801619babc6e906dc5b8c42eeb4d358b74d5"));
/// assert!(!is_document_ref("plain text"));
/// assert!(!is_document_ref("en"));
/// assert!(!is_document_ref("123"));
/// ```
pub fn is_document_ref(s: &str) -> bool {
    TdbIRI::parse(s).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_get_type() {
        let obj = json!({"@type": "Person", "name": "Alice"});
        assert_eq!(get_type(&obj).unwrap(), "Person");

        let no_type = json!({"name": "Alice"});
        assert!(get_type(&no_type).is_err());

        let not_obj = json!("string");
        assert!(get_type(&not_obj).is_err());
    }

    #[test]
    fn test_get_id() {
        let obj = json!({"@id": "Person/123", "@type": "Person"});
        assert_eq!(get_id(&obj).unwrap(), "Person/123");

        let no_id = json!({"@type": "Person"});
        assert!(get_id(&no_id).is_err());
    }

    #[test]
    fn test_is_tagged_union_child() {
        assert!(is_tagged_union_child("SecChild"));
        assert!(is_tagged_union_child("ParagraphTypeChild"));
        assert!(!is_tagged_union_child("Sec"));
        assert!(!is_tagged_union_child("ChildElement"));
    }

    #[test]
    fn test_active_variant() {
        let obj = json!({"@type": "SecChild", "sec": {"@type": "Sec"}});
        let map = obj.as_object().unwrap();
        let (key, _val) = active_variant(map).unwrap();
        assert_eq!(key, "sec");

        let text_variant = json!({"@type": "PChild", "text": "hello"});
        let map = text_variant.as_object().unwrap();
        let (key, val) = active_variant(map).unwrap();
        assert_eq!(key, "text");
        assert_eq!(val, "hello");
    }

    #[test]
    fn test_is_document_ref() {
        assert!(is_document_ref("Sec/abc123"));
        assert!(is_document_ref("Standard/1870093e790d23e22af2822be943801619babc6e906dc5b8c42eeb4d358b74d5"));
        assert!(!is_document_ref("plain text"));
        assert!(!is_document_ref("en"));
        assert!(!is_document_ref(""));
    }
}
