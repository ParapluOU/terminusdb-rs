// Import items from the crate root
use crate::value::{list, string_literal, Var, WoqlInput, IntoWoql2}; // Import list and other helpers
use crate::vars; // Import the macro
use terminusdb_woql2::prelude::{DataValue, Value as Woql2Value};
use terminusdb_schema::XSDAnySimpleType;

#[test]
fn test_vars_macro() {
    // Test single variable
    let var_a = vars!("A");
    assert_eq!(var_a, Var::new("A"));

    // Test multiple variables
    let (var_b, var_c, var_d) = vars!("B", "C", "D");
    assert_eq!(var_b, Var::new("B"));
    assert_eq!(var_c, Var::new("C"));
    assert_eq!(var_d, Var::new("D"));
}

// Test list literal creation
#[test]
fn test_list_helper_function() {
    // Test with string literals
    let list1 = list(vec!["Hello", "World"]);
    match list1 {
        WoqlInput::List(items) => {
            assert_eq!(items.len(), 2);
            match &items[0] {
                WoqlInput::Node(s) => assert_eq!(s, "Hello"),
                _ => panic!("Expected Node"),
            }
            match &items[1] {
                WoqlInput::Node(s) => assert_eq!(s, "World"),
                _ => panic!("Expected Node"),
            }
        }
        _ => panic!("Expected List"),
    }

    // Test with mixed types using string_literal for explicit strings
    let list2 = list(vec![string_literal("foo"), string_literal("bar")]);
    match list2 {
        WoqlInput::List(items) => {
            assert_eq!(items.len(), 2);
            match &items[0] {
                WoqlInput::String(s) => assert_eq!(s, "foo"),
                _ => panic!("Expected String"),
            }
        }
        _ => panic!("Expected List"),
    }

    // Test with variables
    let list3 = list(vec![Var::new("X"), Var::new("Y")]);
    match list3 {
        WoqlInput::List(items) => {
            assert_eq!(items.len(), 2);
            match &items[0] {
                WoqlInput::Variable(v) => assert_eq!(v.name(), "X"),
                _ => panic!("Expected Variable"),
            }
        }
        _ => panic!("Expected List"),
    }
}

#[test]
fn test_vec_to_list_conversion() {
    // Test From<Vec<T>> implementation
    let list1: WoqlInput = vec!["a", "b", "c"].into();
    match list1 {
        WoqlInput::List(items) => {
            assert_eq!(items.len(), 3);
            // These should be nodes by default from &str conversion
            match &items[0] {
                WoqlInput::Node(s) => assert_eq!(s, "a"),
                _ => panic!("Expected Node"),
            }
        }
        _ => panic!("Expected List"),
    }

    // Test with integers
    let list2: WoqlInput = vec![1, 2, 3].into();
    match list2 {
        WoqlInput::List(items) => {
            assert_eq!(items.len(), 3);
            match &items[0] {
                WoqlInput::Integer(i) => assert_eq!(*i, 1),
                _ => panic!("Expected Integer"),
            }
        }
        _ => panic!("Expected List"),
    }

    // Test with booleans
    let list3: WoqlInput = vec![true, false].into();
    match list3 {
        WoqlInput::List(items) => {
            assert_eq!(items.len(), 2);
            match &items[0] {
                WoqlInput::Boolean(b) => assert_eq!(*b, true),
                _ => panic!("Expected Boolean"),
            }
        }
        _ => panic!("Expected List"),
    }
}

#[test]
fn test_nested_lists() {
    // Test nested list creation
    let inner1 = list(vec!["a", "b"]);
    let inner2 = list(vec!["c", "d"]);
    let nested = list(vec![inner1, inner2]);
    
    match nested {
        WoqlInput::List(outer) => {
            assert_eq!(outer.len(), 2);
            match &outer[0] {
                WoqlInput::List(inner) => {
                    assert_eq!(inner.len(), 2);
                    match &inner[0] {
                        WoqlInput::Node(s) => assert_eq!(s, "a"),
                        _ => panic!("Expected Node"),
                    }
                }
                _ => panic!("Expected List"),
            }
        }
        _ => panic!("Expected List"),
    }
}

#[test]
fn test_list_into_woql2_value() {
    // Test conversion to Woql2Value
    let list = list(vec![string_literal("Hello"), string_literal("World")]);
    let woql2_value = list.into_woql2_value();
    
    match woql2_value {
        Woql2Value::List(items) => {
            assert_eq!(items.len(), 2);
            match &items[0] {
                Woql2Value::Data(XSDAnySimpleType::String(s)) => assert_eq!(s, "Hello"),
                _ => panic!("Expected Data String"),
            }
        }
        _ => panic!("Expected List"),
    }
}

#[test]
fn test_list_into_woql2_data_value() {
    // Test conversion to DataValue
    let list = list(vec![string_literal("foo"), string_literal("bar")]);
    let data_value = list.into_woql2_data_value();
    
    match data_value {
        DataValue::List(items) => {
            assert_eq!(items.len(), 2);
            match &items[0] {
                DataValue::Data(XSDAnySimpleType::String(s)) => assert_eq!(s, "foo"),
                _ => panic!("Expected Data String"),
            }
        }
        _ => panic!("Expected List"),
    }
}

#[test]
fn test_list_with_mixed_types() {
    // Test list with mixed data types
    let mixed = list(vec![
        WoqlInput::from("hello"),        // Node
        WoqlInput::from(42),             // Integer
        WoqlInput::from(true),           // Boolean
        WoqlInput::from(Var::new("X")), // Variable
    ]);
    
    match mixed {
        WoqlInput::List(items) => {
            assert_eq!(items.len(), 4);
            assert!(matches!(&items[0], WoqlInput::Node(_)));
            assert!(matches!(&items[1], WoqlInput::Integer(_)));
            assert!(matches!(&items[2], WoqlInput::Boolean(_)));
            assert!(matches!(&items[3], WoqlInput::Variable(_)));
        }
        _ => panic!("Expected List"),
    }
}
