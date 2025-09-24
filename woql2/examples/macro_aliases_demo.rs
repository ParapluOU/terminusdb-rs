use terminusdb_woql2::*;

fn main() {
    println!("Demonstrating WOQL macro aliases\n");
    
    // Using original macros
    let query1 = and!(
        triple!(var!(person), "rdf:type", "Person"),
        triple!(var!(person), "name", var!(name)),
        optional!(triple!(var!(person), "email", var!(email)))
    );
    
    println!("Original macros query:");
    println!("{:?}\n", query1);
    
    // Using aliases - more concise syntax
    let query2 = and!(
        t!(var!(person), "rdf:type", "Person"),
        t!(var!(person), "name", var!(name)),
        opt!(t!(var!(person), "email", var!(email)))
    );
    
    println!("Using aliases (t! for triple!, opt! for optional!):");
    println!("{:?}\n", query2);
    
    // Mix and match - all aliases are interchangeable
    let query3 = and!(
        t!(var!(person), "rdf:type", "Person"),
        triple!(var!(person), "name", var!(name)),
        option!(t!(var!(person), "phone", var!(phone))),  // option! is another alias for optional!
        opt!(triple!(var!(person), "address", var!(address)))
    );
    
    println!("Mix and match - using different aliases:");
    println!("{:?}\n", query3);
    
    // Complex query with aliases
    let complex_query = select!([person, name, email], and!(
        t!(var!(person), "rdf:type", "Person"),
        t!(var!(person), "age", var!(age)),
        greater!(var!(age), data!(21)),
        t!(var!(person), "name", var!(name)),
        opt!(t!(var!(person), "email", var!(email)))
    ));
    
    println!("Complex query using aliases:");
    println!("{:?}", complex_query);
}