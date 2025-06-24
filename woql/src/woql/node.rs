use super::PredicateURI;
use crate::*;
use terminusdb_schema::{Schema, ToTDBSchema};
use std::collections::HashSet;

#[macro_export]
macro_rules! node {
    ($name:ident) => {
        node::<$name>()
    };
}

/// URI pointing to resource such as http://parture/schema/Song
// #[derive(Clone, Debug)]
// pub struct NodeURI(String);

newtype!({
    name: NodeURI,
    type: String,
    schemaclass: STRING
});

impl ToTDBSchema for NodeURI {
    fn to_schema() -> Schema {
        unimplemented!()
    }

    fn to_schema_tree() -> Vec<Schema> {
        unimplemented!()
    }

    fn to_schema_tree_mut(collection: &mut HashSet<Schema>) {
        let schema = <NodeURI as ToTDBSchema>::to_schema();

        // Only add if not already in the collection
        if !collection
            .iter()
            .any(|s| s.class_name() == schema.class_name())
        {
            collection.insert(schema);

            // Since this is a concrete type, no inner types need processing
        }
    }
}

// reference to node
pub fn node<N: ToTDBSchema>() -> NodeURI {
    let schema = N::to_schema();
    NodeURI(schema.class_name().clone())
}

// impl_newtype_derive!(NodeURI => String);

impl ToCLIQueryAST for NodeURI {
    fn to_ast(&self) -> String {
        format!("'{}'", &self.0)
    }
}

impl ToRESTQuery for NodeURI {
    fn to_rest_query_json(&self) -> serde_json::Value {
        self.0.clone().into()
    }
}

impl std::convert::From<PredicateURI> for NodeURI {
    fn from(p: PredicateURI) -> Self {
        Self(p.into())
    }
}

impl std::convert::From<&str> for NodeURI {
    fn from(p: &str) -> Self {
        Self(p.to_string())
    }
}
