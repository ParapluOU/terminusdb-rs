#![recursion_limit = "256"]
//! Live end-to-end test of the `bigdecimal::BigDecimal` xsd:decimal field type
//! against a 12.1 server: insert a model with a high-precision decimal, read it
//! back, and assert the value round-trips exactly (no f64 truncation).

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use terminusdb_bin::TerminusDBServer;
    use terminusdb_client::*;
    use terminusdb_schema::{BigDecimal, EntityIDFor, ToTDBInstance};
    use terminusdb_schema_derive::*;

    #[derive(Debug, Clone, TerminusDBModel)]
    #[tdb(id_field = "id")]
    struct Priced {
        id: EntityIDFor<Self>,
        // 25 significant digits — far beyond f64's ~16.
        amount: BigDecimal,
    }

    #[tokio::test]
    async fn test_v12_high_precision_decimal_field_roundtrip() -> anyhow::Result<()> {
        let server = TerminusDBServer::test_instance().await?;
        server
            .with_db_schema::<(Priced,), _, _, _>("v12_decimal_field", |client, spec| async move {
                let precise = BigDecimal::from_str("1234567890.123456789012345").unwrap();
                let model = Priced {
                    id: EntityIDFor::new("p1").unwrap(),
                    amount: precise.clone(),
                };

                let args = DocumentInsertArgs::from(spec.clone());
                client.insert_instance(&model, args).await?;

                let doc = client
                    .get_document("Priced/p1", &spec, GetOpts::default())
                    .await?;
                // The server stores the exact decimal; with arbitrary_precision it
                // comes back losslessly. Parse the @value (native number or string).
                let amount = &doc["amount"];
                let got = match amount {
                    serde_json::Value::Object(o) => {
                        BigDecimal::from_str(&o["@value"].to_string().trim_matches('"'))
                    }
                    other => BigDecimal::from_str(other.to_string().trim_matches('"')),
                }
                .expect("amount should parse as a decimal");

                assert_eq!(
                    got, precise,
                    "high-precision decimal must round-trip exactly (got {got}, raw {amount:?})"
                );
                Ok(())
            })
            .await
    }
}
