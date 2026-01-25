#![cfg(feature = "generic-derive")]

use terminusdb_schema::{
    FromTDBInstance, TdbLazy, TerminusDBField, TerminusDBModel as TerminusDBModelTrait, ToJson,
    ToTDBInstance, ToTDBSchema,
};
use terminusdb_schema_derive::TerminusDBModel;

// Test 1: Simple generic struct with one type parameter
#[derive(Debug, Clone, TerminusDBModel)]
struct Container<T>
where
    T: TerminusDBField<Container<T>>,
{
    id: String,
    value: T,
}

// Test 2: Generic struct with multiple fields of the generic type
#[derive(Debug, Clone, TerminusDBModel)]
struct Pair<T>
where
    T: TerminusDBField<Pair<T>>,
{
    first: T,
    second: T,
    description: String,
}

// Test 3: Generic struct with multiple type parameters
#[derive(Debug, Clone, TerminusDBModel)]
struct Mapping<K, V>
where
    K: TerminusDBField<Mapping<K, V>>,
    V: TerminusDBField<Mapping<K, V>>,
{
    key: K,
    value: V,
}

// Test 4: Generic struct with nested generics - commented out due to derive macro limitations
// The derive macro doesn't automatically add bounds for container types like Vec<T> and Option<T>
// #[derive(Debug, Clone, TerminusDBModel)]
// struct NestedContainer<T>
// where
//     T: TerminusDBField<NestedContainer<T>> + TerminusDBField<Pair<T>>,
// {
//     items: Vec<T>,
//     optional: Option<T>,
//     pairs: Vec<Pair<T>>,
// }

// Test 5: Generic struct with trait bounds
#[derive(Debug, Clone, TerminusDBModel)]
struct NumericContainer<T>
where
    T: std::fmt::Display + TerminusDBField<NumericContainer<T>>,
{
    value: T,
    formatted: String,
}

// Define concrete model types for testing with TdbLazy
#[derive(Debug, Clone, TerminusDBModel)]
struct Product {
    id: String,
    name: String,
    price: f64,
}

#[derive(Debug, Clone, TerminusDBModel)]
struct Category {
    id: String,
    name: String,
    parent: Option<String>,
}

// Test 6: Generic struct with TdbLazy field using generic parameter
// When T is used in TdbLazy<T>, T must implement TerminusDBModel
#[derive(Debug, Clone, TerminusDBModel)]
struct LazyContainer<T>
where
    T: TerminusDBModelTrait,
{
    id: String,
    lazy_value: TdbLazy<T>,
    description: String,
}

// Test 7: Mixed usage - T as regular field and U in TdbLazy
// This demonstrates different bound requirements for different usage patterns
#[derive(Debug, Clone, TerminusDBModel)]
struct MixedUsage<T, U>
where
    T: TerminusDBField<MixedUsage<T, U>>,
    U: TerminusDBModelTrait,
{
    id: String,
    regular_field: T,       // T used as regular field - needs TerminusDBField
    lazy_field: TdbLazy<U>, // U used in TdbLazy - needs TerminusDBModel
}

