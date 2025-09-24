use terminusdb_woql2::string::Concatenate;
use terminusdb_woql2::value::{DataValue, ListOrVariable};
use terminusdb_schema::XSDAnySimpleType;
use serde_json;

fn main() {
    println!("Demonstrating the DataValue::List serialization fix\n");
    
    // Create a Concatenate query with a list
    let concat = Concatenate {
        list: ListOrVariable::List(vec![
            DataValue::Data(XSDAnySimpleType::String("AwsDBPublication/".to_string())),
            DataValue::Variable("PubId".to_string()),
        ]),
        result_string: DataValue::Variable("PubIRIStr".to_string()),
    };
    
    // Serialize to JSON
    let json = serde_json::to_value(&concat).unwrap();
    let pretty_json = serde_json::to_string_pretty(&json).unwrap();
    
    println!("JSON-LD output for Concatenate:");
    println!("{}", pretty_json);
    
    println!("\nKey points:");
    println!("- The 'list' field is now a JSON array, not an object");
    println!("- Each element in the array has the proper DataValue structure");
    println!("- This matches what TerminusDB expects for the Concatenate operation");
    
    // Show what it would have looked like before the fix
    println!("\nBefore the fix, it would have been:");
    println!(r#"{{
  "list": {{
    "@type": "DataValue",
    "list": [
      {{
        "@type": "DataValue",
        "data": {{
          "String": "AwsDBPublication/"
        }}
      }},
      {{
        "@type": "DataValue",
        "variable": "PubId"
      }}
    ]
  }},
  "result_string": {{
    "@type": "DataValue",
    "variable": "PubIRIStr"
  }}
}}"#);
}