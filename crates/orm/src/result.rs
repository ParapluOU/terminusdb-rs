//! Result wrapper for multi-type fetch operations.
//!
//! Provides a container for documents loaded from TerminusDB,
//! with type-safe accessors for extracting specific types.

use std::collections::HashMap;

use terminusdb_schema::{json::InstanceFromJson, FromTDBInstance, ToTDBSchema};

/// Result container for ORM fetch operations.
///
/// Stores raw JSON documents and provides on-demand type-safe
/// conversion to model types. This allows a single fetch to retrieve
/// documents of multiple types, which are then extracted by type.
///
/// # Example
/// ```ignore
/// let result: OrmResult = query.execute().await?;
///
/// // Get all comments
/// let comments: Vec<Comment> = result.get::<Comment>()?;
///
/// // Get replies indexed by ID
/// let replies: HashMap<String, Reply> = result.get_indexed::<Reply>()?;
/// ```
#[derive(Debug, Clone)]
pub struct OrmResult {
    /// Raw JSON documents, each with an `@type` field indicating its class.
    documents: Vec<serde_json::Value>,
}

impl OrmResult {
    /// Create a new result from a vector of JSON documents.
    pub fn new(documents: Vec<serde_json::Value>) -> Self {
        Self { documents }
    }

    /// Create an empty result.
    pub fn empty() -> Self {
        Self {
            documents: Vec::new(),
        }
    }

    /// Get the raw JSON documents.
    pub fn documents(&self) -> &[serde_json::Value] {
        &self.documents
    }

    /// Get the raw documents mutably.
    pub fn documents_mut(&mut self) -> &mut Vec<serde_json::Value> {
        &mut self.documents
    }

    /// Consume and return the inner documents.
    pub fn into_documents(self) -> Vec<serde_json::Value> {
        self.documents
    }

    /// Get the number of documents.
    pub fn len(&self) -> usize {
        self.documents.len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.documents.is_empty()
    }

    /// Extract the `@type` field from a JSON document.
    fn get_type(doc: &serde_json::Value) -> Option<&str> {
        doc.get("@type").and_then(|v| v.as_str())
    }

    /// Extract the `@id` field from a JSON document.
    fn get_id(doc: &serde_json::Value) -> Option<&str> {
        doc.get("@id").and_then(|v| v.as_str())
    }

    /// Extract all documents of type T.
    ///
    /// Filters documents by matching `@type` (class name) and deserializes
    /// matching documents into the target type.
    ///
    /// # Type Parameters
    /// - `T`: Must implement `FromTDBInstance` + `InstanceFromJson` for deserialization
    ///   and `ToTDBSchema` to determine the type name.
    ///
    /// # Returns
    /// A vector of deserialized instances, or an error if deserialization fails.
    pub fn get<T>(&self) -> anyhow::Result<Vec<T>>
    where
        T: FromTDBInstance + InstanceFromJson + ToTDBSchema,
    {
        let target_class = T::to_schema().class_name().to_string();

        self.documents
            .iter()
            .filter(|doc| Self::get_type(doc) == Some(&target_class))
            .map(|doc| T::from_json(doc.clone()))
            .collect()
    }

    /// Extract a single instance of type T, if present.
    ///
    /// Returns `None` if no matching documents are found.
    /// Returns an error if multiple matching documents are found.
    pub fn get_one<T>(&self) -> anyhow::Result<Option<T>>
    where
        T: FromTDBInstance + InstanceFromJson + ToTDBSchema,
    {
        let mut items = self.get::<T>()?;
        match items.len() {
            0 => Ok(None),
            1 => Ok(Some(items.remove(0))),
            n => Err(anyhow::anyhow!(
                "Expected at most 1 instance of {}, found {}",
                std::any::type_name::<T>(),
                n
            )),
        }
    }

    /// Extract all documents of type T, indexed by their ID.
    ///
    /// Useful for quick lookups when assembling relations.
    ///
    /// # Returns
    /// A HashMap from document ID to deserialized instance.
    pub fn get_indexed<T>(&self) -> anyhow::Result<HashMap<String, T>>
    where
        T: FromTDBInstance + InstanceFromJson + ToTDBSchema,
    {
        let target_class = T::to_schema().class_name().to_string();

        self.documents
            .iter()
            .filter(|doc| Self::get_type(doc) == Some(&target_class))
            .map(|doc| {
                let id = Self::get_id(doc)
                    .ok_or_else(|| {
                        anyhow::anyhow!("Document of {} has no @id", std::any::type_name::<T>())
                    })?
                    .to_string();
                let value = T::from_json(doc.clone())?;
                Ok((id, value))
            })
            .collect()
    }

