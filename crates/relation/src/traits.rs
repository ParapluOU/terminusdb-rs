//! Compositional relation traits for generating typed WOQL constraints from model relationships

use terminusdb_schema::{EntityIDFor, TerminusDBModel};
use terminusdb_woql2::prelude::{Query, Value};
// Import macros so type_! macro can find them
use terminusdb_woql2::{triple, typename};

/// Marker type for relation fields - each field gets its own unique type
pub trait RelationField {
    fn field_name() -> &'static str;
}

/// Default field marker
pub struct DefaultField;
impl RelationField for DefaultField {
    fn field_name() -> &'static str {
        "default"
    }
}

/// Forward relation: Self has a relation to Target
///
/// This trait is automatically implemented by the TerminusDBModel derive macro
/// for each relation field. The where constraints are checked at method call time.
pub trait RelationTo<Target, Field = DefaultField> {
    /// INTERNAL: Unchecked constraint generation for derive macro use only
    /// This method has no where bounds and should not be called directly by users
    fn _constraints_with_vars_unchecked(source_var: &str, target_var: &str) -> Query;

    /// Generate WOQL constraints with custom variable names
    /// This is the public API with proper type safety checks
    fn constraints_with_vars(source_var: &str, target_var: &str) -> Query
    where
        Self: TerminusDBModel,
        Target: TerminusDBModel,
        Field: RelationField,
    {
        Self::_constraints_with_vars_unchecked(source_var, target_var)
    }

    /// Generate WOQL constraints using schema names as variables
    /// Default implementation that calls constraints_with_vars
    fn constraints() -> Query
    where
        Self: TerminusDBModel,
        Target: TerminusDBModel,
        Field: RelationField,
    {
        Self::constraints_with_vars(
            &<Self as terminusdb_schema::ToSchemaClass>::to_class(),
            &<Target as terminusdb_schema::ToSchemaClass>::to_class(),
        )
    }
}

/// Reverse relation: Target has a relation to Self
///
/// This trait is automatically implemented for any type that has a RelationTo implementation
pub trait RelationFrom<Target, Field = DefaultField> {
    /// INTERNAL: Unchecked constraint generation for derive macro use only
    /// This method has no where bounds and should not be called directly by users
    fn _constraints_with_vars_unchecked(source_var: &str, target_var: &str) -> Query;

    /// Generate WOQL constraints with custom variable names
    /// Note the reversed variable order compared to RelationTo
    fn constraints_with_vars(source_var: &str, target_var: &str) -> Query
    where
        Self: TerminusDBModel,
        Target: TerminusDBModel,
        Field: RelationField,
    {
        Self::_constraints_with_vars_unchecked(source_var, target_var)
    }

    /// Generate WOQL constraints using schema names as variables
    fn constraints() -> Query
    where
        Self: TerminusDBModel,
        Target: TerminusDBModel,
        Field: RelationField,
    {
        Self::constraints_with_vars(
            &<Self as terminusdb_schema::ToSchemaClass>::to_class(),
            &<Target as terminusdb_schema::ToSchemaClass>::to_class(),
        )
    }
}

// Automatic implementation: If A relates to B, then B is related from A
impl<Source, Target, Field> RelationFrom<Source, Field> for Target
where
    Source: RelationTo<Target, Field> + TerminusDBModel,
    Target: TerminusDBModel,
    Field: RelationField,
{
    fn _constraints_with_vars_unchecked(source_var: &str, target_var: &str) -> Query {
        // Delegate to the RelationTo implementation with swapped variables
        <Source as RelationTo<Target, Field>>::_constraints_with_vars_unchecked(
            target_var, source_var,
        )
    }
}

/// Helper function to create basic relation constraints
pub fn basic_relation_constraints<Source, Target>(
    field_name: &str,
    source_var: &str,
    target_var: &str,
    is_optional: bool,
) -> Query
where
    Source: TerminusDBModel,
    Target: TerminusDBModel,
{
    let constraint = terminusdb_woql2::and!(
        terminusdb_woql2::triple!(
            terminusdb_woql2::var!(source_var),
            field_name,
            terminusdb_woql2::var!(target_var)
        )
        .into(),
        terminusdb_woql2::type_!(terminusdb_woql2::var!(source_var), Source).into(),
        terminusdb_woql2::type_!(terminusdb_woql2::var!(target_var), Target).into()
    );

    if is_optional {
        terminusdb_woql2::optional!(constraint)
    } else {
        constraint
    }
}

/// Helper function for deriving - generates constraints without needing type params at runtime
pub fn generate_relation_constraints(
    field_name: &str,
    source_type: &str,
    target_type: &str,
    source_var: &str,
    target_var: &str,
    is_optional: bool,
) -> Query {
    // Create variables directly with the passed names
    let source_variable = Value::Variable(source_var.to_string());
    let target_variable = Value::Variable(target_var.to_string());
    let constraint = terminusdb_woql2::and!(
        terminusdb_woql2::triple!(source_variable.clone(), field_name, target_variable.clone())
            .into(),
        terminusdb_woql2::type_!(source_variable.clone(), source_type).into(),
        terminusdb_woql2::type_!(target_variable, target_type).into()
    );

    if is_optional {
        terminusdb_woql2::optional!(constraint)
    } else {
        constraint
    }
}

