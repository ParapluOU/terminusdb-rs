//! Example demonstrating how to use a custom document binding variable
//! in InstanceQueryable implementations.

use terminusdb_client::query::InstanceQueryable;
use terminusdb_woql_builder::builder::WoqlBuilder;
use terminusdb_woql_builder::prelude::Var;
use terminusdb_schema::{InstanceFromJson, TerminusDBModel};
use serde::{Deserialize, Serialize};
use terminusdb_schema_derive::TerminusDBModel;

#[derive(Clone, Debug, TerminusDBModel)]
struct ExampleModel {
    name: String,
}

// Custom query implementation with a different document binding
struct CustomDocumentQuery;

impl InstanceQueryable for CustomDocumentQuery {
    type Model = ExampleModel;
    
    // Override the default "Doc" binding to use a custom name
    const READ_DOCUMENT_BINDING: &'static str = "MyCustomDocVar";
    
    fn build(&self, subject: Var, builder: WoqlBuilder) -> WoqlBuilder {
        // Add custom filtering logic here if needed
        builder
    }
}

fn main() {
    // Demonstrate accessing the custom binding
    println!("Default binding: {}", 
        <terminusdb_client::query::FilteredListModels<ExampleModel> as InstanceQueryable>::READ_DOCUMENT_BINDING
    );
    
    println!("Custom binding: {}", 
        <CustomDocumentQuery as InstanceQueryable>::READ_DOCUMENT_BINDING
    );
    
    // The doc_var() method automatically uses the correct binding
    let default_var = <terminusdb_client::query::FilteredListModels<ExampleModel> as InstanceQueryable>::doc_var();
    let custom_var = <CustomDocumentQuery as InstanceQueryable>::doc_var();
    
    println!("Default var name: {}", default_var.name());
    println!("Custom var name: {}", custom_var.name());
}