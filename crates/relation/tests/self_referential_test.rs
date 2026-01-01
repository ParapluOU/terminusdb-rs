//! Test that EntityIDFor<Self> fields don't cause conflicting relation impls
//!
//! This test verifies the fix for the issue where a model with `id: EntityIDFor<Self>`
//! would generate self-referential relation traits that conflict.

use terminusdb_schema_derive::TerminusDBModel;
use terminusdb_schema::{EntityIDFor, TdbLazy, ToTDBInstance};
use terminusdb_relation::BelongsTo;
use serde::{Serialize, Deserialize};

// Required for TerminusDBModel derive to work
use terminusdb_schema as terminusdb_schema;

/// Model with self-referential EntityIDFor - this used to cause conflicting impls
#[derive(TerminusDBModel, Debug, Clone)]
#[tdb(key = "random", id_field = "id")]
struct SelfRefModel {
    id: EntityIDFor<Self>,
    name: String,
}

/// Model with optional self-referential EntityIDFor
#[derive(TerminusDBModel, Debug, Clone)]
#[tdb(key = "random")]
struct OptionalSelfRef {
    parent_id: Option<EntityIDFor<Self>>,
    name: String,
}

/// Model that references another model via EntityIDFor (not TdbLazy)
#[derive(TerminusDBModel, Debug, Clone, Default)]
#[tdb(key = "random")]
struct Parent {
    name: String,
}

#[derive(TerminusDBModel, Debug, Clone)]
#[tdb(key = "random")]
struct ChildWithEntityId {
    parent_id: EntityIDFor<Parent>,
    name: String,
}

/// Model with TdbLazy self-reference (this SHOULD generate relations)
#[derive(TerminusDBModel, Debug, Clone)]
#[tdb(key = "random")]
struct TreeNode {
    name: String,
    parent: Option<TdbLazy<Self>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_self_ref_entity_id_compiles() {
        // This test passing means EntityIDFor<Self> doesn't cause conflicting impls
        let model = SelfRefModel {
            id: EntityIDFor::new("test-id").unwrap(),
            name: "Test".to_string(),
        };

        // BelongsTo should be generated for EntityIDFor<Self>
        let parent_id: Option<&EntityIDFor<SelfRefModel>> =
            <SelfRefModel as BelongsTo<SelfRefModel, SelfRefModelFields::Id>>::parent_id(&model);

        assert!(parent_id.is_some());
        // EntityIDFor includes class prefix
        assert!(parent_id.unwrap().to_string().contains("test-id"));

        println!("EntityIDFor<Self> works without conflicting impls");
    }

    #[test]
    fn test_optional_self_ref_entity_id() {
        let model_with = OptionalSelfRef {
            parent_id: Some(EntityIDFor::new("parent-123").unwrap()),
            name: "Child".to_string(),
        };

        let model_without = OptionalSelfRef {
            parent_id: None,
            name: "Root".to_string(),
        };

        // BelongsTo with Option<EntityIDFor<Self>>
        let parent_id: Option<&EntityIDFor<OptionalSelfRef>> =
            <OptionalSelfRef as BelongsTo<OptionalSelfRef, OptionalSelfRefFields::ParentId>>::parent_id(&model_with);
        assert!(parent_id.is_some());

        let no_parent: Option<&EntityIDFor<OptionalSelfRef>> =
            <OptionalSelfRef as BelongsTo<OptionalSelfRef, OptionalSelfRefFields::ParentId>>::parent_id(&model_without);
        assert!(no_parent.is_none());

        println!("Option<EntityIDFor<Self>> works correctly");
    }

    #[test]
    fn test_entity_id_cross_reference() {
        let child = ChildWithEntityId {
            parent_id: EntityIDFor::new("parent-1").unwrap(),
            name: "Child".to_string(),
        };

        // BelongsTo should work for EntityIDFor<OtherModel>
        let parent_id: Option<&EntityIDFor<Parent>> =
            <ChildWithEntityId as BelongsTo<Parent, ChildWithEntityIdFields::ParentId>>::parent_id(&child);

        assert!(parent_id.is_some());
        // EntityIDFor includes class prefix
        assert!(parent_id.unwrap().to_string().contains("parent-1"));

        println!("EntityIDFor<OtherModel> generates BelongsTo correctly");
    }

    #[test]
    fn test_tdblazy_self_ref_generates_relations() {
        use terminusdb_relation::{ForwardRelation, ReverseRelation, DefaultField};

        // TdbLazy<Self> SHOULD generate ForwardRelation and ReverseRelation
        // This is a compile-time check - if it compiles, the traits are generated
        fn assert_forward<T: ForwardRelation<TreeNode, TreeNodeFields::Parent>>() {}
        fn assert_reverse<T: ReverseRelation<TreeNode, TreeNodeFields::Parent>>() {}
        fn assert_default_reverse<T: ReverseRelation<TreeNode, DefaultField>>() {}

        assert_forward::<TreeNode>();
        assert_reverse::<TreeNode>();
        assert_default_reverse::<TreeNode>();

        println!("TdbLazy<Self> generates Forward/Reverse relations correctly");
    }

    #[test]
    fn test_entity_id_no_forward_reverse_relations() {
        use terminusdb_relation::{ForwardRelation, ReverseRelation};

        // EntityIDFor<T> should NOT generate ForwardRelation or ReverseRelation
        // We can't easily test "trait not implemented" at runtime, but we can
        // verify the correct traits ARE implemented

        fn assert_belongs_to<T: BelongsTo<SelfRefModel, SelfRefModelFields::Id>>() {}
        assert_belongs_to::<SelfRefModel>();

        // If we uncomment the following, it should NOT compile:
        // fn assert_forward<T: ForwardRelation<SelfRefModel, SelfRefModelFields::Id>>() {}
        // assert_forward::<SelfRefModel>(); // ERROR: trait not implemented

        println!("EntityIDFor only generates BelongsTo, not Forward/Reverse relations");
    }
}
