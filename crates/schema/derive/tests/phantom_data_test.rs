#![cfg(feature = "generic-derive")]

use serde::{Deserialize, Serialize};
use std::marker::PhantomData;
use terminusdb_schema::{Schema, ToTDBInstance, ToTDBSchema};
use terminusdb_schema_derive::TerminusDBModel;

// Simple type to use as a phantom parameter
#[derive(Debug, Clone, TerminusDBModel, Serialize, Deserialize)]
struct Tag {
    name: String,
}

// Generic struct with PhantomData field
// Note: We need Send bounds for ToTDBInstances implementation
#[derive(Debug, Clone, TerminusDBModel)]
struct GenericWithPhantom<T: Send> {
    id: String,
    data: String,
    // PhantomData field - should not appear in schema or instance
    _phantom: PhantomData<T>,
}

// Another example with multiple generic parameters
#[derive(Debug, Clone, TerminusDBModel)]
struct ComplexGeneric<T: Send, U: Send> {
    id: String,
    value: f64,
    _marker_t: PhantomData<T>,
    _marker_u: PhantomData<U>,
}

#[test]
fn test_phantom_data_fields_are_skipped() {
    // Test that PhantomData fields don't appear in schema
    let schema = <GenericWithPhantom<Tag> as ToTDBSchema>::to_schema();
    
    match schema {
        Schema::Class { properties, .. } => {
            // Should only have 2 properties: id and data
            assert_eq!(properties.len(), 2);
            
            let prop_names: Vec<_> = properties
                .iter()
                .map(|p| p.name.as_str())
                .collect();
            
            assert!(prop_names.contains(&"id"));
            assert!(prop_names.contains(&"data"));
            assert!(!prop_names.contains(&"_phantom"));
        }
        _ => panic!("Expected a Class schema"),
    }
}

#[test]
fn test_phantom_data_instance_generation() {
    let obj = GenericWithPhantom::<Tag> {
        id: "test-1".to_string(),
        data: "test data".to_string(),
        _phantom: PhantomData,
    };
    
    let instance = obj.to_instance(None);
    
    // PhantomData field should not appear in properties
    assert_eq!(instance.properties.len(), 2);
    assert!(instance.properties.contains_key("id"));
    assert!(instance.properties.contains_key("data"));
    assert!(!instance.properties.contains_key("_phantom"));
}

#[test]
fn test_multiple_phantom_data_fields() {
    let schema = <ComplexGeneric<Tag, String> as ToTDBSchema>::to_schema();
    
    match schema {
        Schema::Class { properties, .. } => {
            // Should only have 2 properties: id and value
            assert_eq!(properties.len(), 2);
            
            let prop_names: Vec<_> = properties
                .iter()
                .map(|p| p.name.as_str())
                .collect();
            
            assert!(prop_names.contains(&"id"));
            assert!(prop_names.contains(&"value"));
            assert!(!prop_names.contains(&"_marker_t"));
            assert!(!prop_names.contains(&"_marker_u"));
        }
        _ => panic!("Expected a Class schema"),
    }
}

#[test]
fn test_phantom_data_no_bounds_needed() {
    // This test verifies that T in PhantomData<T> doesn't need any trait bounds
    // by using a simple type that doesn't implement TerminusDBModel
    struct SimpleMarker;
    
    #[derive(Debug, Clone, TerminusDBModel)]
    struct WithSimplePhantom {
        id: String,
        _marker: PhantomData<SimpleMarker>,
    }
    
    let obj = WithSimplePhantom {
        id: "test".to_string(),
        _marker: PhantomData,
    };
    
    // Should compile and work even though SimpleMarker has no TerminusDB traits
    let instance = obj.to_instance(None);
    assert_eq!(instance.properties.len(), 1);
    assert!(instance.properties.contains_key("id"));
}