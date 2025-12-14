use crate::*;
use std::collections::BTreeMap;
use crate::instance::build_instance_tree;

#[test]
fn test_subdocument_tagged_union_flatten() {
    // Test that tagged union subdocuments are handled correctly during flattening
    // The key insight is that the variant classes (e.g., AnnotationComment) should be marked as subdocuments
    
    // Test 1: A subdocument instance should not be flattened (count should be 0)
    let mut comment_variant = Instance {
        schema: Schema::Class {
            id: "AnnotationComment".to_string(),
            base: None,
            properties: vec![
                Property {
                    name: "text".to_string(),
                    r#type: None,
                    class: "xsd:string".to_string(),
                },
                Property {
                    name: "author".to_string(),
                    r#type: None,
                    class: "xsd:string".to_string(),
                },
            ],
            subdocument: true, // This variant is a subdocument
            key: Key::ValueHash,
            unfoldable: false,
            documentation: None,
            r#abstract: false,
            inherits: vec![],
        },
        id: None, // Subdocuments should not have IDs
        capture: false,
        ref_props: false,
        properties: {
            let mut props = BTreeMap::new();
            props.insert("text".to_string(), InstanceProperty::Primitive(PrimitiveValue::String("This is a comment".to_string())));
            props.insert("author".to_string(), InstanceProperty::Primitive(PrimitiveValue::String("Alice".to_string())));
            props
        },
    };

    // Test flattening the subdocument directly
    let removed = comment_variant.flatten(false);
    assert_eq!(removed.len(), 0, "Subdocument instance should not have any nested instances to flatten");

    // Test 2: A tagged union containing subdocument variants
    let annotation_instance = Instance {
        schema: Schema::TaggedUnion {
            id: "Annotation".to_string(),
            base: None,
            key: crate::schema::Key::Random,
            r#abstract: false,
            documentation: None,
            subdocument: false,
            unfoldable: false,
            properties: vec![
                Property {
                    name: "comment".to_string(),
                    r#type: None,
                    class: "AnnotationComment".to_string(),
                },
                Property {
                    name: "highlight".to_string(),
                    r#type: None,
                    class: "AnnotationHighlight".to_string(),
                },
            ],
        },
        id: Some("Annotation/123".to_string()),
        capture: false,
        ref_props: false,
        properties: {
            let mut props = BTreeMap::new();
            // The tagged union contains the variant as a relation
            props.insert("comment".to_string(), InstanceProperty::Relation(RelationValue::One(comment_variant.clone())));
            props
        },
    };
    
    // Clone for testing
    let mut test_instance = annotation_instance.clone();
    
    // Flatten should not remove the subdocument variant - it should stay embedded
    let removed = test_instance.flatten(false);
    println!("Removed instances count: {}", removed.len());
    
    // The subdocument variant should remain embedded, so nothing should be removed
    assert_eq!(removed.len(), 0, "Subdocument variant should not be flattened out");

    // Test 3: Test to_instance_tree_flatten behavior
    // Create a parent document containing the tagged union
    let document_instance = Instance {
        schema: Schema::Class {
            id: "Document".to_string(),
            base: None,
            properties: vec![
                Property {
                    name: "title".to_string(),
                    r#type: None,
                    class: "xsd:string".to_string(),
                },
                Property {
                    name: "annotations".to_string(),
                    r#type: Some(TypeFamily::List),
                    class: "Annotation".to_string(),
                },
            ],
            subdocument: false, // Document is NOT a subdocument
            key: Key::Random,
            unfoldable: false,
            documentation: None,
            r#abstract: false,
            inherits: vec![],
        },
        id: Some("Document/456".to_string()),
        capture: false,
        ref_props: false,
        properties: {
            let mut props = BTreeMap::new();
            props.insert("title".to_string(), InstanceProperty::Primitive(PrimitiveValue::String("My Document".to_string())));
            
            // Add the annotation as a relation
            props.insert("annotations".to_string(), InstanceProperty::Relations(vec![
                RelationValue::One(annotation_instance.clone()),
            ]));
            props
        },
    };

    // Build instance tree
    let instance_tree = build_instance_tree(&document_instance);
    println!("Instance tree count: {}", instance_tree.len());
    
    // The tree should contain:
    // 1. The document instance
    // 2. The annotation instance (tagged union)
    // 3. The comment variant instance (subdocument)
    assert!(instance_tree.len() >= 3, "Instance tree should contain at least 3 instances");
    
    // Test filtering in to_instance_tree_flatten
    // This simulates what to_instance_tree_flatten does
    let mut instances = instance_tree.clone();
    for instance in &mut instances {
        instance.flatten(true);
    }
    
    // Filter out subdocuments
    let filtered = instances
        .into_iter()
        .filter(|inst| !inst.schema.is_subdocument())
        .collect::<Vec<_>>();
    
    println!("Filtered instance count: {}", filtered.len());
    
    // After filtering, we should have:
    // 1. The document instance
    // 2. The annotation instance (tagged union - not a subdocument)
    // But NOT the comment variant (it's a subdocument)
    assert_eq!(filtered.len(), 2, "Filtered instances should not include subdocuments");
    
    // Verify the remaining instances
    assert!(filtered.iter().any(|i| i.schema.class_name() == "Document"), "Document should remain");
    assert!(filtered.iter().any(|i| i.schema.class_name() == "Annotation"), "Annotation (tagged union) should remain");
    assert!(!filtered.iter().any(|i| i.schema.class_name() == "AnnotationComment"), "AnnotationComment (subdocument) should be filtered out");
    
    // Test 4: Verify subdocument remains embedded in parent after flattening
    let mut doc_for_flatten = document_instance.clone();
    let removed_from_doc = doc_for_flatten.flatten(true);
    
    println!("Instances removed from document during flatten: {}", removed_from_doc.len());
    
    // Print details about what was removed
    for (i, removed) in removed_from_doc.iter().enumerate() {
        println!("Removed instance {}: {} (subdocument: {})", 
                 i + 1, 
                 removed.schema.class_name(), 
                 removed.schema.is_subdocument());
    }
    
    // Check the document's annotations after flatten
    if let Some(InstanceProperty::Relations(annotations)) = doc_for_flatten.properties.get("annotations") {
        if let Some(rel) = annotations.first() {
            match rel {
                RelationValue::ExternalReference(id) | RelationValue::TransactionRef(id) => {
                    println!("Annotation was flattened to reference: {}", id);
                    // This is expected - the TaggedUnion itself is not a subdocument
                },
                RelationValue::One(ann_instance) => {
                    println!("Annotation instance remains embedded (unexpected!)");
                    if let Some(InstanceProperty::Relation(RelationValue::One(variant))) = ann_instance.properties.get("comment") {
                        assert!(variant.schema.is_subdocument(), "Variant should still be marked as subdocument");
                        assert!(variant.properties.contains_key("text"), "Subdocument content should be preserved");
                        println!("✓ Subdocument variant remains embedded with its data");
                    }
                },
                _ => println!("Unexpected relation type"),
            }
        }
    }
    
    // Test 5: Verify the fix - TaggedUnion with subdocument variant should remain embedded
    println!("\n=== Test 5: Verifying fix - TaggedUnion with subdocument variant ===");
    
    // Create a fresh test instance
    let mut fixed_doc = document_instance.clone();
    
    // Check that should_remain_embedded works correctly
    if let Some(InstanceProperty::Relations(annotations)) = fixed_doc.properties.get("annotations") {
        if let Some(RelationValue::One(ann_instance)) = annotations.first() {
            println!("Annotation should_remain_embedded: {}", ann_instance.should_remain_embedded());
            assert!(ann_instance.should_remain_embedded(), 
                    "TaggedUnion with subdocument variant should report that it should remain embedded");
        }
    }
    
    // Now flatten and verify the TaggedUnion stays embedded
    let removed_fixed = fixed_doc.flatten(true);
    println!("Instances removed with fix: {}", removed_fixed.len());
    
    // The TaggedUnion should NOT be flattened anymore
    assert_eq!(removed_fixed.len(), 0, "With the fix, no instances should be removed");
    
    // Verify the structure is preserved
    if let Some(InstanceProperty::Relations(annotations)) = fixed_doc.properties.get("annotations") {
        if let Some(rel) = annotations.first() {
            match rel {
                RelationValue::One(ann_instance) => {
                    println!("✓ SUCCESS: TaggedUnion remains embedded!");
                    
                    // Verify the subdocument variant is still there
                    if let Some(InstanceProperty::Relation(RelationValue::One(variant))) = ann_instance.properties.get("comment") {
                        assert!(variant.schema.is_subdocument(), "Variant is still marked as subdocument");
                        assert!(variant.properties.contains_key("text"), "Subdocument data is preserved");
                        println!("✓ Subdocument variant data is intact");
                    }
                },
                RelationValue::ExternalReference(_) | RelationValue::TransactionRef(_) => {
                    panic!("TaggedUnion should not be flattened with the fix!");
                },
                _ => panic!("Unexpected relation type"),
            }
        }
    }
    
    println!("\n✓ Fix verified: TaggedUnions with subdocument variants now remain embedded correctly!");

    // Test 6: Verify to_instance_tree_flatten filters out TaggedUnions with subdocument variants
    println!("\n=== Test 6: Verifying to_instance_tree_flatten filtering ===");
    
    // Create a simple ToTDBInstances implementation for our document
    struct DocumentWrapper(Instance);
    
    impl ToTDBInstances for DocumentWrapper {
        fn to_instance_tree(&self) -> Vec<Instance> {
            build_instance_tree(&self.0)
        }
    }
    
    let wrapper = DocumentWrapper(document_instance.clone());
    
    // Get the flattened instance tree
    let flattened_tree = wrapper.to_instance_tree_flatten(true);
    
    println!("Flattened instance tree count: {}", flattened_tree.len());
    for (i, inst) in flattened_tree.iter().enumerate() {
        println!("  Instance {}: {} (should_remain_embedded: {})", 
                 i + 1, 
                 inst.schema.class_name(),
                 inst.should_remain_embedded());
    }
    
    // The flattened tree should only contain the Document instance
    // TaggedUnions with subdocument variants should be filtered out
    assert_eq!(flattened_tree.len(), 1, "Only the Document instance should remain");
    assert_eq!(flattened_tree[0].schema.class_name(), "Document", "The single instance should be the Document");
    
    // Verify that the Annotation (TaggedUnion with subdocument variant) was filtered out
    assert!(!flattened_tree.iter().any(|i| i.schema.class_name() == "Annotation"), 
            "TaggedUnion with subdocument variant should be filtered out");
    
    println!("✓ SUCCESS: to_instance_tree_flatten correctly filters out TaggedUnions with subdocument variants!");
    
    // This is the key fix that ensures insert_instances won't try to insert
    // TaggedUnions with subdocument variants as separate documents
}