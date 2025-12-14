//! Relation wrapper types and traits for ORM.
//!
//! Provides both:
//! - **Wrapper types** (`HasOneField`, `HasManyField`, `BelongsToField`) for field storage
//! - **Traits** (`HasOne`, `HasMany`, `BelongsTo`) for compile-time relation reasoning

use std::ops::{Deref, DerefMut};

use terminusdb_relation::RelationField;
use terminusdb_schema::{EntityIDFor, TdbLazy, TerminusDBModel};

use crate::result::OrmResult;

// ============================================================================
// Re-export ORM relation traits from terminusdb_relation
// ============================================================================

pub use terminusdb_relation::{BelongsTo, DefaultField, ForwardRelation, ReverseRelation};

// ============================================================================
// Additional Relation Traits (for compile-time reasoning)
// ============================================================================

/// Trait for models that have one related entity of type T.
///
/// Used for compile-time validation of `.with::<T>()` calls.
pub trait HasOne<T: TerminusDBModel, Field: RelationField = DefaultField> {
    /// Get the field name for this relation.
    fn field_name() -> &'static str {
        Field::field_name()
    }

    /// Set loaded data from a result container.
    fn set_from_result(&mut self, result: &OrmResult) -> anyhow::Result<()>;
}

/// Trait for models that have many related entities of type T.
///
/// Used for compile-time validation of `.with::<T>()` calls.
pub trait HasMany<T: TerminusDBModel, Field: RelationField = DefaultField> {
    /// Get the field name for this relation.
    fn field_name() -> &'static str {
        Field::field_name()
    }

    /// Set loaded data from a result container.
    fn set_from_result(&mut self, result: &OrmResult) -> anyhow::Result<()>;
}

// ============================================================================
// Wrapper Types (for field storage with lazy loading)
// ============================================================================

/// One-to-one relation field (owning side).
///
/// Wraps `TdbLazy<T>` for models that own a single related entity.
///
/// # Example
/// ```ignore
/// #[derive(TerminusDBModel)]
/// struct User {
///     profile: HasOneField<UserProfile>,
/// }
/// ```
#[derive(Debug, Clone)]
pub struct HasOneField<T: TerminusDBModel>(TdbLazy<T>);

impl<T: TerminusDBModel> HasOneField<T> {
    /// Create from a TdbLazy instance.
    pub fn new(lazy: TdbLazy<T>) -> Self {
        Self(lazy)
    }

    /// Create from just an ID (for lazy loading).
    pub fn from_id(id: EntityIDFor<T>) -> Self {
        Self(TdbLazy::from(id))
    }

    /// Create from an ID string (for lazy loading).
    pub fn from_id_str(id: &str) -> anyhow::Result<Self> {
        Ok(Self(TdbLazy::new_id(id)?))
    }

    /// Create from loaded data.
    pub fn from_data(data: T) -> anyhow::Result<Self> {
        Ok(Self(TdbLazy::new_data(data)?))
    }

    /// Get the inner TdbLazy.
    pub fn into_inner(self) -> TdbLazy<T> {
        self.0
    }

    /// Check if the data is already loaded.
    pub fn is_loaded(&self) -> bool {
        self.0.is_loaded()
    }

    /// Get the entity ID.
    pub fn id(&self) -> &EntityIDFor<T> {
        self.0.id()
    }
}

impl<T: TerminusDBModel> Deref for HasOneField<T> {
    type Target = TdbLazy<T>;

    fn deref(&self) -> &TdbLazy<T> {
        &self.0
    }
}

impl<T: TerminusDBModel> DerefMut for HasOneField<T> {
    fn deref_mut(&mut self) -> &mut TdbLazy<T> {
        &mut self.0
    }
}

impl<T: TerminusDBModel> From<TdbLazy<T>> for HasOneField<T> {
    fn from(lazy: TdbLazy<T>) -> Self {
        Self(lazy)
    }
}

/// One-to-many relation field.
///
/// Wraps `Vec<TdbLazy<T>>` for models that own multiple related entities.
///
/// # Example
/// ```ignore
/// #[derive(TerminusDBModel)]
/// struct User {
///     posts: HasManyField<Post>,
/// }
/// ```
#[derive(Debug, Clone)]
pub struct HasManyField<T: TerminusDBModel>(Vec<TdbLazy<T>>);

