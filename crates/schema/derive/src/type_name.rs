/// Helper functions for prettifying type names from std::any::type_name
///
/// This module provides utilities to convert fully qualified type names
/// (like "alloc::string::String") into cleaner, more readable forms (like "String").

/// Prettifies a type name from std::any::type_name by removing module paths
///
/// # Examples
/// ```ignore
/// assert_eq!(prettify_type_name("alloc::string::String"), "String");
/// assert_eq!(prettify_type_name("core::option::Option<alloc::string::String>"), "Option<String>");
/// assert_eq!(prettify_type_name("std::collections::HashMap<alloc::string::String, i32>"), "HashMap<String, i32>");
/// assert_eq!(prettify_type_name("Vec<Option<String>>"), "Vec<Option<String>>");
/// ```
pub fn prettify_type_name(full_name: &str) -> String {
    let mut result = String::new();
    let mut chars = full_name.chars().peekable();
    let mut current_segment = String::new();

    while let Some(ch) = chars.next() {
        match ch {
            '<' => {
                // We've hit generic parameters - prettify the base type first
                let pretty_base = prettify_simple_type(&current_segment);
                result.push_str(&pretty_base);
                result.push('<');
                current_segment.clear();

                // Process generic parameters
                let generic_params = collect_generic_params(&mut chars);
                let pretty_params = prettify_generic_params(&generic_params);
                result.push_str(&pretty_params);
                result.push('>');
            }
            '>' => {
                // This shouldn't happen in well-formed input at this level
                // but handle it gracefully
                if !current_segment.is_empty() {
                    result.push_str(&prettify_simple_type(&current_segment));
                    current_segment.clear();
                }
                result.push('>');
            }
            _ => {
                current_segment.push(ch);
            }
        }
    }

    // Handle any remaining segment
    if !current_segment.is_empty() {
        result.push_str(&prettify_simple_type(&current_segment));
    }

    result
}

/// Prettifies a simple type name (no generics) by taking only the last component
fn prettify_simple_type(type_name: &str) -> &str {
    type_name.rsplit("::").next().unwrap_or(type_name)
}

/// Collects all characters that form generic parameters, handling nested generics
fn collect_generic_params(chars: &mut std::iter::Peekable<std::str::Chars>) -> String {
    let mut params = String::new();
    let mut depth = 1; // We've already seen the opening '<'

    while let Some(ch) = chars.next() {
        match ch {
            '<' => {
                depth += 1;
                params.push(ch);
            }
            '>' => {
                depth -= 1;
                if depth == 0 {
                    // We've closed the generic parameters
                    return params;
                }
                params.push(ch);
            }
            _ => {
                params.push(ch);
            }
        }
    }

    params
}

/// Prettifies generic parameters, handling multiple parameters and nested generics
fn prettify_generic_params(params: &str) -> String {
    let mut result = String::new();
    let mut current_param = String::new();
    let mut depth = 0;

    for ch in params.chars() {
        match ch {
            ',' if depth == 0 => {
                // End of a parameter at the top level
                if !current_param.is_empty() {
                    if !result.is_empty() {
                        result.push_str(", ");
                    }
                    // Recursively prettify this parameter
                    result.push_str(&prettify_type_name(current_param.trim()));
                    current_param.clear();
                }
            }
            '<' => {
                depth += 1;
                current_param.push(ch);
            }
            '>' => {
                depth -= 1;
                current_param.push(ch);
            }
            _ => {
                current_param.push(ch);
            }
        }
    }

    // Don't forget the last parameter
    if !current_param.is_empty() {
        if !result.is_empty() {
            result.push_str(", ");
        }
        result.push_str(&prettify_type_name(current_param.trim()));
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_types() {
        assert_eq!(prettify_type_name("String"), "String");
        assert_eq!(prettify_type_name("i32"), "i32");
        assert_eq!(prettify_type_name("alloc::string::String"), "String");
        assert_eq!(prettify_type_name("std::vec::Vec"), "Vec");
    }

    #[test]
    fn test_generic_types() {
        assert_eq!(
            prettify_type_name("core::option::Option<alloc::string::String>"),
            "Option<String>"
        );
        assert_eq!(prettify_type_name("alloc::vec::Vec<i32>"), "Vec<i32>");
        assert_eq!(
            prettify_type_name("std::collections::HashMap<alloc::string::String, i32>"),
            "HashMap<String, i32>"
        );
    }

    #[test]
    fn test_nested_generics() {
        assert_eq!(
            prettify_type_name("Vec<Option<String>>"),
            "Vec<Option<String>>"
        );
        assert_eq!(
            prettify_type_name(
                "std::collections::HashMap<alloc::string::String, alloc::vec::Vec<i32>>"
            ),
            "HashMap<String, Vec<i32>>"
        );
        assert_eq!(
            prettify_type_name("core::option::Option<std::collections::HashMap<alloc::string::String, alloc::vec::Vec<i32>>>"),
            "Option<HashMap<String, Vec<i32>>>"
        );
    }
}