    /// Get all unique class names present in the result.
    pub fn class_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .documents
            .iter()
            .filter_map(|doc| Self::get_type(doc).map(|s| s.to_string()))
            .collect();
        names.sort();
        names.dedup();
        names
    }

    /// Count documents by class name.
    pub fn count_by_class(&self) -> HashMap<String, usize> {
        let mut counts = HashMap::new();
        for doc in &self.documents {
            if let Some(name) = Self::get_type(doc) {
                *counts.entry(name.to_string()).or_insert(0) += 1;
            }
        }
        counts
    }

    /// Check if the result contains any documents of type T.
    pub fn contains<T>(&self) -> bool
    where
        T: ToTDBSchema,
    {
        let target_class = T::to_schema().class_name().to_string();
        self.documents
            .iter()
            .any(|doc| Self::get_type(doc) == Some(&target_class))
    }

    /// Merge another result into this one.
    pub fn merge(&mut self, other: OrmResult) {
        self.documents.extend(other.documents);
    }

    /// Combine two results into a new one.
    pub fn combine(mut self, other: OrmResult) -> Self {
        self.merge(other);
        self
    }

    // =========================================================================
    // Grouped Relation Access (for relation resolution)
    // =========================================================================

    /// Extract documents of type T grouped by a foreign key field.
    ///
    /// This is useful for efficiently associating child entities with their parents.
    /// For example, grouping Comments by their post_id.
    ///
    /// # Arguments
    /// * `field_name` - The name of the foreign key field to group by
    ///
    /// # Returns
    /// A HashMap from parent ID to list of child entities.
    ///
    /// # Example
    /// ```ignore
    /// // Get comments grouped by post_id
    /// let comments_by_post: HashMap<String, Vec<Comment>> =
    ///     result.get_grouped_by_field::<Comment>("post_id")?;
    /// ```
    pub fn get_grouped_by_field<T>(
        &self,
        field_name: &str,
    ) -> anyhow::Result<HashMap<String, Vec<T>>>
    where
        T: FromTDBInstance + InstanceFromJson + ToTDBSchema,
    {
        let target_class = T::to_schema().class_name().to_string();
        let mut grouped: HashMap<String, Vec<T>> = HashMap::new();

        for doc in &self.documents {
            if Self::get_type(doc) != Some(&target_class) {
                continue;
            }

            // Extract the foreign key value
            let fk_value = doc.get(field_name).and_then(|v| {
                // Handle both string IDs and object references with @id
                if let Some(s) = v.as_str() {
                    Some(s.to_string())
                } else if let Some(obj) = v.as_object() {
                    obj.get("@id")
                        .and_then(|id| id.as_str())
                        .map(|s| s.to_string())
                } else {
                    None
                }
            });

            if let Some(fk) = fk_value {
                let entity = T::from_json(doc.clone())?;
                grouped.entry(fk).or_default().push(entity);
            }
        }

        Ok(grouped)
    }

    /// Get all IDs of documents of type T.
    ///
    /// Useful for collecting IDs to use in batch loading of related entities.
    pub fn get_ids<T>(&self) -> Vec<String>
    where
        T: ToTDBSchema,
    {
        let target_class = T::to_schema().class_name().to_string();
        self.documents
            .iter()
            .filter(|doc| Self::get_type(doc) == Some(&target_class))
            .filter_map(|doc| Self::get_id(doc).map(|s| s.to_string()))
            .collect()
    }

    /// Extract values of a specific field from documents of type T.
    ///
    /// Useful for collecting foreign key values for batch loading.
    ///
    /// # Arguments
    /// * `field_name` - The field to extract values from
    ///
    /// # Returns
    /// A vector of field values (as strings).
    pub fn get_field_values<T>(&self, field_name: &str) -> Vec<String>
    where
        T: ToTDBSchema,
    {
        let target_class = T::to_schema().class_name().to_string();
        self.documents
            .iter()
            .filter(|doc| Self::get_type(doc) == Some(&target_class))
            .filter_map(|doc| {
                doc.get(field_name).and_then(|v| {
                    if let Some(s) = v.as_str() {
                        Some(s.to_string())
                    } else if let Some(obj) = v.as_object() {
                        obj.get("@id")
                            .and_then(|id| id.as_str())
                            .map(|s| s.to_string())
                    } else {
                        None
                    }
                })
            })
            .collect()
    }

    /// Extract values from a Vec field (for many-to-many relations).
    ///
    /// # Arguments
    /// * `field_name` - The Vec field to extract values from
    ///
    /// # Returns
    /// A vector of all referenced IDs, flattened.
    pub fn get_vec_field_values<T>(&self, field_name: &str) -> Vec<String>
    where
        T: ToTDBSchema,
    {
        let target_class = T::to_schema().class_name().to_string();
        self.documents
            .iter()
            .filter(|doc| Self::get_type(doc) == Some(&target_class))
            .filter_map(|doc| doc.get(field_name))
            .filter_map(|v| v.as_array())
            .flat_map(|arr| {
                arr.iter().filter_map(|item| {
                    if let Some(s) = item.as_str() {
                        Some(s.to_string())
                    } else if let Some(obj) = item.as_object() {
                        obj.get("@id")
                            .and_then(|id| id.as_str())
                            .map(|s| s.to_string())
                    } else {
                        None
                    }
                })
            })
            .collect()
    }
}

impl From<Vec<serde_json::Value>> for OrmResult {
    fn from(documents: Vec<serde_json::Value>) -> Self {
        Self::new(documents)
    }
}

impl IntoIterator for OrmResult {
    type Item = serde_json::Value;
    type IntoIter = std::vec::IntoIter<serde_json::Value>;

    fn into_iter(self) -> std::vec::IntoIter<serde_json::Value> {
        self.documents.into_iter()
    }
}

impl<'a> IntoIterator for &'a OrmResult {
    type Item = &'a serde_json::Value;
    type IntoIter = std::slice::Iter<'a, serde_json::Value>;

    fn into_iter(self) -> std::slice::Iter<'a, serde_json::Value> {
        self.documents.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_result() {
        let result = OrmResult::empty();
        assert!(result.is_empty());
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_class_names() {
        let result = OrmResult::empty();
        assert!(result.class_names().is_empty());
    }
}
