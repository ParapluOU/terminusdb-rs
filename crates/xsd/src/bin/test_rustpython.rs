//! Standalone test of RustPython functionality
//!
//! This is a minimal binary to test if RustPython works in our setup
//! without requiring the full workspace to build.

use rustpython_vm::Interpreter;

fn main() {
    println!("=== RustPython Test ===\n");

    println!("Creating RustPython interpreter...");
    let interpreter = Interpreter::with_init(Default::default(), |vm| {
        vm.add_native_modules(rustpython_vm::stdlib::get_module_inits());
    });
    println!("✓ Interpreter created\n");

    println!("Testing basic Python execution...");
    interpreter.enter(|vm| {
        let code = r#"
print("Hello from RustPython!")
result = 2 + 2
print(f"2 + 2 = {result}")
"#;
        let scope = vm.new_scope_with_builtins();
        match vm.run_code_string(scope.clone(), code, "<test>".to_owned()) {
            Ok(_) => {
                println!("✓ Python code executed successfully\n");

                let result = scope.globals.get_item("result", vm).expect("Should have result");
                let value: i32 = result.try_into_value(vm).expect("Should convert to i32");
                println!("Result from Python: {}\n", value);
            }
            Err(e) => {
                eprintln!("✗ Error: {}", e);
            }
        }
    });

    println!("Testing JSON module...");
    interpreter.enter(|vm| {
        let code = r#"
import json

data = {
    "test": "RustPython",
    "working": True,
    "version": "0.4"
}

result = json.dumps(data, indent=2)
print("JSON output:")
print(result)
"#;
        let scope = vm.new_scope_with_builtins();
        match vm.run_code_string(scope, code, "<json_test>".to_owned()) {
            Ok(_) => println!("✓ JSON module works\n"),
            Err(e) => eprintln!("✗ JSON error: {}", e),
        }
    });

    println!("Testing xmlschema import (this will likely fail)...");
    interpreter.enter(|vm| {
        let code = "import xmlschema";
        let scope = vm.new_scope_with_builtins();
        match vm.run_code_string(scope, code, "<xmlschema_test>".to_owned()) {
            Ok(_) => {
                println!("✓ xmlschema imported successfully!");
                println!("  This means RustPython CAN run xmlschema!");
                println!("  The experiment is successful!\n");
            }
            Err(e) => {
                println!("✗ xmlschema import failed:");
                println!("  {}\n", e);
                println!("Expected failure reasons:");
                println!("  - RustPython doesn't support all Python features xmlschema needs");
                println!("  - xmlschema or its dependencies aren't installed");
                println!("  - xmlschema uses C extensions (lxml, etc.) that RustPython can't load\n");
                println!("This suggests we may need to pursue alternative approaches.");
            }
        }
    });

    println!("=== Test Complete ===");
}
