//! Parse TerminusDB schema-graph documents (the authored JSON, as emitted by
//! `Schema::to_json`) into a small raw model the catalog builder consumes.
//!
//! We parse the JSON ourselves rather than deserializing into
//! [`terminusdb_schema::Schema`], because that type's derived `Deserialize` is
//! explicitly *not* compliant with TerminusDB's schema JSON (it exists only for
//! internal RPC — see `crates/schema/src/schema/schema.rs`). We only need the
//! subset relevant to tables/columns, so a focused parser is simpler and safer.

use serde_json::Value;

use crate::error::{Result, SqlError};

/// The parsed schema graph: concrete classes + enums.
#[derive(Debug, Clone, Default)]
pub struct RawSchema {
    pub classes: Vec<RawClass>,
    pub enums: Vec<RawEnum>,
}

/// A `Class` or `TaggedUnion` document.
#[derive(Debug, Clone)]
pub struct RawClass {
    pub id: String,
    pub is_abstract: bool,
    pub is_subdocument: bool,
    pub inherits: Vec<String>,
    pub properties: Vec<RawProperty>,
}

/// One property (edge / field) of a class.
#[derive(Debug, Clone)]
pub struct RawProperty {
    /// The field name as authored (e.g. `name`, `employer`).
    pub name: String,
    /// The range: an `xsd:*` / `sys:*` datatype, or a class id.
    pub class: String,
    /// The type family, if the value was a container/optional object.
    pub family: Option<Family>,
    /// Forced nullable regardless of family — set for `TaggedUnion` variants and
    /// `@oneOf` members, which are mutually exclusive and thus optional.
    pub force_nullable: bool,
}

/// Property type families we distinguish (cardinality detail is irrelevant to v1
/// since any multi-valued property is rejected).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    Optional,
    Set,
    List,
    Array,
}

impl Family {
    /// True for the container families (rejected in v1 as multi-valued).
    pub fn is_multivalued(self) -> bool {
        matches!(self, Family::Set | Family::List | Family::Array)
    }

    /// The container name for an omission reason.
    pub fn container_name(self) -> &'static str {
        match self {
            Family::Set => "set",
            Family::List => "list",
            Family::Array => "array",
            Family::Optional => "optional",
        }
    }
}

/// An `Enum` document.
#[derive(Debug, Clone)]
pub struct RawEnum {
    pub id: String,
    pub values: Vec<String>,
}

/// Parse a list of schema-graph documents into [`RawSchema`]. The `@context`
/// document is skipped.
pub fn parse_schema(docs: &[Value]) -> Result<RawSchema> {
    let mut out = RawSchema::default();
    for doc in docs {
        let obj = match doc.as_object() {
            Some(o) => o,
            None => continue, // non-object entries are not schema documents
        };
        let ty = obj.get("@type").and_then(Value::as_str).unwrap_or("");
        match ty {
            "@context" => continue,
            "Enum" => out.enums.push(parse_enum(obj)?),
            "Class" => out.classes.push(parse_class(obj, false)?),
            "TaggedUnion" => out.classes.push(parse_class(obj, true)?),
            other => {
                // Unknown top-level document type — skip rather than fail the whole
                // load, but this should not normally occur in a schema graph.
                let _ = other;
            }
        }
    }
    Ok(out)
}

fn parse_enum(obj: &serde_json::Map<String, Value>) -> Result<RawEnum> {
    let id = required_str(obj, "@id")?;
    let values = obj
        .get("@value")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    Ok(RawEnum { id, values })
}

fn parse_class(obj: &serde_json::Map<String, Value>, tagged_union: bool) -> Result<RawClass> {
    let id = required_str(obj, "@id")?;
    let is_abstract = obj.contains_key("@abstract");
    let is_subdocument = obj.contains_key("@subdocument");
    let inherits = parse_inherits(obj.get("@inherits"));

    let mut properties = Vec::new();
    for (key, val) in obj {
        if key.starts_with('@') {
            // Handle the one structural key that introduces properties.
            if key == "@oneOf" {
                parse_one_of(val, &mut properties)?;
            }
            continue;
        }
        properties.push(parse_property(key, val, tagged_union)?);
    }
    Ok(RawClass {
        id,
        is_abstract,
        is_subdocument,
        inherits,
        properties,
    })
}

