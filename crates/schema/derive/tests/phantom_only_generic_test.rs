#![cfg(feature = "generic-derive")]

use serde::{Deserialize, Serialize};
use std::marker::PhantomData;
use terminusdb_schema::*;
use terminusdb_schema_derive::TerminusDBModel;

// User-defined trait that doesn't require any TerminusDB traits
pub trait MyConstraint: Clone + Serialize + Send {
    fn process(&self);
}

// Simple type that implements the user trait but NOT TerminusDB traits
#[derive(Clone, Serialize)]
pub struct SimpleMarker;

impl MyConstraint for SimpleMarker {
    fn process(&self) {
        println!("Processing marker");
    }
}

// This should work now - T is only used in PhantomData, not as an actual field
#[derive(Debug, Clone, TerminusDBModel)]
pub struct PhantomOnlyContainer<T: MyConstraint> {
    pub id: String,
    pub count: usize,
    pub name: String,
    // T is only used here, not as an actual field
    // PhantomData is skipped by the TDB derive (it's a zero-sized type)
    _phantom: PhantomData<T>,
}

// Multiple phantom markers
#[derive(Debug, Clone, TerminusDBModel)]
pub struct MultiPhantom<T, U, V>
where
    T: Send,
    U: Clone + Send,
    V: Send + 'static,
{
    pub data: String,
    _t: PhantomData<T>,
    _u: PhantomData<U>,
    _v: PhantomData<V>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_phantom_only_no_terminusdb_traits() {
        // SimpleMarker doesn't implement any TerminusDB traits
        // This should compile because T is only in PhantomData
        let container = PhantomOnlyContainer::<SimpleMarker> {
            id: "test-1".to_string(),
            count: 42,
            name: "Test Container".to_string(),
            _phantom: PhantomData,
        };

        // Test schema generation
        let schema = <PhantomOnlyContainer<SimpleMarker> as ToTDBSchema>::to_schema();
        assert_eq!(schema.class_name(), "PhantomOnlyContainer<SimpleMarker>");

        // Verify PhantomData field is not in schema
        if let Some(props) = <PhantomOnlyContainer<SimpleMarker> as ToTDBSchema>::properties() {
            assert_eq!(props.len(), 3); // id, count, name (not _phantom)
            assert!(props.iter().any(|p| p.name == "id"));
            assert!(props.iter().any(|p| p.name == "count"));
            assert!(props.iter().any(|p| p.name == "name"));
            assert!(!props.iter().any(|p| p.name == "_phantom"));
        }

        // Test instance generation
        let instance = container.to_instance(None);
        assert_eq!(instance.properties.len(), 3);
        assert!(!instance.properties.contains_key("_phantom"));
    }

    #[test]
    fn test_multi_phantom_markers() {
        // Types that don't implement TerminusDB traits
        #[derive(Clone)]
        struct MarkerA;
        #[derive(Clone)]
        struct MarkerB;
        #[derive(Clone)]
        struct MarkerC;

        let multi = MultiPhantom::<MarkerA, MarkerB, MarkerC> {
            data: "test data".to_string(),
            _t: PhantomData,
            _u: PhantomData,
            _v: PhantomData,
        };

        let schema = <MultiPhantom<MarkerA, MarkerB, MarkerC> as ToTDBSchema>::to_schema();
        assert_eq!(
            schema.class_name(),
            "MultiPhantom<MarkerA, MarkerB, MarkerC>"
        );

        // Only the data field should be in the schema
        if let Some(props) = <MultiPhantom<MarkerA, MarkerB, MarkerC> as ToTDBSchema>::properties()
        {
            assert_eq!(props.len(), 1);
            assert!(props.iter().any(|p| p.name == "data"));
        }

        let instance = multi.to_instance(None);
        assert_eq!(instance.properties.len(), 1);
        assert!(instance.properties.contains_key("data"));
    }
}
