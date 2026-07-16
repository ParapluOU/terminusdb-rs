//! Ergonomic per-field conversion for building a model from a tuple.
//!
//! `#[derive(FromTuple)]` generates `From<(A, B, …)> for Model` that calls
//! [`IntoField::into_field`] on each tuple element. This is a **local** trait
//! rather than [`From`]/[`Into`] so we can offer conversions the orphan rule
//! forbids for `From` — notably `Option<&str> -> Option<String>` — and so we can
//! turn a bare `&str` into a typed id or link.
//!
//! The `&str -> id/link` conversions **panic** on an unparseable id. That is
//! intended for fixtures, examples, and tests (where ids are literals), not for
//! untrusted input.
//!
//! ```ignore
//! #[derive(TerminusDBModel, FromTuple)]
//! struct Book { id: EntityIDFor<Self>, title: String, author: Ref<Author> }
//!
//! let b = Book::from(("hp1", "Philosopher's Stone", "rowling"));
//! ```

use crate::{EntityIDFor, TdbLazy, TerminusDBModel, ToTDBSchema};

/// Convert a convenient source value into a model field of type `T`.
pub trait IntoField<T> {
    fn into_field(self) -> T;
}

/// Reflexive: any value converts to its own type unchanged. Mirrors
/// `impl<T> From<T> for T` — specific cross-type impls below coexist with it just
/// as `From<&str> for String` coexists with reflexive `From` in std.
impl<T> IntoField<T> for T {
    fn into_field(self) -> T {
        self
    }
}

impl IntoField<String> for &str {
    fn into_field(self) -> String {
        self.to_string()
    }
}

// --- ids -----------------------------------------------------------------------

impl<M: ToTDBSchema> IntoField<EntityIDFor<M>> for &str {
    fn into_field(self) -> EntityIDFor<M> {
        EntityIDFor::new_untyped(self).expect("IntoField: invalid entity id")
    }
}
impl<M: ToTDBSchema> IntoField<EntityIDFor<M>> for String {
    fn into_field(self) -> EntityIDFor<M> {
        EntityIDFor::new_untyped(&self).expect("IntoField: invalid entity id")
    }
}

// --- links (Ref<M> / TdbLazy<M>) ----------------------------------------------

impl<M: TerminusDBModel> IntoField<TdbLazy<M>> for &str {
    fn into_field(self) -> TdbLazy<M> {
        TdbLazy::from(EntityIDFor::<M>::new_untyped(self).expect("IntoField: invalid entity id"))
    }
}
impl<M: TerminusDBModel> IntoField<TdbLazy<M>> for String {
    fn into_field(self) -> TdbLazy<M> {
        TdbLazy::from(EntityIDFor::<M>::new_untyped(&self).expect("IntoField: invalid entity id"))
    }
}

// --- optionals ----------------------------------------------------------------
// Reflexive already covers `Option<T> -> Option<T>`; this adds the common
// `Option<&str> -> Option<String>` that the orphan rule forbids for `From`.

impl IntoField<Option<String>> for Option<&str> {
    fn into_field(self) -> Option<String> {
        self.map(str::to_string)
    }
}
impl IntoField<Option<String>> for &str {
    fn into_field(self) -> Option<String> {
        Some(self.to_string())
    }
}
