use schemas::{Dita12, Dita13, SchemaBundle};
use std::path::PathBuf;
use tempfile::TempDir;
use terminusdb_xsd::XsdModel;

fn main() {
    // Test DITA 1.3
    println!("=== Testing DITA 1.3 ===");
    let dir13 = TempDir::new().unwrap();
    Dita13::write_to_directory(dir13.path()).unwrap();

    let ditabase_13 = dir13.path().join("technicalContent/xsd/ditabase.xsd");
    let map_13 = dir13.path().join("technicalContent/xsd/map.xsd");

    println!("ditabase.xsd exists: {}", ditabase_13.exists());
    println!("map.xsd exists: {}", map_13.exists());

    // Load DITA 1.3 ditabase
    match XsdModel::from_file(&ditabase_13, None::<&str>) {
        Ok(model) => {
            println!("DITA 1.3 ditabase.xsd: {} schemas", model.schemas().len());

            let topic_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<topic id="test" xmlns:ditaarch="http://dita.oasis-open.org/architecture/2005/" ditaarch:DITAArchVersion="1.3">
  <title>Test</title><body><p>Test.</p></body>
</topic>"#;

            match model.parse_xml_to_instances(topic_xml) {
                Ok(i) => println!("  Topic parsed: {} instances ✓", i.len()),
                Err(e) => println!("  Topic parse ERROR: {:?}", e),
            }

            let concept_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<concept id="test" xmlns:ditaarch="http://dita.oasis-open.org/architecture/2005/" ditaarch:DITAArchVersion="1.3">
  <title>Test Concept</title><conbody><p>Test.</p></conbody>
</concept>"#;

            match model.parse_xml_to_instances(concept_xml) {
                Ok(i) => println!("  Concept parsed: {} instances ✓", i.len()),
                Err(e) => println!("  Concept parse ERROR: {:?}", e),
            }
        }
        Err(e) => println!("Failed to load DITA 1.3 ditabase: {:?}", e),
    }

    // Load DITA 1.3 map
    if map_13.exists() {
        match XsdModel::from_file(&map_13, None::<&str>) {
            Ok(model) => {
                println!("DITA 1.3 map.xsd: {} schemas", model.schemas().len());

                let map_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<map id="test" xmlns:ditaarch="http://dita.oasis-open.org/architecture/2005/" ditaarch:DITAArchVersion="1.3">
  <title>Test</title>
</map>"#;

                match model.parse_xml_to_instances(map_xml) {
                    Ok(i) => println!("  Map parsed: {} instances ✓", i.len()),
                    Err(e) => println!("  Map parse ERROR: {:?}", e),
                }
            }
            Err(e) => println!("Failed to load DITA 1.3 map.xsd: {:?}", e),
        }
    }

    // Test DITA 1.3 combined
    println!("\n=== DITA 1.3 Combined Entry Points ===");
    match XsdModel::from_entry_points(&[ditabase_13.clone(), map_13.clone()], None::<PathBuf>) {
        Ok(model) => {
            println!("Combined DITA 1.3: {} schemas", model.schemas().len());

            let topic_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<topic id="test" xmlns:ditaarch="http://dita.oasis-open.org/architecture/2005/" ditaarch:DITAArchVersion="1.3">
  <title>Test</title><body><p>Test.</p></body>
</topic>"#;
            let map_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<map id="test" xmlns:ditaarch="http://dita.oasis-open.org/architecture/2005/" ditaarch:DITAArchVersion="1.3">
  <title>Test</title>
</map>"#;

            match model.parse_xml_to_instances(topic_xml) {
                Ok(i) => println!("  Topic: {} instances ✓", i.len()),
                Err(e) => println!("  Topic ERROR: {:?}", e),
            }
            match model.parse_xml_to_instances(map_xml) {
                Ok(i) => println!("  Map: {} instances ✓", i.len()),
                Err(e) => println!("  Map ERROR: {:?}", e),
            }
        }
        Err(e) => println!("Combined load failed: {:?}", e),
    }

    // Compare with DITA 1.2
    println!("\n=== Comparison ===");
    let dir12 = TempDir::new().unwrap();
    Dita12::write_to_directory(dir12.path()).unwrap();
    let ditabase_12 = dir12
        .path()
        .join("xsd1.2-url/technicalContent/xsd/ditabase.xsd");
    let map_12 = dir12.path().join("xsd1.2-url/technicalContent/xsd/map.xsd");

    let model_12 = XsdModel::from_entry_points(&[ditabase_12, map_12], None::<PathBuf>).unwrap();
    let model_13 = XsdModel::from_entry_points(&[ditabase_13, map_13], None::<PathBuf>).unwrap();

    println!("DITA 1.2 combined schemas: {}", model_12.schemas().len());
    println!("DITA 1.3 combined schemas: {}", model_13.schemas().len());
}
