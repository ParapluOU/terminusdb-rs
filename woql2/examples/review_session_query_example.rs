//! Example demonstrating type-checked query DSL for a complex review session query
//!
//! This example shows how to replicate a complex JSON-LD query using the type-checked
//! query DSL, with anonymized models and compile-time field verification.

use terminusdb_woql2::prelude::*;
use terminusdb_woql2::query;

// Simplified, anonymized models with only the fields used in the query
#[allow(dead_code)]
#[derive(Clone, Debug)]
struct ReviewSession {
    id: String,
    owner: String,  // Reference to User
    title: String,
    publication_id: String,  // Reference to Publication
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
struct Publication {
    id: String,
    title: String,
    committee: Option<String>,  // Optional reference to Committee
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
struct User {
    id: String,
    first_name: Option<String>,
    last_name: Option<String>,
    email: Option<String>,
    phone: Option<String>,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
struct Committee {
    id: String,
    name: String,
    description: String,
}

fn main() {
    println!("Review Session Query Example with Type-Checked DSL\n");
    
    // Construct the complex query using type-checked DSL with new optional syntax
    let review_query = limit!(10, query!{{
        select [SessionId, PublicationId, OwnerFirstName, OwnerLastName, 
                PublicationTitle, CommitteeId, CommitteeName, CommitteeDescription] {
            
            // ReviewSession type block with compile-time checked fields
            ReviewSession {
                id = v!(SessionId),
                owner = v!(Owner),
                title = v!(SessionTitle),
                publication_id = v!(Publication)
            }
            
            // Publication linked from ReviewSession
            Publication {
                id = v!(PublicationId),
                title = v!(PublicationTitle)
            }
            
            // User (owner) information
            User {
                id = v!(Owner),
                first_name = v!(OwnerFirstName),
                last_name = v!(OwnerLastName)
            }
            
            // Optional committee information using new syntax
            optional {
                // Link from Publication to Committee
                triple!(v!(Publication), field!(Publication:committee), v!(Committee)),
                
                Committee {
                    id = v!(CommitteeId),
                    name = v!(CommitteeName),
                    description = v!(CommitteeDescription)
                }
            }
        }
    }});
    
    println!("Generated Query DSL:");
    println!("{}\n", review_query.to_dsl());
    
    // Example showing query-level optional blocks
    println!("Example with query-level optional blocks:");
    
    let optional_block_query = query!{{
        User {
            id = v!(UserId),
            first_name = v!(FirstName),
            last_name = v!(LastName)
        }
        
        // Optional email and phone information
        optional {
            triple!(v!(User), field!(User:email), v!(Email)),
            triple!(v!(User), field!(User:phone), v!(Phone))
        }
    }};
    
    println!("{}\n", optional_block_query.to_dsl());
    
    // Complex example with nested optional patterns
    println!("Complex example with nested optional blocks:");
    
    let complex_optional_query = query!{{
        Person {
            id = v!(PersonId),
            name = v!(Name)
        }
        
        optional {
            // Entire address section is optional
            Address {
                id = v!(AddressId),
                person_id = v!(PersonId),
                street = v!(Street)
            }
            
            // Within the optional section, we can have more logic
            optional {
                triple!(v!(Address), field!(Address:apartment), v!(Apartment))
            }
        }
    }};
    
    println!("{}\n", complex_optional_query.to_dsl());
    
    // Demonstrate compile-time type checking
    println!("Key features of the new optional syntax:");
    println!("1. Optional blocks at query level: optional {{ ... }}");
    println!("2. Nested optional blocks are supported");
    println!("3. All property names are still verified at compile-time");
    println!("4. Clear visual distinction between required and optional data");
    
    // This would fail to compile if uncommented:
    // let bad_query = query!{{
    //     ReviewSession {
    //         titl = v!(Title)  // Error: no field `titl` on type `ReviewSession`
    //     }
    // }};
    
    println!("\nBenefits of the enhanced query DSL:");
    println!("- Consistent optional syntax throughout");
    println!("- No need to drop down to lower-level macros");
    println!("- Compile-time safety for all fields");
    println!("- Natural representation of data models with optional relationships");
    println!("- Supports complex nested optional patterns");
}

// Additional models for the examples
#[allow(dead_code)]
#[derive(Clone, Debug)]
struct Person {
    id: String,
    name: String,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
struct Address {
    id: String,
    person_id: String,
    street: String,
    apartment: Option<String>,
}