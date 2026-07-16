#![recursion_limit = "256"]
//! Live verification of TerminusDB 12 numeric precision (rational arithmetic,
//! native JSON numbers) and the Allen interval-algebra ops against a 12.1
//! server. Informs the M1 numeric overhaul: shows exactly what precision the
//! server returns and how the current client parses it.

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use chrono::NaiveDate;
    use serde_json::Value;
    use terminusdb_bin::TerminusDBServer;
    use terminusdb_client::*;
    use terminusdb_schema::XSDAnySimpleType;
    use terminusdb_woql2::expression::{ArithmeticExpression, ArithmeticValue, Divide};
    use terminusdb_woql2::interval::{Interval, IntervalRelation};
    use terminusdb_woql2::prelude::{DataValue, Query};
    use terminusdb_woql2::query::Eval;

    fn int_expr(n: i64) -> ArithmeticExpression {
        ArithmeticExpression::Value(ArithmeticValue::Data(XSDAnySimpleType::Integer(n)))
    }
    fn date(s: &str) -> DataValue {
        DataValue::Data(XSDAnySimpleType::Date(
            NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap(),
        ))
    }
    fn var(name: &str) -> DataValue {
        DataValue::Variable(name.to_string())
    }

    async fn run(
        client: &TerminusDBHttpClient,
        spec: &BranchSpec,
        query: Query,
    ) -> anyhow::Result<Vec<HashMap<String, Value>>> {
        let res: WOQLResult<HashMap<String, Value>> =
            client.query(Some(spec.clone()), query).await?;
        Ok(res.bindings)
    }

    /// v12 does exact rational arithmetic: 1/3 should return a high-precision
    /// decimal (~20 significant digits), NOT a truncated float. This probe
    /// records exactly what comes back so M1 can decide the decimal type.
    #[tokio::test]
    async fn test_v12_rational_division_precision() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;
        server
            .with_tmp_db("v12_precision", |client, spec| async move {
                let q = Query::Eval(Eval {
                    expression: ArithmeticExpression::Divide(Divide {
                        left: Box::new(int_expr(1)),
                        right: Box::new(int_expr(3)),
                    }),
                    result_value: ArithmeticValue::Variable("R".to_string()),
                });
                let b = run(&client, &spec, q).await?;
                assert!(!b.is_empty(), "Eval(1/3) returned no bindings");
                let r = b[0].get("R").expect("no R binding");
                println!("v12 Eval(1/3) = {}", serde_json::to_string(r)?);

                // Record the @type and how many significant digits survived.
                if let Some(val) = r.get("@value") {
                    let s = val.to_string();
                    let digits = s.chars().filter(|c| c.is_ascii_digit()).count();
                    println!(
                        "  @type={:?} value={} ({} digits)",
                        r.get("@type"),
                        s,
                        digits
                    );
                }
                Ok(())
            })
            .await
    }

    /// Allen's interval algebra: classify the relation between two half-open
    /// date intervals. x=[Jan,Jun) overlaps y=[Mar,Sep).
    #[tokio::test]
    async fn test_v12_interval_relation() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;
        server
            .with_tmp_db("v12_intervals", |client, spec| async move {
                // Classify (relation is a variable -> determine which Allen relation holds)
                let q = Query::IntervalRelation(IntervalRelation {
                    relation: var("Rel"),
                    x_start: date("2020-01-01"),
                    x_end: date("2020-06-01"),
                    y_start: date("2020-03-01"),
                    y_end: date("2020-09-01"),
                });
                let b = run(&client, &spec, q).await?;
                println!(
                    "IntervalRelation(x=[Jan,Jun), y=[Mar,Sep)) -> {:?}",
                    b.first().and_then(|m| m.get("Rel"))
                );

                // Construct an interval value from two endpoints.
                let q = Query::Interval(Interval {
                    start: date("2020-01-01"),
                    end: date("2020-06-01"),
                    interval: var("Iv"),
                });
                let b = run(&client, &spec, q).await?;
                println!(
                    "Interval([Jan,Jun)) -> {:?}",
                    b.first().and_then(|m| m.get("Iv"))
                );
                Ok(())
            })
            .await
    }
}
