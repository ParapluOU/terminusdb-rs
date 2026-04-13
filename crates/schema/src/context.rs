use crate::{
    json::ToJson, ContextDocumentation, SetCardinality, DEFAULT_BASE_STRING, DEFAULT_SCHEMA_STRING,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

/// TerminusDB context object (`@type: "@context"`).
///
/// Defines namespace prefixes for the schema. Any key that doesn't start with `@`
/// is a prefix→URI mapping (per TDB docs: guides/reference-guides/schema.md).
/// For example, `"xsd" → "http://www.w3.org/2001/XMLSchema#"` enables `xsd:string`.
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct Context {
    pub schema: String,
    pub base: String,
    /// Namespace prefix mappings. Keys are prefixes (e.g., "xsd", "xlink"),
    /// values are URIs (e.g., "http://www.w3.org/2001/XMLSchema#").
    pub prefixes: BTreeMap<String, String>,
    pub documentation: Option<ContextDocumentation>,
}

impl Context {
    pub fn woql() -> Self {
        Self {
            schema: "http://terminusdb.com/schema/woql#".to_string(),
            base: "terminusdb://woql/data/".to_string(),
            prefixes: BTreeMap::from([(
                "xsd".to_string(),
                "http://www.w3.org/2001/XMLSchema#".to_string(),
            )]),
            documentation: None,
        }
    }
}

impl ToJson for Context {
    fn to_map(&self) -> Map<String, Value> {
        let mut map = serde_json::Map::new();
        map.insert("@type".to_string(), "@context".to_string().into());
        map.insert("@schema".to_string(), self.schema.clone().into());
        map.insert("@base".to_string(), self.base.clone().into());
        for (prefix, uri) in &self.prefixes {
            map.insert(prefix.clone(), uri.clone().into());
        }
        if let Some(doc) = &self.documentation {
            map.insert("@documentation".to_string(), doc.to_map().into());
        }
        map
    }
}

impl Default for Context {
    fn default() -> Self {
        Context {
            schema: DEFAULT_SCHEMA_STRING.to_string(),
            base: DEFAULT_BASE_STRING.to_string(),
            prefixes: BTreeMap::from([(
                "xsd".to_string(),
                "http://www.w3.org/2001/XMLSchema#".to_string(),
            )]),
            documentation: None,
        }
    }
}

#[test]
fn test_context_json() {
    // example from https://terminusdb.com/docs/index/terminusx-db/reference-guides/schema#context-object
    let ctx = Context {
        schema: "http://terminusdb.com/schema/woql#".to_string(),
        base: "terminusdb://woql/data/".to_string(),
        prefixes: BTreeMap::from([(
            "xsd".to_string(),
            "http://www.w3.org/2001/XMLSchema#".to_string(),
        )]),
        documentation: Some(ContextDocumentation {
            title: "WOQL schema".to_string(),
            authors: vec!["Gavin Mendel-Gleason".to_string()],
            description: "The WOQL schema providing a complete specification of the WOQL syntax."
                .to_string(),
        }),
    };

    assert_eq!(
        ctx.to_json(),
        json!({
            "@type": "@context",
            "@schema": "http://terminusdb.com/schema/woql#",
            "@base": "terminusdb://woql/data/",
            "xsd": "http://www.w3.org/2001/XMLSchema#",
            "@documentation": {
                "@title": "WOQL schema",
                "@authors": ["Gavin Mendel-Gleason"],
                "@description": "The WOQL schema providing a complete specification of the WOQL syntax."
            }
        })
    )
}

#[test]
fn test_context_with_custom_prefixes() {
    let ctx = Context {
        schema: "http://niso.org/schema#".to_string(),
        base: "http://niso.org/data/".to_string(),
        prefixes: BTreeMap::from([
            ("xsd".to_string(), "http://www.w3.org/2001/XMLSchema#".to_string()),
            ("xlink".to_string(), "http://www.w3.org/1999/xlink#".to_string()),
            ("xml".to_string(), "http://www.w3.org/XML/1998/namespace#".to_string()),
        ]),
        documentation: None,
    };

    let json = ctx.to_json();
    assert_eq!(json["xlink"], "http://www.w3.org/1999/xlink#");
    assert_eq!(json["xml"], "http://www.w3.org/XML/1998/namespace#");
    assert_eq!(json["xsd"], "http://www.w3.org/2001/XMLSchema#");
}