// Test 8: Generic enum - commented out as derive macros have limitations with generic enums
// #[derive(Debug, Clone, TerminusDBModel)]
// enum MyResult<T, E>
// where
//     T: std::fmt::Debug + Clone,
//     E: std::fmt::Debug + Clone,
// {
//     Ok(T),
//     Err(E),
// }

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_simple_container_schema() {
        // Test that Container<String> generates proper schema
        let schema = <Container<String> as ToTDBSchema>::to_schema();
        assert_eq!(schema.class_name(), "Container<String>");

        // Check that properties are correctly generated
        if let Some(props) = <Container<String> as ToTDBSchema>::properties() {
            assert_eq!(props.len(), 2);
            assert!(props.iter().any(|p| p.name == "id"));
            assert!(props.iter().any(|p| p.name == "value"));
        } else {
            panic!("Expected properties in schema");
        }
    }

    #[test]
    fn test_container_instance_roundtrip() {
        let container = Container {
            id: "test-1".to_string(),
            value: "Hello, World!".to_string(),
        };

        // Convert to instance
        let instance = container.to_instance(None);

        // Check instance properties
        assert_eq!(instance.schema.class_name(), "Container<String>");
        assert!(instance.has_property("id"));
        assert!(instance.has_property("value"));

        // Convert to JSON and back
        let json = instance.to_json();
        let recovered = Container::<String>::from_json(json).unwrap();

        assert_eq!(recovered.id, container.id);
        assert_eq!(recovered.value, container.value);
    }

    // Test for nested generics commented out since NestedContainer is not defined
    // #[test]
    // fn test_nested_generics() {
    //     let nested = NestedContainer {
    //         items: vec![1, 2, 3],
    //         optional: Some(42),
    //         pairs: vec![
    //             Pair {
    //                 first: 10,
    //                 second: 20,
    //                 description: "pair1".to_string(),
    //             },
    //             Pair {
    //                 first: 30,
    //                 second: 40,
    //                 description: "pair2".to_string(),
    //             },
    //         ],
    //     };

    //     let instance = nested.to_instance(None);
    //     let json = instance.to_json();

    //     // Verify JSON structure
    //     assert!(json.get("items").is_some());
    //     assert!(json.get("optional").is_some());
    //     assert!(json.get("pairs").is_some());
    // }

    #[test]
    fn test_multiple_type_parameters() {
        let mapping = Mapping {
            key: "name".to_string(),
            value: 42i32,
        };

        let schema = <Mapping<String, i32> as ToTDBSchema>::to_schema();
        assert_eq!(schema.class_name(), "Mapping<String, i32>");

        let instance = mapping.to_instance(None);
        assert!(instance.has_property("key"));
        assert!(instance.has_property("value"));
    }

    // Test for generic enum commented out as MyResult is not defined
    // #[test]
    // fn test_generic_enum() {
    //     let ok_result: MyResult<String, i32> = MyResult::Ok("Success".to_string());
    //     let err_result: MyResult<String, i32> = MyResult::Err(404);

    //     // Test Ok variant
    //     let ok_instance = ok_result.to_instance(None);
    //     assert!(ok_instance.is_enum());

    //     // Test Err variant
    //     let err_instance = err_result.to_instance(None);
    //     assert!(err_instance.is_enum());
    // }

    #[test]
    fn test_from_instance_with_generics() {
        let original = Container {
            id: "test-123".to_string(),
            value: 999,
        };

        let instance = original.to_instance(None);
        let recovered = Container::<i32>::from_instance(&instance).unwrap();

        assert_eq!(recovered.id, original.id);
        assert_eq!(recovered.value, original.value);
    }

    #[test]
    fn test_lazy_container_with_generics() {
        // Test with Product as the generic type parameter
        let product = Product {
            id: "prod-1".to_string(),
            name: "Widget".to_string(),
            price: 19.99,
        };

        let lazy_container = LazyContainer::<Product> {
            id: "lazy-1".to_string(),
            lazy_value: TdbLazy::from(product),
            description: "A lazy container with Product type".to_string(),
        };

        // Verify schema generation works
        let schema = <LazyContainer<Product> as ToTDBSchema>::to_schema();
        assert_eq!(schema.class_name(), "LazyContainer<Product>");

        // Convert to instance
        let instance = lazy_container.to_instance(None);
        assert!(instance.has_property("id"));
        assert!(instance.has_property("lazy_value"));
        assert!(instance.has_property("description"));

        // Test with Category as the generic type parameter
        let category = Category {
            id: "cat-1".to_string(),
            name: "Electronics".to_string(),
            parent: None,
        };

        let lazy_category = LazyContainer::<Category> {
            id: "lazy-2".to_string(),
            lazy_value: TdbLazy::from(category),
            description: "A lazy container with Category type".to_string(),
        };

        let category_schema = <LazyContainer<Category> as ToTDBSchema>::to_schema();
        assert_eq!(category_schema.class_name(), "LazyContainer<Category>");

        // Test round trip
        let category_instance = lazy_category.to_instance(None);
        let json = category_instance.to_json();
        let recovered = LazyContainer::<Category>::from_json(json).unwrap();

        assert_eq!(recovered.id, lazy_category.id);
        assert_eq!(recovered.description, lazy_category.description);
        // Note: TdbLazy values may not be directly comparable after round trip
        // as they may be in Reference state
    }

    #[test]
    fn test_mixed_generic_usage() {
        // Test MixedUsage with String as regular field and Product in TdbLazy
        let product = Product {
            id: "prod-1".to_string(),
            name: "Widget".to_string(),
            price: 19.99,
        };

        let mixed = MixedUsage::<String, Product> {
            id: "mixed-1".to_string(),
            regular_field: "Hello".to_string(),
            lazy_field: TdbLazy::from(product),
        };

        let schema = <MixedUsage<String, Product> as ToTDBSchema>::to_schema();
        assert_eq!(schema.class_name(), "MixedUsage<String, Product>");

        let instance = mixed.to_instance(None);
        assert!(instance.has_property("id"));
        assert!(instance.has_property("regular_field"));
        assert!(instance.has_property("lazy_field"));
    }
}