fn parse_property(name: &str, val: &Value, force_nullable: bool) -> Result<RawProperty> {
    match val {
        // Bare string range => exactly-one of that class/datatype.
        Value::String(class) => Ok(RawProperty {
            name: name.to_string(),
            class: class.clone(),
            family: None,
            force_nullable,
        }),
        // Container / optional object.
        Value::Object(o) => {
            let fam = o.get("@type").and_then(Value::as_str);
            let family = match fam {
                Some("Optional") => Family::Optional,
                Some("Set") => Family::Set,
                Some("List") => Family::List,
                Some("Array") => Family::Array,
                other => {
                    return Err(SqlError::SchemaParse(format!(
                        "property `{name}` has unrecognised type family {other:?}"
                    )))
                }
            };
            let class = o
                .get("@class")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    SqlError::SchemaParse(format!("property `{name}` is missing `@class`"))
                })?
                .to_string();
            Ok(RawProperty {
                name: name.to_string(),
                class,
                family: Some(family),
                force_nullable,
            })
        }
        other => Err(SqlError::SchemaParse(format!(
            "property `{name}` has unexpected value {other}"
        ))),
    }
}

/// `@oneOf` is an array of objects, each mapping property name → range. All such
/// properties are mutually exclusive, hence nullable.
fn parse_one_of(val: &Value, out: &mut Vec<RawProperty>) -> Result<()> {
    let groups = val.as_array().ok_or_else(|| {
        SqlError::SchemaParse("`@oneOf` must be an array of property groups".into())
    })?;
    for group in groups {
        let obj = group.as_object().ok_or_else(|| {
            SqlError::SchemaParse("`@oneOf` entries must be objects".into())
        })?;
        for (name, v) in obj {
            if name.starts_with('@') {
                continue;
            }
            out.push(parse_property(name, v, true)?);
        }
    }
    Ok(())
}

fn parse_inherits(val: Option<&Value>) -> Vec<String> {
    match val {
        Some(Value::String(s)) => vec![s.clone()],
        Some(Value::Array(arr)) => arr
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect(),
        _ => Vec::new(),
    }
}

fn required_str(obj: &serde_json::Map<String, Value>, key: &str) -> Result<String> {
    obj.get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| SqlError::SchemaParse(format!("schema document missing `{key}`")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_class_with_families() {
        let docs = vec![
            json!({"@type": "@context", "@base": "i/", "@schema": "s#"}),
            json!({
                "@id": "Person", "@type": "Class",
                "name": "xsd:string",
                "age": {"@type": "Optional", "@class": "xsd:integer"},
                "employer": "Company",
                "friends": {"@type": "Set", "@class": "Person"}
            }),
            json!({"@id": "Company", "@type": "Class", "name": "xsd:string"}),
            json!({"@id": "Color", "@type": "Enum", "@value": ["red", "green"]}),
        ];
        let raw = parse_schema(&docs).unwrap();
        assert_eq!(raw.classes.len(), 2);
        assert_eq!(raw.enums.len(), 1);
        assert_eq!(raw.enums[0].values, vec!["red", "green"]);

        let person = raw.classes.iter().find(|c| c.id == "Person").unwrap();
        let byname = |n: &str| person.properties.iter().find(|p| p.name == n).unwrap();
        assert_eq!(byname("name").class, "xsd:string");
        assert!(byname("name").family.is_none());
        assert_eq!(byname("age").family, Some(Family::Optional));
        assert_eq!(byname("employer").class, "Company");
        assert_eq!(byname("friends").family, Some(Family::Set));
    }

    #[test]
    fn parses_abstract_subdocument_and_inherits() {
        let docs = vec![
            json!({"@id": "Named", "@type": "Class", "@abstract": [], "name": "xsd:string"}),
            json!({"@id": "Addr", "@type": "Class", "@subdocument": [], "@key": {"@type": "Random"}, "city": "xsd:string"}),
            json!({"@id": "Person", "@type": "Class", "@inherits": ["Named"], "age": "xsd:integer"}),
        ];
        let raw = parse_schema(&docs).unwrap();
        let named = raw.classes.iter().find(|c| c.id == "Named").unwrap();
        assert!(named.is_abstract);
        let addr = raw.classes.iter().find(|c| c.id == "Addr").unwrap();
        assert!(addr.is_subdocument);
        let person = raw.classes.iter().find(|c| c.id == "Person").unwrap();
        assert_eq!(person.inherits, vec!["Named"]);
    }

    #[test]
    fn tagged_union_props_are_forced_nullable() {
        let docs = vec![json!({
            "@id": "Tree", "@type": "TaggedUnion",
            "leaf": "xsd:string", "node": "Tree"
        })];
        let raw = parse_schema(&docs).unwrap();
        let tree = &raw.classes[0];
        assert!(tree.properties.iter().all(|p| p.force_nullable));
    }
}
