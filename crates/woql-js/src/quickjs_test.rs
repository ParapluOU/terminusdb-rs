//! QuickJS feasibility test module
//!
//! These tests validate that the QuickJS runtime can execute the WOQL parsing code.
//! They're separate from the main integration tests in lib.rs to allow for more
//! detailed testing of the QuickJS internals.

#[cfg(all(test, feature = "quickjs"))]
mod tests {
    use rquickjs::{Context, Runtime};

    // The existing Node.js bundle (for comparison/validation)
    const BUNDLE: &str = include_str!("../scripts/parse-woql.bundle.js");

    // The new QuickJS-compatible bundle
    const QUICKJS_BUNDLE: &str = include_str!("../scripts/parse-woql.quickjs.js");

    #[test]
    fn test_basic_quickjs() {
        let rt = Runtime::new().expect("Failed to create runtime");
        let ctx = Context::full(&rt).expect("Failed to create context");

        ctx.with(|ctx| {
            // Test basic JS execution
            let result: i32 = ctx.eval("1 + 2").expect("Failed to eval");
            assert_eq!(result, 3);

            // Test JSON handling
            let json_result: String = ctx
                .eval(r#"JSON.stringify({foo: "bar", num: 42})"#)
                .expect("Failed to eval JSON");
            assert_eq!(json_result, r#"{"foo":"bar","num":42}"#);
        });
    }

    #[test]
    fn test_function_definition() {
        let rt = Runtime::new().expect("Failed to create runtime");
        let ctx = Context::full(&rt).expect("Failed to create context");

        ctx.with(|ctx| {
            // Define a function and call it
            let _: () = ctx
                .eval(
                    r#"
                function parseQuery(query) {
                    return JSON.stringify({ type: "Query", content: query });
                }
            "#,
                )
                .expect("Failed to define function");

            let result: String = ctx
                .eval(r#"parseQuery("test")"#)
                .expect("Failed to call function");
            assert_eq!(result, r#"{"type":"Query","content":"test"}"#);
        });
    }

    #[test]
    fn test_commonjs_pattern() {
        let rt = Runtime::new().expect("Failed to create runtime");
        let ctx = Context::full(&rt).expect("Failed to create context");

        ctx.with(|ctx| {
            // Test if CommonJS-style module pattern works
            let result: String = ctx
                .eval(
                    r#"
                var exports = {};
                var module = { exports: exports };

                // Simulate a simple module
                (function(exports, module) {
                    exports.hello = function(name) {
                        return "Hello, " + name;
                    };
                })(exports, module);

                module.exports.hello("World")
            "#,
                )
                .expect("Failed to eval CommonJS pattern");
            assert_eq!(result, "Hello, World");
        });
    }

    #[test]
    fn test_eval_with_prelude() {
        let rt = Runtime::new().expect("Failed to create runtime");
        let ctx = Context::full(&rt).expect("Failed to create context");

        ctx.with(|ctx| {
            // This simulates how WOQL.emerge() would work - defining functions that
            // are then available when evaluating user code
            let prelude = r#"
                function triple(s, p, o) {
                    return {
                        type: "Triple",
                        subject: s,
                        predicate: p,
                        object: o,
                        json: function() {
                            return {
                                "@type": "Triple",
                                "subject": this.subject,
                                "predicate": this.predicate,
                                "object": this.object
                            };
                        }
                    };
                }
            "#;

            // First eval the prelude
            let _: () = ctx.eval(prelude).expect("Failed to eval prelude");

            // Then eval the user query and get JSON
            let result: String = ctx
                .eval(r#"JSON.stringify(triple("v:S", "v:P", "v:O").json())"#)
                .expect("Failed to eval query");

            let parsed: serde_json::Value =
                serde_json::from_str(&result).expect("Failed to parse result");
            assert_eq!(parsed["@type"], "Triple");
            assert_eq!(parsed["subject"], "v:S");
        });
    }

    #[test]
    fn test_bundle_structure_analysis() {
        // Analyze the bundle to understand what we need to modify
        // The bundle uses process.stdin which is Node.js specific

        // Check that the bundle contains the expected entry point
        assert!(BUNDLE.contains("process.stdin"));
        assert!(BUNDLE.contains("WOQL.emerge()") || BUNDLE.contains("WOQL.emerge"));

        // Check that it has the TerminusClient/WOQL exports
        assert!(BUNDLE.contains("require_terminusdb_client"));
    }

    #[test]
    fn test_modified_bundle_for_quickjs() {
        // Create a modified version of the bundle that works with QuickJS
        // We need to:
        // 1. Remove process.stdin handling
        // 2. Export a parseWoql function instead

        let rt = Runtime::new().expect("Failed to create runtime");
        let ctx = Context::full(&rt).expect("Failed to create context");

        ctx.with(|ctx| {
            // First, provide Node.js polyfills that QuickJS doesn't have
            let polyfills = r#"
                var process = { stdin: { on: function() {}, setEncoding: function() {} }, exit: function() {} };
                var console = {
                    log: function() {},
                    error: function() {}
                };
            "#;

            let _: () = ctx.eval(polyfills).expect("Failed to eval polyfills");

            // Now extract just the library part of the bundle (before the stdin handling)
            // The bundle structure is:
            // - CommonJS module definitions (__commonJS, __require, etc.)
            // - Library code
            // - parse-woql.js entry point that uses process.stdin

            // Find where the entry point starts
            let entry_marker = "// parse-woql.js";
            let entry_idx = BUNDLE.find(entry_marker);

            if let Some(idx) = entry_idx {
                // Get just the library part
                let library_part = &BUNDLE[..idx];

                // Eval the library part
                let eval_result: Result<(), _> = ctx.eval(library_part);
                if let Err(e) = eval_result {
                    // This might fail due to missing Node.js APIs, let's see
                    println!("Library eval error (expected): {:?}", e);
                }
            }
        });
    }

    #[test]
    fn test_quickjs_bundle_loads() {
        // Test that the QuickJS-specific bundle loads correctly
        // Note: No shims needed - the woql-only.js bundle avoids Node.js dependencies
        let rt = Runtime::new().expect("Failed to create runtime");
        let ctx = Context::full(&rt).expect("Failed to create context");

        ctx.with(|ctx| {
            // Eval the QuickJS bundle directly
            let result: Result<(), _> = ctx.eval(QUICKJS_BUNDLE);
            if let Err(e) = &result {
                // Try to get exception info
                if let Some(exc) = ctx.catch().as_exception() {
                    eprintln!("Exception message: {:?}", exc.message());
                    if let Some(stack) = exc.stack() {
                        eprintln!("Stack: {}", stack);
                    }
                }
                eprintln!("Error: {:?}", e);
            }
            assert!(
                result.is_ok(),
                "Failed to load QuickJS bundle: {:?}",
                result.err()
            );
        });
    }

    #[test]
    fn test_quickjs_bundle_parses_triple() {
        // Test parsing a simple triple query with the QuickJS bundle
        let rt = Runtime::new().expect("Failed to create runtime");
        let ctx = Context::full(&rt).expect("Failed to create context");

        ctx.with(|ctx| {
            // Load the bundle
            let _: () = ctx.eval(QUICKJS_BUNDLE).expect("Failed to load bundle");

            // Call parseWoql
            let result: String = ctx
                .eval(r#"parseWoql('triple("v:S", "v:P", "v:O")')"#)
                .expect("Failed to parse triple");

            // Verify the result
            let json: serde_json::Value =
                serde_json::from_str(&result).expect("Failed to parse JSON");
            assert_eq!(json["@type"], "Triple");
        });
    }

    #[test]
    fn test_quickjs_bundle_parses_select() {
        // Test parsing a select query with the QuickJS bundle
        let rt = Runtime::new().expect("Failed to create runtime");
        let ctx = Context::full(&rt).expect("Failed to create context");

        ctx.with(|ctx| {
            // Load the bundle
            let _: () = ctx.eval(QUICKJS_BUNDLE).expect("Failed to load bundle");

            // Call parseWoql with a more complex query
            let result: String = ctx
                .eval(
                    r#"parseWoql('select("Name", triple("v:Person", "@schema:name", "v:Name"))')"#,
                )
                .expect("Failed to parse select");

            // Verify the result
            let json: serde_json::Value =
                serde_json::from_str(&result).expect("Failed to parse JSON");
            assert_eq!(json["@type"], "Select");
        });
    }

    #[test]
    fn test_quickjs_bundle_parses_complex_query() {
        // Test parsing a complex query with the QuickJS bundle
        let rt = Runtime::new().expect("Failed to create runtime");
        let ctx = Context::full(&rt).expect("Failed to create context");

        ctx.with(|ctx| {
            // Load the bundle
            let _: () = ctx.eval(QUICKJS_BUNDLE).expect("Failed to load bundle");

            // Call parseWoql with a complex query
            let query = r#"
                select(
                    "Name", "Age",
                    and(
                        triple("v:Person", "rdf:type", "@schema:Person"),
                        triple("v:Person", "@schema:name", "v:Name"),
                        triple("v:Person", "@schema:age", "v:Age"),
                        greater("v:Age", 18)
                    )
                )
            "#;
            // Escape for JS string
            let js_code = format!("parseWoql({})", serde_json::to_string(query).unwrap());
            let result: String = ctx.eval(js_code).expect("Failed to parse complex query");

            // Verify the result
            let json: serde_json::Value =
                serde_json::from_str(&result).expect("Failed to parse JSON");
            assert_eq!(json["@type"], "Select");
        });
    }

    #[test]
    fn test_quickjs_bundle_output_matches_nodejs() {
        // Verify that the QuickJS bundle produces the same output as Node.js would
        let rt = Runtime::new().expect("Failed to create runtime");
        let ctx = Context::full(&rt).expect("Failed to create context");

        ctx.with(|ctx| {
            let _: () = ctx.eval(QUICKJS_BUNDLE).expect("Failed to load bundle");

            // Parse a simple triple
            let result: String = ctx
                .eval(r#"parseWoql('triple("v:Subject", "v:Predicate", "v:Object")')"#)
                .expect("Failed to parse triple");

            let json: serde_json::Value =
                serde_json::from_str(&result).expect("Failed to parse JSON");

            // Verify the expected structure
            assert_eq!(json["@type"], "Triple");
            assert!(json["subject"].is_object());
            assert!(json["predicate"].is_object());
            assert!(json["object"].is_object());

            // Check subject is a variable
            assert_eq!(json["subject"]["@type"], "NodeValue");
            assert_eq!(json["subject"]["variable"], "Subject");
        });
    }
}
