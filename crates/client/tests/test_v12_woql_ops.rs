#![recursion_limit = "256"]
//! Live verification of the TerminusDB 12 WOQL operations against a running
//! 12.1 server. These construct the woql2 AST directly (the builder is
//! deprecated) and run them through the client, asserting on the bindings.
//!
//! Purpose: catch any AST -> JSON-LD serialization mismatch that the offline
//! schema_match conformance test cannot, and map which v12 ops actually work.

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use serde_json::Value;
    use terminusdb_bin::TerminusDBServer;
    use terminusdb_client::*;
    use terminusdb_schema::XSDAnySimpleType;
    use terminusdb_woql2::collection::{ListToSet, SetDifference, SetIntersection, SetUnion, Slice};
    use terminusdb_woql2::compare::{Gte, Lte};
    use terminusdb_woql2::misc::InRange;
    use terminusdb_woql2::prelude::{DataValue, Query};

    fn int(n: i64) -> DataValue {
        DataValue::Data(XSDAnySimpleType::Integer(n))
    }
    fn list(ns: &[i64]) -> DataValue {
        DataValue::List(ns.iter().copied().map(int).collect())
    }
    fn var(name: &str) -> DataValue {
        DataValue::Variable(name.to_string())
    }

    /// Run a query and return its bindings (list of var->value maps).
    async fn run(
        client: &TerminusDBHttpClient,
        spec: &BranchSpec,
        query: Query,
    ) -> anyhow::Result<Vec<HashMap<String, Value>>> {
        let res: WOQLResult<HashMap<String, Value>> =
            client.query(Some(spec.clone()), query).await?;
        Ok(res.bindings)
    }

    #[tokio::test]
    async fn test_v12_set_and_list_ops() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;
        server
            .with_tmp_db("v12_set_ops", |client, spec| async move {
                // SetUnion([1,2,3], [3,4]) -> {1,2,3,4}
                let b = run(
                    &client,
                    &spec,
                    Query::SetUnion(SetUnion {
                        list_a: list(&[1, 2, 3]),
                        list_b: list(&[3, 4]),
                        result: var("R"),
                    }),
                )
                .await?;
                assert!(!b.is_empty(), "SetUnion returned no bindings");
                println!("SetUnion -> {:?}", b[0].get("R"));

                // SetIntersection([1,2,3], [2,3,4]) -> {2,3}
                let b = run(
                    &client,
                    &spec,
                    Query::SetIntersection(SetIntersection {
                        list_a: list(&[1, 2, 3]),
                        list_b: list(&[2, 3, 4]),
                        result: var("R"),
                    }),
                )
                .await?;
                println!("SetIntersection -> {:?}", b.first().and_then(|m| m.get("R")));

                // SetDifference([1,2,3], [2]) -> {1,3}
                let b = run(
                    &client,
                    &spec,
                    Query::SetDifference(SetDifference {
                        list_a: list(&[1, 2, 3]),
                        list_b: list(&[2]),
                        result: var("R"),
                    }),
                )
                .await?;
                println!("SetDifference -> {:?}", b.first().and_then(|m| m.get("R")));

                // ListToSet([1,1,2,3,3]) -> {1,2,3}
                let b = run(
                    &client,
                    &spec,
                    Query::ListToSet(ListToSet {
                        list: list(&[1, 1, 2, 3, 3]),
                        set: var("R"),
                    }),
                )
                .await?;
                println!("ListToSet -> {:?}", b.first().and_then(|m| m.get("R")));

                // Slice([10,20,30,40], 1, 3) -> [20,30]
                let b = run(
                    &client,
                    &spec,
                    Query::Slice(Slice {
                        list: list(&[10, 20, 30, 40]),
                        start: int(1),
                        end: Some(int(3)),
                        result: var("R"),
                    }),
                )
                .await?;
                assert!(!b.is_empty(), "Slice returned no bindings");
                println!("Slice(1,3) -> {:?}", b[0].get("R"));

                Ok(())
            })
            .await
    }

    #[tokio::test]
    async fn test_v12_comparison_and_range_ops() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;
        server
            .with_tmp_db("v12_cmp_ops", |client, spec| async move {
                // Gte(5, 3) should succeed (>=)
                let b = run(
                    &client,
                    &spec,
                    Query::Gte(Gte {
                        left: int(5),
                        right: int(3),
                    }),
                )
                .await?;
                assert!(!b.is_empty(), "Gte(5,3) should hold");

                // Lte(3, 3) should succeed (<=)
                let b = run(
                    &client,
                    &spec,
                    Query::Lte(Lte {
                        left: int(3),
                        right: int(3),
                    }),
                )
                .await?;
                assert!(!b.is_empty(), "Lte(3,3) should hold");

                // Gte(2, 9) should fail (no bindings)
                let b = run(
                    &client,
                    &spec,
                    Query::Gte(Gte {
                        left: int(2),
                        right: int(9),
                    }),
                )
                .await?;
                assert!(b.is_empty(), "Gte(2,9) should not hold");

                // InRange: is 3 in [1,5)?  value bound -> membership test
                let b = run(
                    &client,
                    &spec,
                    Query::InRange(InRange {
                        value: int(3),
                        start: int(1),
                        end: int(5),
                    }),
                )
                .await?;
                println!("InRange(3 in [1,5)) -> {} binding(s)", b.len());

                Ok(())
            })
            .await
    }
}
