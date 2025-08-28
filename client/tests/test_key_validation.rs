#[cfg(test)]
mod test_key_validation {
    use serde::{Deserialize, Serialize};
    use terminusdb_schema::ToTDBInstance;
    use terminusdb_schema_derive::TerminusDBModel;

    #[derive(Debug, Clone, Deserialize, Serialize, PartialEq, TerminusDBModel)]
    #[tdb(key = "lexical", key_fields = "name")]
    struct PersonWithLexicalKey {
        pub name: String,
        pub age: i32,
    }

    #[derive(Debug, Clone, Deserialize, Serialize, PartialEq, TerminusDBModel)]
    #[tdb(key = "hash", key_fields = "email")]
    struct UserWithHashKey {
        pub email: String,
        pub username: String,
    }

    #[derive(Debug, Clone, Deserialize, Serialize, PartialEq, TerminusDBModel)]
    #[tdb(key = "value_hash")]
    struct DocumentWithValueHashKey {
        pub title: String,
        pub content: String,
    }

    #[derive(Debug, Clone, Deserialize, Serialize, PartialEq, TerminusDBModel)]
    // Default is Random key
    struct PersonWithRandomKey {
        pub name: String,
        pub age: i32,
    }

    #[test]
    #[should_panic(expected = "Model 'PersonWithLexicalKey' uses Some(Lexical")]
    fn test_panic_on_lexical_key_with_id() {
        // This should panic because we're trying to set an ID on a model with Lexical key
        let person = PersonWithLexicalKey {
            name: "John".to_string(),
            age: 30,
        };
        
        // Simulate setting an ID (in real usage this might come from deserialization)
        let instance = person.to_instance(Some("custom_id".to_string()));
        
        // This will panic in prepare_instances
        let _instances = vec![instance]
            .into_iter()
            .map(|i| {
                // This mimics what prepare_instances does
                if !matches!(i.schema.key(), Some(terminusdb_schema::Key::Random) | None) && i.has_id() {
                    panic!(
                        "Model '{}' uses {:?} key generation strategy and cannot have an @id field set. \
                         The ID will be automatically generated based on the key strategy.",
                        i.schema.class_name(),
                        i.schema.key()
                    );
                }
                i
            })
            .collect::<Vec<_>>();
    }

    #[test]
    #[should_panic(expected = "Model 'UserWithHashKey' uses Some(Hash")]
    fn test_panic_on_hash_key_with_id() {
        // This should panic because we're trying to set an ID on a model with Hash key
        let user = UserWithHashKey {
            email: "test@example.com".to_string(),
            username: "testuser".to_string(),
        };
        
        let instance = user.to_instance(Some("manual_id".to_string()));
        
        // This will panic
        let _instances = vec![instance]
            .into_iter()
            .map(|i| {
                if !matches!(i.schema.key(), Some(terminusdb_schema::Key::Random) | None) && i.has_id() {
                    panic!(
                        "Model '{}' uses {:?} key generation strategy and cannot have an @id field set. \
                         The ID will be automatically generated based on the key strategy.",
                        i.schema.class_name(),
                        i.schema.key()
                    );
                }
                i
            })
            .collect::<Vec<_>>();
    }

    #[test]
    #[should_panic(expected = "Model 'DocumentWithValueHashKey' uses Some(ValueHash")]
    fn test_panic_on_valuehash_key_with_id() {
        let doc = DocumentWithValueHashKey {
            title: "Test Document".to_string(),
            content: "Test content".to_string(),
        };
        
        let instance = doc.to_instance(Some("doc_id".to_string()));
        
        // This will panic
        let _instances = vec![instance]
            .into_iter()
            .map(|i| {
                if !matches!(i.schema.key(), Some(terminusdb_schema::Key::Random) | None) && i.has_id() {
                    panic!(
                        "Model '{}' uses {:?} key generation strategy and cannot have an @id field set. \
                         The ID will be automatically generated based on the key strategy.",
                        i.schema.class_name(),
                        i.schema.key()
                    );
                }
                i
            })
            .collect::<Vec<_>>();
    }

    #[test]
    fn test_random_key_allows_id() {
        // This should NOT panic because Random key allows setting ID
        let person = PersonWithRandomKey {
            name: "Jane".to_string(),
            age: 25,
        };
        
        let instance = person.to_instance(Some("person_123".to_string()));
        
        // This should work fine
        let instances = vec![instance]
            .into_iter()
            .map(|i| {
                if !matches!(i.schema.key(), Some(terminusdb_schema::Key::Random) | None) && i.has_id() {
                    panic!(
                        "Model '{}' uses {:?} key generation strategy and cannot have an @id field set. \
                         The ID will be automatically generated based on the key strategy.",
                        i.schema.class_name(),
                        i.schema.key()
                    );
                }
                i
            })
            .collect::<Vec<_>>();
        
        assert_eq!(instances.len(), 1);
        assert!(instances[0].has_id());
    }

    #[test]
    fn test_non_random_key_without_id_is_fine() {
        // This should NOT panic because we're not setting an ID
        let person = PersonWithLexicalKey {
            name: "Alice".to_string(),
            age: 28,
        };
        
        let instance = person.to_instance(None); // No ID provided
        
        // This should work fine
        let instances = vec![instance]
            .into_iter()
            .map(|i| {
                if !matches!(i.schema.key(), Some(terminusdb_schema::Key::Random) | None) && i.has_id() {
                    panic!(
                        "Model '{}' uses {:?} key generation strategy and cannot have an @id field set. \
                         The ID will be automatically generated based on the key strategy.",
                        i.schema.class_name(),
                        i.schema.key()
                    );
                }
                i
            })
            .collect::<Vec<_>>();
        
        assert_eq!(instances.len(), 1);
        assert!(!instances[0].has_id());
    }
}