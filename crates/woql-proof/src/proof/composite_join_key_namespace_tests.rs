    use super::*;

    /// One `LayerTripleCount` leaf: a single `GeneratedDecimalU64` column (not a Store ID).
    fn generated_u64_leaf() -> CompiledRelation {
        CompiledRelation {
            schema: vec!["_".into()],
            domains: vec![None],
            statement: CompiledRelationStatement::LayerTripleCount,
        }
    }

    /// A two-column relation whose columns `a`/`b` are both `GeneratedDecimalU64` — i.e. NOT
    /// bounded Store dictionary IDs, so they must never reach the composite key packing.
    fn two_generated_columns(a: &str, b: &str) -> CompiledRelation {
        CompiledRelation {
            schema: vec![a.into(), b.into()],
            domains: vec![None, None],
            statement: CompiledRelationStatement::InnerJoin {
                left: Box::new(generated_u64_leaf()),
                right: Box::new(generated_u64_leaf()),
                key_pairs: Vec::new(),
            },
        }
    }

    #[test]
    fn composite_join_rejects_non_store_id_key_namespace() {
        let left = two_generated_columns("a", "b");
        let right = two_generated_columns("a", "b");
        // Both a Not (LeftAnti) and an Optional (LeftOuter) route composite keys here.
        for optional in [false, true] {
            let err = shared_key_pairs(&left, &right, optional).unwrap_err();
            assert!(
                matches!(err, BgpPlanError::CompositeJoinKeyNotStoreId { .. }),
                "expected CompositeJoinKeyNotStoreId, got {err:?}"
            );
        }
    }

    #[test]
    fn scalar_join_over_generated_key_stays_unrestricted() {
        // A single shared key uses injective equality, never the 2^64 packing, so a
        // generated-decimal scalar key must NOT be rejected by the composite guard.
        let left = generated_u64_leaf();
        let right = generated_u64_leaf();
        let pairs = shared_key_pairs(&left, &right, false).unwrap();
        assert_eq!(pairs, [(0, 0)]);
    }