/// Blanket implementations for container types
/// These allow the derive macro to generate RelationTo for container field types
/// and let the trait resolver find the appropriate implementation at usage time.

// Option<T> wraps the relation in WoqlOptional
impl<T, Target, Field> RelationTo<Target, Field> for Option<T>
where
    T: RelationTo<Target, Field> + TerminusDBModel,
    Target: TerminusDBModel,
    Field: RelationField,
{
    fn _constraints_with_vars_unchecked(source_var: &str, target_var: &str) -> Query {
        terminusdb_woql2::optional!(T::_constraints_with_vars_unchecked(source_var, target_var))
    }
}

// Vec<T> uses the same constraints as T - the triple store naturally handles multiple values
impl<T, Target, Field> RelationTo<Target, Field> for Vec<T>
where
    T: RelationTo<Target, Field> + TerminusDBModel,
    Target: TerminusDBModel,
    Field: RelationField,
{
    fn _constraints_with_vars_unchecked(source_var: &str, target_var: &str) -> Query {
        T::_constraints_with_vars_unchecked(source_var, target_var)
    }
}

// Box<T> just delegates to T
impl<T, Target, Field> RelationTo<Target, Field> for Box<T>
where
    T: RelationTo<Target, Field> + TerminusDBModel,
    Target: TerminusDBModel,
    Field: RelationField,
{
    fn _constraints_with_vars_unchecked(source_var: &str, target_var: &str) -> Query {
        T::_constraints_with_vars_unchecked(source_var, target_var)
    }
}

// ============================================================================
// ORM Relation Traits
// ============================================================================

/// Trait for models that belong to (reference) entity T via a foreign key.
///
/// This is the "child" side of a relation - the side with the `EntityIDFor<T>` field.
/// When `Child` has a field like `parent_id: EntityIDFor<Parent>`, this trait marks that relationship.
///
/// # Implementation
/// The derive macro automatically implements this for each `EntityIDFor<T>` field:
/// ```ignore
/// impl BelongsTo<Parent, ChildFields::ParentId> for Child {
///     fn parent_id(&self) -> Option<&EntityIDFor<Parent>> {
///         Some(&self.parent_id)
///     }
/// }
/// ```
pub trait BelongsTo<T: TerminusDBModel, Field: RelationField = DefaultField> {
    /// Get the field name for this relation.
    fn field_name() -> &'static str {
        Field::field_name()
    }

    /// Get the parent entity's ID from this model.
    fn parent_id(&self) -> Option<&EntityIDFor<T>>;
}

/// Forward relation: Self has a field pointing to Target (HasMany or HasOne).
///
/// Use with `.with_field::<Target, Self::Fields::FieldName>()` in ORM queries.
///
/// # Implementation
/// The derive macro automatically implements this for each `EntityIDFor<T>` field:
/// ```ignore
/// impl ForwardRelation<Wheel, CarFields::FrontWheel> for Car {}
/// ```
pub trait ForwardRelation<Target: TerminusDBModel, Field: RelationField> {
    /// Get the field name for this relation.
    fn field_name() -> &'static str {
        Field::field_name()
    }
}

/// Reverse relation: Self has a BelongsTo<Target> field (references Target).
///
/// Use with `.with::<Self>()` or `.with_via::<Self, Self::Fields::FieldName>()` in ORM queries.
///
/// - `.with::<Reply>()` on Comment query → loads all Replies with any BelongsTo<Comment>
/// - `.with_via::<Doc, DocFields::Author>()` → loads only via specific field
///
/// # Implementation
/// The derive macro generates two types of impls:
/// - `ReverseRelation<T, SpecificField>` for each TdbLazy<T> field (enables `.with_via()`)
/// - `ReverseRelation<T, DefaultField>` for any type with at least one TdbLazy<T> (enables `.with()`)
///
/// The `DefaultField` version acts as a marker saying "this type has at least one reference to T".
/// When there's exactly ONE TdbLazy<T> field, `default_field_name()` returns that field's name
/// so `.with()` can automatically use the correct field.
pub trait ReverseRelation<Target: TerminusDBModel, Field: RelationField = DefaultField> {
    /// Get the field name for this relation (or "default" for any-field queries).
    fn field_name() -> &'static str {
        Field::field_name()
    }

    /// Get the default field name for DefaultField impls (used by `.with()`).
    ///
    /// Returns `Some(field_name)` when there's exactly one TdbLazy<Target> field,
    /// allowing `.with()` to automatically use the correct field name in GraphQL queries.
    /// Returns `None` when there are multiple fields (ambiguous) or for specific field impls.
    fn default_field_name() -> Option<&'static str> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_relation_constraints() {
        // Test basic relation constraint generation
        let query = generate_relation_constraints("posts", "User", "Post", "u", "p", false);
        println!("Generated query: {:?}", query);

        // Test optional relation
        let optional_query =
            generate_relation_constraints("manager", "User", "User", "u1", "u2", true);
        println!("Generated optional query: {:?}", optional_query);
    }

    #[test]
    fn test_default_field() {
        assert_eq!(DefaultField::field_name(), "default");
    }
}
