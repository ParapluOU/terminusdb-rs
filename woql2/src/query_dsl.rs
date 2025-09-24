//! Higher-level query DSL macros for more intuitive WOQL query writing
//!
//! This module provides a set of macros that enable writing WOQL queries using a more
//! natural, declarative syntax. Instead of manually constructing triples and type 
//! declarations, you can use a syntax that resembles object notation.
//!
//! # Overview
//!
//! The query DSL provides several key improvements over the standard WOQL macro syntax:
//!
//! 1. **Type blocks** - Group all properties of a type together
//! 2. **Variable shorthand** - Use `v!(name)` instead of `var!("name")`
//! 3. **Natural comparisons** - Write comparisons inline with query logic
//! 4. **Select syntax** - Cleaner syntax for select queries
//! 5. **Type-checked properties** - Property names are verified at compile-time against model structs
//! 6. **Optional blocks** - Express optional sections and fields naturally
//!
//! # Basic Example
//!
//! ```ignore
//! use terminusdb_woql2::prelude::*;
//! 
//! // Traditional WOQL syntax
//! let traditional = and!(
//!     type_!(var!("Person"), "Person"),
//!     id!(var!("Person"), data!("person123")),
//!     triple!(var!("Person"), "name", var!("name")),
//!     triple!(var!("Person"), "age", var!("age")),
//!     greater!(var!("age"), data!(18))
//! );
//!
//! // Query DSL syntax (with type-checked properties)
//! let dsl = query!{{
//!     Person {
//!         id = data!("person123"),
//!         name = v!(name),
//!         age = v!(age)
//!     }
//!     greater!(v!(age), data!(18))
//! }};
//! ```
//! 
//! Note: The model struct `Person` must be in scope for property name verification.
//!
//! # Syntax Reference
//!
//! ## Type Blocks
//!
//! Type blocks automatically generate type declarations and property triples:
//!
//! ```ignore
//! query!{{
//!     TypeName {
//!         property1 = value1,
//!         property2 = value2,
//!         // Special handling for 'id' property
//!         id = data!("some-id")  // Uses id! macro instead of triple!
//!     }
//! }}
//! ```
//!
//! This expands to:
//! ```ignore
//! and!(
//!     type_!(var!("TypeName"), "TypeName"),
//!     id!(var!("TypeName"), data!("some-id")),
//!     triple!(var!("TypeName"), "property1", value1),
//!     triple!(var!("TypeName"), "property2", value2)
//! )
//! ```
//!
//! ## Variable References
//!
//! The `v!` macro provides a shorthand for variable references:
//!
//! ```ignore
//! v!(PersonId)  // Equivalent to var!("PersonId")
//! ```
//!
//! ## Select Queries
//!
//! Select queries have a special syntax:
//!
//! ```ignore
//! query!{{
//!     select [var1, var2, var3] {
//!         // Query body here
//!     }
//! }}
//! ```
//!
//! ## Optional Blocks
//!
//! Optional sections can be expressed at the query level:
//!
//! ```ignore
//! query!{{
//!     Person { name = v!(name) }
//!     
//!     optional {
//!         // This entire section is optional
//!         Address { street = v!(street) }
//!     }
//! }}
//! ```
//!
//! ## Combining with Standard Macros
//!
//! The query DSL is designed to work seamlessly with standard WOQL macros:
//!
//! ```ignore
//! query!{{
//!     Person {
//!         id = v!(PersonId),
//!         age = v!(Age)
//!     }
//!     // Standard WOQL macros work here
//!     greater!(v!(Age), data!(21)),
//!     less!(v!(Age), data!(65)),
//!     optional!(triple!(v!(Person), "email", v!(Email))),
//!     read_doc!(v!(Person), v!(PersonDoc))
//! }}
//! ```
//!
//! # Complex Example
//!
//! Here's a more complex example showing multiple types and relationships:
//!
//! ```ignore
//! let query = query!{{
//!     select [AnnotationDoc] {
//!         // Define ReviewSession and its properties
//!         ReviewSession {
//!             id = data!(session_id),
//!             publication_id = v!(PublicationId),
//!             date_range = v!(DateRange)
//!         }
//!         
//!         // Define DateRange properties
//!         DateRange {
//!             start = v!(StartDate),
//!             end = v!(EndDate)
//!         }
//!         
//!         // Define Publication
//!         AwsDBPublication {
//!             id = v!(PublicationId),
//!             document_map = v!(DocumentMap)
//!         }
//!         
//!         // Define relationships and constraints
//!         AwsDBPublicationMap {
//!             chunks = v!(Chunk)
//!         }
//!         
//!         // Annotations linked to chunks
//!         Annotation {
//!             document_id = v!(ChunkId),
//!             timestamp = v!(Timestamp)
//!         }
//!         
//!         // Time constraints
//!         greater!(v!(Timestamp), v!(StartDate)),
//!         less!(v!(Timestamp), v!(EndDate)),
//!         
//!         // Read the full annotation document
//!         read_doc!(v!(Annotation), v!(AnnotationDoc))
//!     }
//! }};
//! ```
//!
//! # How It Works
//!
//! The `query!` macro uses Rust's macro pattern matching to transform the high-level
//! syntax into standard WOQL queries. The transformation process:
//!
//! 1. Type blocks are converted to `type_!` declarations plus property triples
//! 2. The special `id` property uses the `id!` macro instead of `triple!`
//! 3. All expressions are collected and wrapped in an `and!` query
//! 4. Select queries are handled specially to preserve the variable list
//!
//! # Type-Checked Properties
//!
//! When a model struct is in scope, the query DSL automatically verifies property names
//! at compile-time using the `field!` macro internally. This prevents runtime errors from:
//! - Typos in property names
//! - Properties that don't exist on the model
//! - Schema changes that break queries
//!
//! ```ignore
//! struct Person { name: String, age: i32 }
//! 
//! // This compiles successfully
//! query!{{ Person { name = v!(n) } }}
//! 
//! // This would fail at compile-time:
//! // query!{{ Person { nam = v!(n) } }}
//! // Error: no field `nam` on type `Person`
//! ```
//!
//! # Limitations
//!
//! - Type names must be valid Rust identifiers and correspond to structs in scope
//! - Property names must be valid Rust identifiers and exist on the model struct
//! - Values must be valid Rust expressions
//! - The macro requires double braces `{{}}` around the query body
//!
//! # Performance
//!
//! The query DSL macros are compile-time transformations with zero runtime overhead.
//! The generated code is identical to manually written WOQL queries.

