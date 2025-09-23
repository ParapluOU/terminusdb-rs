use terminusdb_woql2::prelude::*;
use terminusdb_schema::XSDAnySimpleType;

#[test]
fn test_member_with_vec_string() {
    // Test with Vec<String>
    let fruits = vec!["apple".to_string(), "banana".to_string(), "cherry".to_string()];
    let query = member!(var!(fruit), fruits);
    
    match query {
        Query::Member(member) => {
            // Check that list was properly converted
            match &member.list {
                DataValue::List(items) => {
                    assert_eq!(items.len(), 3);
                    match &items[0] {
                        DataValue::Data(XSDAnySimpleType::String(s)) => assert_eq!(s, "apple"),
                        _ => panic!("Expected string data value"),
                    }
                    match &items[1] {
                        DataValue::Data(XSDAnySimpleType::String(s)) => assert_eq!(s, "banana"),
                        _ => panic!("Expected string data value"),
                    }
                    match &items[2] {
                        DataValue::Data(XSDAnySimpleType::String(s)) => assert_eq!(s, "cherry"),
                        _ => panic!("Expected string data value"),
                    }
                },
                _ => panic!("Expected list variant"),
            }
        },
        _ => panic!("Expected Member query"),
    }
}

#[test]
fn test_member_with_vec_value() {
    // Test with Vec<Value>
    let values = vec![
        data!("hello"),
        data!(42),
        data!(true),
    ];
    let query = member!(var!(x), values);
    
    match query {
        Query::Member(member) => {
            match &member.list {
                DataValue::List(items) => {
                    assert_eq!(items.len(), 3);
                    match &items[0] {
                        DataValue::Data(XSDAnySimpleType::String(s)) => assert_eq!(s, "hello"),
                        _ => panic!("Expected string data value"),
                    }
                    match &items[1] {
                        DataValue::Data(XSDAnySimpleType::Integer(i)) => assert_eq!(*i, 42),
                        _ => panic!("Expected integer data value"),
                    }
                    match &items[2] {
                        DataValue::Data(XSDAnySimpleType::Boolean(b)) => assert_eq!(*b, true),
                        _ => panic!("Expected boolean data value"),
                    }
                },
                _ => panic!("Expected list variant"),
            }
        },
        _ => panic!("Expected Member query"),
    }
}

#[test]
fn test_member_with_vec_datavalue() {
    // Test with Vec<DataValue>
    let data_values = vec![
        DataValue::Data(XSDAnySimpleType::String("test".to_string())),
        DataValue::Variable("X".to_string()),
        DataValue::Data(XSDAnySimpleType::Integer(100)),
    ];
    let query = member!(var!(elem), data_values);
    
    match query {
        Query::Member(member) => {
            match &member.list {
                DataValue::List(items) => {
                    assert_eq!(items.len(), 3);
                    match &items[0] {
                        DataValue::Data(XSDAnySimpleType::String(s)) => assert_eq!(s, "test"),
                        _ => panic!("Expected string data value"),
                    }
                    match &items[1] {
                        DataValue::Variable(v) => assert_eq!(v, "X"),
                        _ => panic!("Expected variable"),
                    }
                    match &items[2] {
                        DataValue::Data(XSDAnySimpleType::Integer(i)) => assert_eq!(*i, 100),
                        _ => panic!("Expected integer data value"),
                    }
                },
                _ => panic!("Expected list variant"),
            }
        },
        _ => panic!("Expected Member query"),
    }
}

#[test]
fn test_member_with_empty_vec() {
    // Test with empty Vec
    let empty: Vec<String> = vec![];
    let query = member!(var!(x), empty);
    
    match query {
        Query::Member(member) => {
            match &member.list {
                DataValue::List(items) => {
                    assert_eq!(items.len(), 0);
                },
                _ => panic!("Expected list variant"),
            }
        },
        _ => panic!("Expected Member query"),
    }
}

#[test]
fn test_member_with_vec_str() {
    // Test with Vec<&str>
    let items = vec!["one", "two", "three"];
    let query = member!(var!(item), items);
    
    match query {
        Query::Member(member) => {
            match &member.list {
                DataValue::List(items) => {
                    assert_eq!(items.len(), 3);
                    match &items[0] {
                        DataValue::Data(XSDAnySimpleType::String(s)) => assert_eq!(s, "one"),
                        _ => panic!("Expected string data value"),
                    }
                },
                _ => panic!("Expected list variant"),
            }
        },
        _ => panic!("Expected Member query"),
    }
}