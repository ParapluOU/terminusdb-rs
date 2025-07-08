//! Structured results for instance insertion operations

use std::collections::HashMap;
use std::ops::Deref;
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
    
    /// The commit ID that created/modified these instances
    pub commit_id: Option<String>,
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
        
        let mut sub_entities = all_results;
        sub_entities.remove(&root_id);
        
        Ok(Self {
            root_id,
            root_result,
            sub_entities,
            commit_id: None,
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
        1 + self.sub_entities.len()
    }
    
    /// Get the number of sub-entities
    pub fn sub_entity_count(&self) -> usize {
        self.sub_entities.len()
    }
    
    /// Extract the commit ID from the TerminusDB-Data-Version header
    /// Format is typically "branch:COMMIT_ID", this returns just the COMMIT_ID part
    pub fn extract_commit_id(&self) -> Option<String> {
        self.commit_id.as_ref().and_then(|header_value| {
            // Split on ':' and take the last part (the actual commit ID)
            header_value.split(':').last().map(|s| s.to_string())
        })
    }
}

impl Deref for InsertInstanceResult {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.root_id
    }
}

impl AsRef<str> for InsertInstanceResult {
    fn as_ref(&self) -> &str {
        &self.root_id
    }
}