/// Main query DSL macro that transforms high-level expressions into WOQL queries
///
/// This macro provides a more natural syntax for writing WOQL queries by allowing
/// type blocks, cleaner variable references, and integrated query expressions.
///
/// # Syntax Elements
///
/// ## Type Blocks
/// ```ignore
/// TypeName {
///     property = value,
///     another_property = v!(variable),
///     id = data!("unique-id")  // Special handling
/// }
/// ```
///
/// ## Select Queries
/// ```ignore
/// select [var1, var2] {
///     // query body
/// }
/// ```
///
/// ## Variable References
/// Use `v!(name)` as shorthand for `var!("name")`
///
/// ## Standard WOQL Macros
/// All standard WOQL macros can be used within the query body:
/// - `greater!`, `less!`, `eq!`, `compare!`
/// - `optional!`, `not!`, `or!`
/// - `read_doc!`, `insert_doc!`, `update_doc!`, `delete_doc!`
/// - `triple!`, `link!`, `data_triple!`
/// - etc.
///
/// # Examples
///
/// ## Simple Query
/// ```ignore
/// query!{{
///     Person {
///         id = data!("person123"),
///         name = v!(name),
///         age = v!(age)
///     }
///     greater!(v!(age), data!(18))
/// }}
/// ```
///
/// ## Select Query
/// ```ignore
/// query!{{
///     select [name, age] {
///         Person {
///             id = v!(PersonId),
///             name = v!(name),
///             age = v!(age)
///         }
///         greater!(v!(age), data!(21))
///     }
/// }}
/// ```
///
/// ## Multiple Types
/// ```ignore
/// query!{{
///     Author {
///         id = v!(AuthorId),
///         name = v!(AuthorName)
///     }
///     Book {
///         id = v!(BookId),
///         title = v!(BookTitle),
///         author = v!(AuthorId)  // Reference to Author
///     }
/// }}
/// ```
#[macro_export]
macro_rules! query {
    // Select query with variables and body
    ({ select [$($var:ident),* $(,)?] { $($body:tt)* } }) => {
        select!([$($var),*], query!{{ $($body)* }})
    };
    
    // Main query body processing with braces
    ({ $($body:tt)* }) => {
        query!(@parse_body [] $($body)*)
    };
    
    // Parse body - accumulate expressions
    (@parse_body [$($acc:expr),*] ) => {
        and!($($acc),*)
    };
    
    // Parse optional block at query level
    (@parse_body [$($acc:expr),*] optional { $($opt_body:tt)* } $($rest:tt)*) => {
        query!(@parse_body [
            $($acc,)*
            optional!(query!{{ $($opt_body)* }})
        ] $($rest)*)
    };
    
    // Parse type block - simplified approach
    (@parse_body [$($acc:expr),*] $type:ident { $($field:ident = $value:expr),* $(,)? } $($rest:tt)*) => {
        query!(@parse_body [
            $($acc,)*
            type_!(var!($type), $type),
            $(query!(@parse_field $type, $field, $value)),*
        ] $($rest)*)
    };
    
    // Parse standalone expressions (comparisons, function calls)
    (@parse_body [$($acc:expr),*] $expr:expr, $($rest:tt)*) => {
        query!(@parse_body [
            $($acc,)*
            $expr
        ] $($rest)*)
    };
    
    // Parse standalone expression at end
    (@parse_body [$($acc:expr),*] $expr:expr) => {
        query!(@parse_body [
            $($acc,)*
            $expr
        ])
    };
    
    // Parse field assignment for type blocks
    (@parse_field $type:ident, id, $value:expr) => {
        id!(var!($type), $value)
    };
    
    (@parse_field $type:ident, $field:ident, $value:expr) => {
        triple!(var!($type), field!($type:$field), $value)
    };
}

