use anyhow::*;
use parture_terminusdb_client::deserialize::*;
use parture_terminusdb_client::DocumentInsertArgs;
use terminusdb_schema::*;
use terminusdb_schema_derive::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TerminusDBModel, FromTDBInstance)]
struct TestStruct {
    name: String,
    count: i32,
    active: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip() -> anyhow::Result<()> {
        // Create a test instance
        let original = TestStruct {
            name: "Test Item".to_string(),
            count: 42,
            active: true,
        };

        let instance = original.to_instance(None);

        let json = instance.to_json();

        println!("{:#?}", &json);

        let retrieved: TestStruct = (DefaultTDBDeserializer {}).from_instance(json).unwrap();

        // Verify the roundtrip
        assert_eq!(retrieved, original);

        Ok(())
    }
}