impl<T: TerminusDBModel> HasManyField<T> {
    /// Create an empty collection.
    pub fn new() -> Self {
        Self(Vec::new())
    }

    /// Create from a vector of TdbLazy instances.
    pub fn from_vec(items: Vec<TdbLazy<T>>) -> Self {
        Self(items)
    }

    /// Push a new lazy reference.
    pub fn push(&mut self, item: TdbLazy<T>) {
        self.0.push(item);
    }

    /// Get the number of items.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Iterate over references.
    pub fn iter(&self) -> impl Iterator<Item = &TdbLazy<T>> {
        self.0.iter()
    }

    /// Iterate mutably.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut TdbLazy<T>> {
        self.0.iter_mut()
    }

    /// Get the inner vector.
    pub fn into_inner(self) -> Vec<TdbLazy<T>> {
        self.0
    }
}

impl<T: TerminusDBModel> Default for HasManyField<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: TerminusDBModel> Deref for HasManyField<T> {
    type Target = Vec<TdbLazy<T>>;

    fn deref(&self) -> &Vec<TdbLazy<T>> {
        &self.0
    }
}

impl<T: TerminusDBModel> DerefMut for HasManyField<T> {
    fn deref_mut(&mut self) -> &mut Vec<TdbLazy<T>> {
        &mut self.0
    }
}

impl<T: TerminusDBModel> From<Vec<TdbLazy<T>>> for HasManyField<T> {
    fn from(items: Vec<TdbLazy<T>>) -> Self {
        Self(items)
    }
}

impl<T: TerminusDBModel> IntoIterator for HasManyField<T> {
    type Item = TdbLazy<T>;
    type IntoIter = std::vec::IntoIter<TdbLazy<T>>;

    fn into_iter(self) -> std::vec::IntoIter<TdbLazy<T>> {
        self.0.into_iter()
    }
}

impl<'a, T: TerminusDBModel> IntoIterator for &'a HasManyField<T> {
    type Item = &'a TdbLazy<T>;
    type IntoIter = std::slice::Iter<'a, TdbLazy<T>>;

    fn into_iter(self) -> std::slice::Iter<'a, TdbLazy<T>> {
        self.0.iter()
    }
}

/// Inverse relation field (foreign key side).
///
/// Wraps `TdbLazy<T>` for models that reference a parent entity.
///
/// # Example
/// ```ignore
/// #[derive(TerminusDBModel)]
/// struct Reply {
///     parent_comment: BelongsToField<Comment>,
/// }
/// ```
#[derive(Debug, Clone)]
pub struct BelongsToField<T: TerminusDBModel>(TdbLazy<T>);

impl<T: TerminusDBModel> BelongsToField<T> {
    /// Create from a TdbLazy instance.
    pub fn new(lazy: TdbLazy<T>) -> Self {
        Self(lazy)
    }

    /// Create from just an ID (for lazy loading).
    pub fn from_id(id: EntityIDFor<T>) -> Self {
        Self(TdbLazy::from(id))
    }

    /// Create from an ID string (for lazy loading).
    pub fn from_id_str(id: &str) -> anyhow::Result<Self> {
        Ok(Self(TdbLazy::new_id(id)?))
    }

    /// Create from loaded data.
    pub fn from_data(data: T) -> anyhow::Result<Self> {
        Ok(Self(TdbLazy::new_data(data)?))
    }

    /// Get the inner TdbLazy.
    pub fn into_inner(self) -> TdbLazy<T> {
        self.0
    }

    /// Check if the data is already loaded.
    pub fn is_loaded(&self) -> bool {
        self.0.is_loaded()
    }

    /// Get the entity ID (the foreign key).
    pub fn id(&self) -> &EntityIDFor<T> {
        self.0.id()
    }
}

impl<T: TerminusDBModel> Deref for BelongsToField<T> {
    type Target = TdbLazy<T>;

    fn deref(&self) -> &TdbLazy<T> {
        &self.0
    }
}

impl<T: TerminusDBModel> DerefMut for BelongsToField<T> {
    fn deref_mut(&mut self) -> &mut TdbLazy<T> {
        &mut self.0
    }
}

impl<T: TerminusDBModel> From<TdbLazy<T>> for BelongsToField<T> {
    fn from(lazy: TdbLazy<T>) -> Self {
        Self(lazy)
    }
}
