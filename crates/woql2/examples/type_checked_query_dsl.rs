//! Example demonstrating type-checked query DSL with field! macro integration
//!
//! This example shows how the query DSL automatically verifies that property names
//! match the actual fields in your model structs, preventing runtime errors from
//! typos or schema changes.

use terminusdb_woql2::prelude::*;
use terminusdb_woql2::query;

// Example models
#[allow(dead_code)]
#[derive(Clone, Debug)]
struct Person {
    name: String,
    age: i32,
    email: Option<String>,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
struct Company {
    name: String,
    founded_year: i32,
    ceo: String,
}

fn main() {
    println!("Type-checked Query DSL Examples\n");

    // Example 1: Simple type-checked query
    let person_query = query! {{
        Person {
            id = data!("person123"),
            name = v!(name),
            age = v!(age)
        }
        greater!(v!(age), data!(18))
    }};

    println!("Person query (type-checked):");
    println!("{}\n", person_query.to_dsl());

    // Example 2: Multiple types with relationships
    let company_query = query! {{
        Company {
            id = v!(CompanyId),
            name = v!(CompanyName),
            founded_year = v!(Year),
            ceo = v!(CeoId)
        }
        Person {
            id = v!(CeoId),
            name = v!(CeoName),
            age = v!(CeoAge)
        }
        greater!(v!(Year), data!(2000))
    }};

    println!("Company-Person relationship query:");
    println!("{}\n", company_query.to_dsl());

    // Example 3: Select query with type checking
    let select_query = query! {{
        select [CompanyName, CeoName] {
            Company {
                id = v!(CompanyId),
                name = v!(CompanyName),
                ceo = v!(CeoId)
            }
            Person {
                id = v!(CeoId),
                name = v!(CeoName)
            }
        }
    }};

    println!("Select query (returns company and CEO names):");
    println!("{}\n", select_query.to_dsl());

    // The following would fail to compile if uncommented:
    // let bad_query = query!{{
    //     Person {
    //         nam = v!(name)  // Error: no field `nam` on type `Person`
    //     }
    // }};

    println!("Key benefits of type-checked queries:");
    println!("1. Compile-time verification of property names");
    println!("2. IDE autocomplete for model fields");
    println!("3. Automatic updates when model schemas change");
    println!("4. No runtime errors from typos in property names");
}
