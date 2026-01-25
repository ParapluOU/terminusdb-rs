use terminusdb_woql2::order::{Order, OrderTemplate};
use terminusdb_woql2::prelude::*;
use terminusdb_woql2::{order_by, triple, var};

#[test]
fn test_order_by_original_syntax() {
    // Test the original OrderTemplate syntax still works
    let query = order_by!(
        [
            OrderTemplate {
                variable: "name".to_string(),
                order: Order::Asc
            },
            OrderTemplate {
                variable: "age".to_string(),
                order: Order::Desc
            },
        ],
        triple!(var!(person), "name", var!(name))
    );

    match query {
        Query::OrderBy(order_by) => {
            assert_eq!(order_by.ordering.len(), 2);
            assert_eq!(order_by.ordering[0].variable, "name");
            assert_eq!(order_by.ordering[0].order, Order::Asc);
            assert_eq!(order_by.ordering[1].variable, "age");
            assert_eq!(order_by.ordering[1].order, Order::Desc);
        }
        _ => panic!("Expected OrderBy query"),
    }
}

#[test]
fn test_order_by_tuple_syntax_with_variables() {
    // Test tuple syntax with var!() macro
    let query = order_by!(
        [(var!(name), Order::Asc), (var!(age), Order::Desc)],
        triple!(var!(person), "name", var!(name))
    );

    match query {
        Query::OrderBy(order_by) => {
            assert_eq!(order_by.ordering.len(), 2);
            assert_eq!(order_by.ordering[0].variable, "name");
            assert_eq!(order_by.ordering[0].order, Order::Asc);
            assert_eq!(order_by.ordering[1].variable, "age");
            assert_eq!(order_by.ordering[1].order, Order::Desc);
        }
        _ => panic!("Expected OrderBy query"),
    }
}

#[test]
fn test_order_by_tuple_syntax_with_strings() {
    // Test tuple syntax with string literals
    let query = order_by!(
        [("name", Order::Asc), ("age", Order::Desc)],
        triple!(var!(person), "name", var!(name))
    );

    match query {
        Query::OrderBy(order_by) => {
            assert_eq!(order_by.ordering.len(), 2);
            assert_eq!(order_by.ordering[0].variable, "name");
            assert_eq!(order_by.ordering[0].order, Order::Asc);
            assert_eq!(order_by.ordering[1].variable, "age");
            assert_eq!(order_by.ordering[1].order, Order::Desc);
        }
        _ => panic!("Expected OrderBy query"),
    }
}

#[test]
fn test_order_by_arrow_syntax() {
    // Test arrow syntax
    let query = order_by!(
        [name => Order::Asc, age => Order::Desc],
        triple!(var!(person), "name", var!(name))
    );

    match query {
        Query::OrderBy(order_by) => {
            assert_eq!(order_by.ordering.len(), 2);
            assert_eq!(order_by.ordering[0].variable, "name");
            assert_eq!(order_by.ordering[0].order, Order::Asc);
            assert_eq!(order_by.ordering[1].variable, "age");
            assert_eq!(order_by.ordering[1].order, Order::Desc);
        }
        _ => panic!("Expected OrderBy query"),
    }
}

#[test]
fn test_order_by_single_ordering() {
    // Test with a single ordering
    let query1 = order_by!(
        [(var!(name), Order::Desc)],
        triple!(var!(person), "name", var!(name))
    );

    let query2 = order_by!(
        [name => Order::Desc],
        triple!(var!(person), "name", var!(name))
    );

    match (query1, query2) {
        (Query::OrderBy(order_by1), Query::OrderBy(order_by2)) => {
            assert_eq!(order_by1.ordering.len(), 1);
            assert_eq!(order_by2.ordering.len(), 1);
            assert_eq!(order_by1.ordering[0].variable, "name");
            assert_eq!(order_by2.ordering[0].variable, "name");
            assert_eq!(order_by1.ordering[0].order, Order::Desc);
            assert_eq!(order_by2.ordering[0].order, Order::Desc);
        }
        _ => panic!("Expected OrderBy queries"),
    }
}

#[test]
fn test_order_by_mixed_types() {
    // Test mixing String and &str
    let name_var = "name".to_string();
    let query = order_by!(
        [(name_var, Order::Asc), ("age", Order::Desc)],
        triple!(var!(person), "name", var!(name))
    );

    match query {
        Query::OrderBy(order_by) => {
            assert_eq!(order_by.ordering.len(), 2);
            assert_eq!(order_by.ordering[0].variable, "name");
            assert_eq!(order_by.ordering[0].order, Order::Asc);
            assert_eq!(order_by.ordering[1].variable, "age");
            assert_eq!(order_by.ordering[1].order, Order::Desc);
        }
        _ => panic!("Expected OrderBy query"),
    }
}
