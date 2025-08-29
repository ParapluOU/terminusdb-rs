use terminusdb_schema::*;
use terminusdb_schema_derive::TerminusDBModel;

// Model with ServerIDFor using random key
#[derive(Clone, Debug, Default, TerminusDBModel, serde::Serialize, serde::Deserialize)]
#[tdb(key = "random", id_field = "id")]
pub struct TestModel {
    pub id: ServerIDFor<Self>,
    pub name: String,
    pub value: i32,
}

fn main() {
    // Create a new model without an ID
    let mut model = TestModel {
        id: ServerIDFor::new(),
        name: "Test Model".to_string(),
        value: 42,
    };
    
    println!("Model before server ID: {:?}", model);
    println!("ID is none: {}", model.id.is_none());
    
    // Simulate server setting the ID (normally done during deserialization)
    model.id.__set_from_server(EntityIDFor::new("test-123").unwrap());
    
    println!("Model after server ID: {:?}", model);
    println!("ID is some: {}", model.id.is_some());
    println!("ID value: {}", model.id.as_ref().unwrap().id());
    
    // Serialize to JSON
    let json = serde_json::to_string_pretty(&model).unwrap();
    println!("Serialized JSON:\n{}", json);
    
    // Deserialize from JSON
    let json_str = r#"{
        "id": "TestModel/from-server",
        "name": "From Server",
        "value": 100
    }"#;
    
    let from_server: TestModel = serde_json::from_str(json_str).unwrap();
    println!("Deserialized model: {:?}", from_server);
    println!("Server ID: {}", from_server.id.as_ref().unwrap().id());
}