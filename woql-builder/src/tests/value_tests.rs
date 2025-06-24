// Import items from the crate root
use crate::value::Var; // Only Var needed for vars! test
use crate::vars; // Import the macro

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

// Add more tests for value/conversion logic if needed
// e.g., test node(), string_literal(), From impls, IntoWoql2 impls
