//! Structured results for instance insertion operations

use crate::{CommitId, TDBInsertInstanceResult, VersionedEntityIDFor};
use std::collections::HashMap;
use std::ops::Deref;
use terminusdb_schema::{EntityIDFor, TdbIRI, ToTDBSchema};
use anyhow::anyhow;

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
    pub commit_id: Option<CommitId>,
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
    pub fn extract_commit_id(&self) -> Option<CommitId> {
        self.commit_id.as_ref().and_then(|commit_id| {
            // Split on ':' and take the last part (the actual commit ID)
            commit_id.as_str().split(':').last().map(CommitId::from)
        })
    }

    /// Get the root ID as a typed EntityIDFor<T>
    pub fn root_ref<T: ToTDBSchema>(&self) -> anyhow::Result<EntityIDFor<T>> {
        EntityIDFor::new_unchecked(&self.root_id)
    }
    
    /// Get the parsed IRI for the root instance
    pub fn get_root_iri(&self) -> anyhow::Result<TdbIRI> {
        TdbIRI::parse(&self.root_id)
    }
    
    /// Extract the type name and ID from the root instance
    /// Returns (type_name, id)
    pub fn root_ref_parts(&self) -> anyhow::Result<(String, String)> {
        let iri = self.get_root_iri()?;
        Ok((iri.type_name().to_string(), iri.id().to_string()))
    }

    /// Get the root entity as a versioned reference.
    ///
    /// Combines the root entity ID with the commit ID from the response.
    /// Requires a commit ID to be present in the result.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let result = client.insert_instance_with_commit_id(&person, args).await?;
    /// let versioned_ref: VersionedEntityIDFor<Person> = result.0.root_versioned_ref()?;
    ///
    /// println!("Person {} at commit {}", versioned_ref.id, versioned_ref.version);
    /// ```
    pub fn root_versioned_ref<T: ToTDBSchema>(&self) -> anyhow::Result<VersionedEntityIDFor<T>> {
        let commit_id = self.extract_commit_id()
            .ok_or_else(|| anyhow!("No commit ID in insert result"))?;

        let entity_id = self.root_ref::<T>()?;
        Ok(VersionedEntityIDFor::new(entity_id, commit_id))
    }

    /// Get all entities (root + sub-entities) as versioned references, filtered by type T.
    ///
    /// Only includes entities whose IDs successfully parse as `EntityIDFor<T>`.
    /// Since sub-entities may be of different types, this filters to only include type T.
    /// Requires a commit ID to be present in the result.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let result = client.insert_instance_with_commit_id(&company, args).await?;
    ///
    /// // Get all Person entities (might be employees as sub-entities)
    /// let person_refs: Vec<VersionedEntityIDFor<Person>> = result.0.all_versioned_refs()?;
    ///
    /// // Get all Company entities (just the root in this case)
    /// let company_refs: Vec<VersionedEntityIDFor<Company>> = result.0.all_versioned_refs()?;
    /// ```
    pub fn all_versioned_refs<T: ToTDBSchema>(&self) -> anyhow::Result<Vec<VersionedEntityIDFor<T>>> {
        let commit_id = self.extract_commit_id()
            .ok_or_else(|| anyhow!("No commit ID in insert result"))?;

        let mut refs = Vec::new();

        // Try to add root if it's type T
        if let Ok(root_id) = EntityIDFor::<T>::new_unchecked(&self.root_id) {
            refs.push(VersionedEntityIDFor::new(root_id, commit_id.clone()));
        }

        // Filter sub-entities to only type T
        for (id_str, _result) in &self.sub_entities {
            if let Ok(entity_id) = EntityIDFor::<T>::new_unchecked(id_str) {
                refs.push(VersionedEntityIDFor::new(entity_id, commit_id.clone()));
            }
        }

        Ok(refs)
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

#[cfg(test)]
mod tests {
    use super::*;
    use terminusdb_schema::*;
    use terminusdb_schema_derive::TerminusDBModel;

    #[derive(Debug, Clone, TerminusDBModel)]
    struct Person {
        name: String,
    }

    #[derive(Debug, Clone, TerminusDBModel)]
    struct Company {
        name: String,
    }

    #[test]
    fn test_root_versioned_ref_success() {
        let mut results = HashMap::new();
        results.insert(
            "Person/123".to_string(),
            TDBInsertInstanceResult::Inserted("Person/123".to_string()),
        );

        let mut result = InsertInstanceResult::new(results, "Person/123".to_string()).unwrap();
        result.commit_id = Some(CommitId::new("branch:abc123"));

        let versioned_ref: VersionedEntityIDFor<Person> = result.root_versioned_ref().unwrap();

        assert_eq!(versioned_ref.id.typed(), "Person/123");
        assert_eq!(versioned_ref.version.as_str(), "abc123");
    }

    #[test]
    fn test_root_versioned_ref_missing_commit_id() {
        let mut results = HashMap::new();
        results.insert(
            "Person/123".to_string(),
            TDBInsertInstanceResult::Inserted("Person/123".to_string()),
        );

        let result = InsertInstanceResult::new(results, "Person/123".to_string()).unwrap();
        // No commit_id set

        let err = result.root_versioned_ref::<Person>().unwrap_err();
        assert!(err.to_string().contains("No commit ID"));
    }

    #[test]
    fn test_all_versioned_refs_filters_by_type() {
        let mut results = HashMap::new();
        results.insert(
            "Person/123".to_string(),
            TDBInsertInstanceResult::Inserted("Person/123".to_string()),
        );
        results.insert(
            "Company/456".to_string(),
            TDBInsertInstanceResult::Inserted("Company/456".to_string()),
        );
        results.insert(
            "Person/789".to_string(),
            TDBInsertInstanceResult::Inserted("Person/789".to_string()),
        );

        let mut result = InsertInstanceResult::new(results, "Person/123".to_string()).unwrap();
        result.commit_id = Some(CommitId::new("branch:abc123"));

        // Get only Person refs
        let person_refs: Vec<VersionedEntityIDFor<Person>> = result.all_versioned_refs().unwrap();
        assert_eq!(person_refs.len(), 2); // Person/123 and Person/789

        // Get only Company refs
        let company_refs: Vec<VersionedEntityIDFor<Company>> = result.all_versioned_refs().unwrap();
        assert_eq!(company_refs.len(), 1); // Company/456
    }

    #[test]
    fn test_all_versioned_refs_missing_commit_id() {
        let mut results = HashMap::new();
        results.insert(
            "Person/123".to_string(),
            TDBInsertInstanceResult::Inserted("Person/123".to_string()),
        );

        let result = InsertInstanceResult::new(results, "Person/123".to_string()).unwrap();
        // No commit_id set

        let err = result.all_versioned_refs::<Person>().unwrap_err();
        assert!(err.to_string().contains("No commit ID"));
    }
}