/// Variable reference macro for use in query DSL
///
/// Provides a concise way to create variable references without quoting.
///
/// # Examples
/// ```ignore
/// v!(name)       // Equivalent to var!(name)
/// v!(PersonId)   // Equivalent to var!(PersonId)
/// v!(StartDate)  // Equivalent to var!(StartDate)
/// ```
///
/// # Usage in Queries
/// ```ignore
/// query!{{
///     Person {
///         name = v!(PersonName),
///         age = v!(PersonAge)
///     }
///     greater!(v!(PersonAge), data!(18))
/// }}
/// ```
#[macro_export]
macro_rules! v {
    ($var:ident) => {
        var!($var)
    };
}

/// Helper macro for creating type-safe property references
///
/// This macro helps ensure property names are consistent when used
/// multiple times in a query.
///
/// # Examples
/// ```ignore
/// let name_prop = prop!(Person::name);
/// let age_prop = prop!(Person::age);
/// 
/// // Use in queries
/// triple!(v!(person), prop!(Person::name), v!(name))
/// ```
#[macro_export]
macro_rules! prop {
    ($type:ident::$field:ident) => {
        stringify!($field)
    };
}

/// Helper macro for creating schema type references
///
/// Generates the full schema URI for a type name.
///
/// # Examples
/// ```ignore
/// schema_type!(Person)        // Returns "@schema:Person"
/// schema_type!(ReviewSession) // Returns "@schema:ReviewSession"
/// 
/// // Use in type declarations
/// type_!(v!(obj), schema_type!(Person))
/// ```
#[macro_export]
macro_rules! schema_type {
    ($type:ident) => {
        concat!("@schema:", stringify!($type))
    };
}

#[cfg(test)]
mod tests {
    use crate::prelude::*;
    
    // Test model for type-checked queries
    #[allow(dead_code)]
    struct Person {
        id: String,
        name: String,
        age: i32,
    }
    
    #[test]
    fn test_simple_query_dsl() {
        let result = query!{{
            Person {
                id = data!("person123"),
                name = v!(name),
                age = v!(age)
            }
            greater!(v!(age), data!(18)),
            less!(v!(age), data!(65))
        }};
        
        // Verify it's an And query
        assert!(matches!(result, Query::And(_)));
    }
    
    #[test]
    fn test_v_macro() {
        let var = v!(PersonId);
        match var {
            Value::Variable(s) => assert_eq!(s, "PersonId"),
            _ => panic!("Expected Variable"),
        }
    }
    
    #[test]
    fn test_optional_block_query() {
        // Test model for optional fields
        #[allow(dead_code)]
        struct Document {
            id: String,
            title: String,
            author: Option<String>,
            published_date: Option<String>,
        }
        
        let result = query!{{
            Document {
                id = v!(DocId),
                title = v!(Title)
            }
            optional {
                triple!(v!(Document), field!(Document:author), v!(Author)),
                triple!(v!(Document), field!(Document:published_date), v!(PublishedDate))
            }
        }};
        
        // Should be And(type, id, title, Optional(And(author, published_date)))
        match result {
            Query::And(ref and) => {
                assert_eq!(and.and.len(), 4); // type, id, title, optional
                
                // Check that the last element is an optional
                assert!(matches!(&and.and[3], Query::WoqlOptional(_)));
                
                // Check the contents of the optional
                if let Query::WoqlOptional(opt) = &and.and[3] {
                    assert!(matches!(&*opt.query, Query::And(_)));
                }
            }
            _ => panic!("Expected And query"),
        }
    }
    
    #[test]
    fn test_nested_optional_blocks() {
        // Test model
        #[allow(dead_code)]
        struct Company {
            id: String,
            name: String,
            address: Option<String>,
        }
        
        #[allow(dead_code)]
        struct Address {
            id: String,
            street: String,
            city: String,
            postal_code: Option<String>,
        }
        
        let result = query!{{
            Company {
                id = v!(CompanyId),
                name = v!(CompanyName)
            }
            
            optional {
                triple!(v!(Company), field!(Company:address), v!(Address)),
                Address {
                    id = v!(Address),
                    street = v!(Street),
                    city = v!(City)
                }
                optional {
                    triple!(v!(Address), field!(Address:postal_code), v!(PostalCode))
                }
            }
        }};
        
        // Verify nested optional structure
        match result {
            Query::And(ref and) => {
                // Should have type, id, name, and optional block
                assert_eq!(and.and.len(), 4);
                
                // Last should be optional
                if let Query::WoqlOptional(outer_opt) = &and.and[3] {
                    // The outer optional should contain an And
                    if let Query::And(inner_and) = &*outer_opt.query {
                        // Should have link triple, Address type block fields, and inner optional
                        let last_idx = inner_and.and.len() - 1;
                        assert!(matches!(&inner_and.and[last_idx], Query::WoqlOptional(_)));
                    } else {
                        panic!("Expected And inside outer optional");
                    }
                } else {
                    panic!("Expected WoqlOptional as last element");
                }
            }
            _ => panic!("Expected And query"),
        }
    }
}