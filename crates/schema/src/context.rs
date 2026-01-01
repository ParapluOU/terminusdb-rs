use crate::{
    json::ToJson, ContextDocumentation, SetCardinality, DEFAULT_BASE_STRING, DEFAULT_SCHEMA_STRING,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

// todo: use default Json derive and use field hints to rename to using the @
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct Context {
    pub schema: String,
    pub base: String,
    pub xsd: Option<String>,
    pub documentation: Option<ContextDocumentation>,
}

impl Context {
    pub fn woql() -> Self {
        Self {
            schema: "http://terminusdb.com/schema/woql#".to_string(),
            base: "terminusdb://woql/data/".to_string(),
            xsd: Some("http://www.w3.org/2001/XMLSchema#".to_string()),
            documentation: None,
        }
    }
}

impl ToJson for Context {
    fn to_map(&self) -> Map<String, Value> {
        let mut map = serde_json::Map::new();
        map.insert("@type".to_string(), "@context".to_string().into());
        // todo: if not empty?
        map.insert("@schema".to_string(), self.schema.clone().into());
        // todo: if not empty?
        map.insert("@base".to_string(), self.base.clone().into());
        if let Some(xsd) = &self.xsd {
            map.insert("xsd".to_string(), xsd.clone().into());
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
            xsd: Some("http://www.w3.org/2001/XMLSchema#".to_string()),
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
        xsd: Some("http://www.w3.org/2001/XMLSchema#".to_string()),
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
