//! Versioned Entity ID types that combine entity IDs with commit tracking.

use std::ops::Deref;

use serde::Serialize;
use terminusdb_schema::{EntityIDFor, TdbIRI, ToTDBSchema};

use crate::CommitId;

/// A versioned entity ID that combines an `EntityIDFor<T>` with a `CommitId`.
///
/// This type is useful for tracking which commit an entity was created or last modified in.
/// It implements `Deref` to allow transparent access to the underlying `EntityIDFor<T>` methods.
///
/// # Examples
///
/// ```rust,ignore
/// let id = EntityIDFor::<User>::random();
/// let commit = CommitId::new("abc123");
/// let versioned = VersionedEntityIDFor::new(id, commit);
///
/// // Access EntityIDFor methods via Deref
/// println!("ID: {}", versioned.typed());
/// println!("Commit: {}", versioned.version);
/// ```
#[derive(Debug, Clone, Eq, Hash, Serialize)]
pub struct VersionedEntityIDFor<T: ToTDBSchema> {
    /// The entity ID
    pub id: EntityIDFor<T>,
    /// The commit ID where this entity was created or last modified
    pub version: CommitId,
}

impl<T: ToTDBSchema> VersionedEntityIDFor<T> {
    /// Create a new versioned entity ID from an entity ID and commit ID.
    pub fn new(id: EntityIDFor<T>, version: CommitId) -> Self {
        Self { id, version }
    }
}

// Deref to EntityIDFor for transparent access to its methods
impl<T: ToTDBSchema> Deref for VersionedEntityIDFor<T> {
    type Target = EntityIDFor<T>;

    fn deref(&self) -> &Self::Target {
        &self.id
    }
}

// PartialEq implementations for convenient comparisons
impl<T: ToTDBSchema> PartialEq for VersionedEntityIDFor<T> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && self.version == other.version
    }
}

impl<T: ToTDBSchema> PartialEq<EntityIDFor<T>> for VersionedEntityIDFor<T> {
    fn eq(&self, other: &EntityIDFor<T>) -> bool {
        &self.id == other
    }
}

impl<T: ToTDBSchema> PartialEq<String> for VersionedEntityIDFor<T> {
    fn eq(&self, other: &String) -> bool {
        self.id.typed() == *other
    }
}

impl<T: ToTDBSchema> PartialEq<&str> for VersionedEntityIDFor<T> {
    fn eq(&self, other: &&str) -> bool {
        self.id.typed() == *other
    }
}

impl<T: ToTDBSchema> PartialEq<TdbIRI> for VersionedEntityIDFor<T> {
    fn eq(&self, other: &TdbIRI) -> bool {
        self.id.get_iri() == other
    }
}

// Note: We intentionally DO NOT implement schema traits like ToTDBSchema, ToInstanceProperty,
// FromInstanceProperty, or InstancePropertyFromJson because:
// 1. VersionedEntityIDFor is a client-side utility type, not a schema type
// 2. It should always have a CommitId, which cannot be constructed from instance data
// 3. The inner EntityIDFor can be accessed via Deref for schema operations

#[cfg(test)]
mod tests {
    use super::*;
    use terminusdb_schema::*;
    use terminusdb_schema_derive::TerminusDBModel;

    #[derive(Debug, Clone, TerminusDBModel, serde::Serialize)]
    struct TestEntity {
        name: String,
    }

    #[test]
    fn test_new_and_deref() {
        let id = EntityIDFor::<TestEntity>::new_unchecked("TestEntity/test123").unwrap();
        let commit = CommitId::new("commit456");
        let versioned = VersionedEntityIDFor::new(id.clone(), commit.clone());

        // Test deref access
        assert_eq!(versioned.typed(), id.typed());
        assert_eq!(versioned.version, commit);
    }

    #[test]
    fn test_partial_eq_entity_id() {
        let id = EntityIDFor::<TestEntity>::new_unchecked("TestEntity/test123").unwrap();
        let commit = CommitId::new("commit456");
        let versioned = VersionedEntityIDFor::new(id.clone(), commit);

        assert_eq!(versioned, id);
    }

    #[test]
    fn test_partial_eq_string() {
        let id = EntityIDFor::<TestEntity>::new_unchecked("TestEntity/test123").unwrap();
        let commit = CommitId::new("commit456");
        let versioned = VersionedEntityIDFor::new(id.clone(), commit);

        assert_eq!(versioned, id.typed());
        assert_eq!(versioned, "TestEntity/test123");
    }

    #[test]
    fn test_partial_eq_tdb_iri() {
        let id = EntityIDFor::<TestEntity>::new_unchecked("TestEntity/test123").unwrap();
        let commit = CommitId::new("commit456");
        let versioned = VersionedEntityIDFor::new(id.clone(), commit);

        assert_eq!(versioned, *id.get_iri());
    }

    #[test]
    fn test_serialize() {
        let id = EntityIDFor::<TestEntity>::new_unchecked("TestEntity/test123").unwrap();
        let commit = CommitId::new("commit456");
        let versioned = VersionedEntityIDFor::new(id, commit);

        let json = serde_json::to_value(&versioned).unwrap();
        assert_eq!(json["id"], "TestEntity/test123");
        assert_eq!(json["version"], "commit456");
    }

    #[test]
    fn test_clone_and_equality() {
        let id = EntityIDFor::<TestEntity>::new_unchecked("TestEntity/test123").unwrap();
        let commit = CommitId::new("commit456");
        let versioned1 = VersionedEntityIDFor::new(id.clone(), commit.clone());
        let versioned2 = versioned1.clone();

        assert_eq!(versioned1, versioned2);
    }

    #[test]
    fn test_different_versions_not_equal() {
        let id = EntityIDFor::<TestEntity>::new_unchecked("TestEntity/test123").unwrap();
        let commit1 = CommitId::new("commit456");
        let commit2 = CommitId::new("commit789");
        let versioned1 = VersionedEntityIDFor::new(id.clone(), commit1);
        let versioned2 = VersionedEntityIDFor::new(id, commit2);

        assert_ne!(versioned1, versioned2);
    }
}
