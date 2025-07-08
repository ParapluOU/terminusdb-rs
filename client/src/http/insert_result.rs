//! Structured results for instance insertion operations

use std::collections::HashMap;
use crate::TDBInsertInstanceResult;

/// Result of inserting an instance with sub-entities
#[derive(Debug, Clone)]
pub struct InsertInstanceResult {
    /// The ID of the root instance that was inserted
    pub root_id: String,
    
    /// The result for the root instance (Inserted or AlreadyExists)
    pub root_result: TDBInsertInstanceResult,
    
    /// Results for all sub-entities that were inserted alongside the root
    /// Key is the instance ID, value is the insert result
    pub sub_entities: HashMap<String, TDBInsertInstanceResult>,
    
    /// All results including root and sub-entities
    /// This maintains backward compatibility
    pub all_results: HashMap<String, TDBInsertInstanceResult>,
}

impl InsertInstanceResult {
    /// Create a new InsertInstanceResult from a HashMap of results and the root ID
    pub fn new(
        all_results: HashMap<String, TDBInsertInstanceResult>,
        root_id: String,
    ) -> anyhow::Result<Self> {
        let root_result = all_results
            .get(&root_id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Root ID not found in results"))?;
        
        let mut sub_entities = all_results.clone();
        sub_entities.remove(&root_id);
        
        Ok(Self {
            root_id: root_id.clone(),
            root_result,
            sub_entities,
            all_results,
        })
    }
    
    /// Check if the root instance was newly inserted
    pub fn was_inserted(&self) -> bool {
        matches!(self.root_result, TDBInsertInstanceResult::Inserted(_))
    }
    
    /// Check if the root instance already existed
    pub fn already_existed(&self) -> bool {
        matches!(self.root_result, TDBInsertInstanceResult::AlreadyExists(_))
    }
    
    /// Get the total number of entities (root + sub-entities)
    pub fn total_entities(&self) -> usize {
        self.all_results.len()
    }
    
    /// Get the number of sub-entities
    pub fn sub_entity_count(&self) -> usize {
        self.sub_entities.len()
    }
}