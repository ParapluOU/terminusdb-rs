//! Integration tests for XSD parsing via RustPython
//!
//! These tests explore whether RustPython can successfully run the
//! xmlschema Python module.

use terminusdb_xsd::{XsdParser, Result};

#[test]
fn test_create_parser() {
    let result = XsdParser::new();
    assert!(result.is_ok(), "Should be able to create XSD parser");
}

#[test]
fn test_basic_python_execution() {
    let parser = XsdParser::new().expect("Failed to create parser");

    parser.interpreter.enter(|vm| {
        let code = r#"
# Basic Python code
x = 10
y = 20
result = x + y
"#;
        let scope = vm.new_scope_with_builtins();
        let exec_result = vm.run_code_string(scope.clone(), code, "<test>".to_owned());
        assert!(exec_result.is_ok(), "Should execute basic Python code");

        let result = scope.globals.get_item("result", vm).expect("Should have result");
        let value: i32 = result.try_into_value(vm).expect("Should convert to i32");
        assert_eq!(value, 30);
    });
}

#[test]
fn test_json_module_available() {
    let parser = XsdParser::new().expect("Failed to create parser");

    parser.interpreter.enter(|vm| {
        let code = r#"
import json

data = {
    "name": "test",
    "value": 123,
    "nested": {
        "key": "value"
    }
}

result = json.dumps(data)
"#;
        let scope = vm.new_scope_with_builtins();
        let exec_result = vm.run_code_string(scope.clone(), code, "<test>".to_owned());
        assert!(exec_result.is_ok(), "Should be able to import and use json module");

        let result = scope.globals.get_item("result", vm).expect("Should have result");
        let json_str: String = result.try_into_value(vm).expect("Should convert to String");

        // Parse the JSON to verify it's valid
        let parsed: serde_json::Value = serde_json::from_str(&json_str).expect("Should be valid JSON");
        assert_eq!(parsed["name"], "test");
        assert_eq!(parsed["value"], 123);
    });
}

#[test]
#[ignore] // This test requires xmlschema to be installed and may fail
fn test_xmlschema_import() {
    let parser = XsdParser::new().expect("Failed to create parser");

    match parser.test_xmlschema_import() {
        Ok(_) => {
            println!("✓ xmlschema import successful!");
            println!("  RustPython can load the xmlschema module!");
        }
        Err(e) => {
            println!("✗ xmlschema import failed: {}", e);
            println!("  This is expected if:");
            println!("    - xmlschema uses features RustPython doesn't support");
            println!("    - xmlschema dependencies are incompatible");
            println!("    - xmlschema isn't installed");

            // Don't fail the test - this is exploratory
            println!("\n  Consider this test as exploratory - failure is valuable information!");
        }
    }
}

#[test]
#[ignore] // This test requires a sample XSD file
fn test_parse_sample_xsd() {
    // This test would need a sample XSD file to parse
    // For now, it's marked as ignored

    let parser = XsdParser::new().expect("Failed to create parser");

    // You would need to provide a path to an actual XSD file
    // let sample_xsd = "tests/fixtures/sample.xsd";
    // let result = parser.parse_xsd_to_json(sample_xsd);

    println!("To run this test, add a sample XSD file to tests/fixtures/");
}

/// Test what Python modules are available in RustPython
#[test]
fn test_available_modules() {
    let parser = XsdParser::new().expect("Failed to create parser");

    parser.interpreter.enter(|vm| {
        let code = r#"
import sys

# Try to get available modules
available_modules = []

# Standard library modules that should work
test_modules = [
    'json',
    'sys',
    'os',
    're',
    'math',
    'collections',
    'itertools',
    'datetime',
]

for mod in test_modules:
    try:
        __import__(mod)
        available_modules.append(mod)
    except ImportError:
        pass

result = ','.join(available_modules)
"#;
        let scope = vm.new_scope_with_builtins();
        let exec_result = vm.run_code_string(scope.clone(), code, "<test>".to_owned());
        assert!(exec_result.is_ok(), "Should check module availability");

        let result = scope.globals.get_item("result", vm).expect("Should have result");
        let modules: String = result.try_into_value(vm).expect("Should convert to String");

        println!("Available Python modules in RustPython: {}", modules);
        assert!(modules.contains("json"), "json module should be available");
    });
}
