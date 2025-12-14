//! Example demonstrating the flat syntax for query DSL with optional select/limit
//!
//! This example shows how to use the cleaner, more SQL-like syntax where
//! select and limit are statements within the query block.

use terminusdb_woql2::prelude::*;
use terminusdb_woql2::query;

// Example models
#[allow(dead_code)]
#[derive(Clone, Debug)]
struct Person {
    id: String,
    name: String,
    age: i32,
    email: Option<String>,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
struct Company {
    id: String,
    name: String,
    employees: Vec<String>,
}

fn main() {
    println!("Flat Syntax Query DSL Examples\n");
    
    // Example 1: Query with both select and limit
    println!("1. Query with select and limit:");
    let full_query = query!{{
        select [Name, Age];
        limit 10;
        
        Person {
            id = v!(PersonId),
            name = v!(Name),
            age = v!(Age)
        }
        greater!(v!(Age), data!(18))
    }};
    println!("{}\n", full_query.to_dsl());
    
    // Example 2: Query with only select (no limit)
    println!("2. Query with only select:");
    let select_only = query!{{
        select [Name];
        
        Person {
            id = v!(PersonId),
            name = v!(Name),
            age = v!(Age)
        }
    }};
    println!("{}\n", select_only.to_dsl());
    
    // Example 3: Query with only limit (no select)
    println!("3. Query with only limit:");
    let limit_only = query!{{
        limit 5;
        
        Person {
            name = v!(Name),
            email = v!(Email)
        }
    }};
    println!("{}\n", limit_only.to_dsl());
    
    // Example 4: Plain query (no select, no limit)
    println!("4. Plain query without modifiers:");
    let plain_query = query!{{
        Person {
            name = v!(Name),
            age = v!(Age)
        }
        Company {
            name = v!(CompanyName),
            employees = v!(Employees)
        }
    }};
    println!("{}\n", plain_query.to_dsl());
    
    // Example 5: Complex query with flat syntax
    println!("5. Complex query with optional blocks:");
    let complex_query = query!{{
        select [PersonName, CompanyName, Email];
        limit 20;
        
        Person {
            id = v!(PersonId),
            name = v!(PersonName),
            age = v!(Age)
        }
        
        optional {
            triple!(v!(Person), field!(Person:email), v!(Email)),
            
            Company {
                id = v!(CompanyId),
                name = v!(CompanyName)
            }
            
            triple!(v!(Company), field!(Company:employees), v!(PersonId))
        }
    }};
    println!("{}\n", complex_query.to_dsl());
    
    // Example 6: Modifiers can be in any order
    println!("6. Modifiers in reverse order:");
    let reverse_order = query!{{
        limit 15;
        select [Name, Age];
        
        Person {
            name = v!(Name),
            age = v!(Age)
        }
    }};
    println!("{}\n", reverse_order.to_dsl());
    
    println!("Key benefits of flat syntax:");
    println!("- Select and limit are optional and independent");
    println!("- Cleaner, more SQL-like appearance");
    println!("- No deep nesting required");
    println!("- Modifiers visually separated from query logic");
    println!("- Can be specified in any order");
    
    // Backward compatibility - nested syntax still works
    println!("\nBackward compatible nested syntax still works:");
    let nested = query!{{
        select [Name] {
            Person { name = v!(Name) }
        }
    }};
    println!("{}", nested.to_dsl());
}