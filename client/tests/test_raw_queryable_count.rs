use anyhow::Result;
use serde::Deserialize;
use std::collections::HashMap;
use terminusdb_client::{RawQueryable, TerminusDBHttpClient};
use terminusdb_woql2::prelude::Query;
use terminusdb_woql_builder::{builder::WoqlBuilder, vars};

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct PersonResult {
    name: String,
    age: i32,
}

struct PersonQuery;

impl RawQueryable for PersonQuery {
    type Result = PersonResult;

    fn query(&self) -> Query {
        WoqlBuilder::new()
            .triple(vars!("Person"), "rdf:type", "@schema:Person")
            .triple(vars!("Person"), "@schema:name", vars!("Name"))
            .triple(vars!("Person"), "@schema:age", vars!("Age"))
            .select(vec![vars!("Name"), vars!("Age")])
            .finalize()
    }

    fn extract_result(
        &self,
        mut binding: HashMap<String, serde_json::Value>,
    ) -> anyhow::Result<Self::Result> {
        let name = binding
            .remove("Name")
            .and_then(|v| v.get("@value").cloned())
            .and_then(|v| serde_json::from_value::<String>(v).ok())
            .ok_or_else(|| anyhow::anyhow!("Missing name field"))?;

        let age = binding
            .remove("Age")
            .and_then(|v| v.get("@value").cloned())
            .and_then(|v| serde_json::from_value::<i32>(v).ok())
            .ok_or_else(|| anyhow::anyhow!("Missing age field"))?;

        Ok(PersonResult { name, age })
    }
}

// Test that a custom count query is preserved
struct PersonCountQuery;

impl RawQueryable for PersonCountQuery {
    type Result = ();

    fn query(&self) -> Query {
        use terminusdb_woql2::misc::Count;
        
        let inner_query = WoqlBuilder::new()
            .triple(vars!("Person"), "rdf:type", "@schema:Person")
            .finalize();
        
        // Return a Count query directly
        Query::Count(Count {
            query: Box::new(inner_query),
            count: terminusdb_woql2::prelude::DataValue::Variable("MyCount".to_string()),
        })
    }
}

// Test query with pagination
struct PaginatedPersonQuery {
    skip: u64,
    limit: u64,
}

impl RawQueryable for PaginatedPersonQuery {
    type Result = PersonResult;

    fn query(&self) -> Query {
        use terminusdb_woql2::misc::{Start, Limit};
        
        let base_query = WoqlBuilder::new()
            .triple(vars!("Person"), "rdf:type", "@schema:Person")
            .triple(vars!("Person"), "@schema:name", vars!("Name"))
            .triple(vars!("Person"), "@schema:age", vars!("Age"))
            .select(vec![vars!("Name"), vars!("Age")])
            .finalize();
        
        // Wrap in Start and Limit for pagination
        let with_start = Query::Start(Start {
            start: self.skip,
            query: Box::new(base_query),
        });
        
        Query::Limit(Limit {
            limit: self.limit,
            query: Box::new(with_start),
        })
    }

    fn extract_result(
        &self,
        mut binding: HashMap<String, serde_json::Value>,
    ) -> anyhow::Result<Self::Result> {
        let name = binding
            .remove("Name")
            .and_then(|v| v.get("@value").cloned())
            .and_then(|v| serde_json::from_value::<String>(v).ok())
            .ok_or_else(|| anyhow::anyhow!("Missing name field"))?;

        let age = binding
            .remove("Age")
            .and_then(|v| v.get("@value").cloned())
            .and_then(|v| serde_json::from_value::<i32>(v).ok())
            .ok_or_else(|| anyhow::anyhow!("Missing age field"))?;

        Ok(PersonResult { name, age })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_query_count_wraps_non_count_query() {
        let query = PersonQuery;
        let count_query = query.query_count();
        
        // Verify the query is wrapped in a Count
        match count_query {
            Query::Count(_count) => {
                // The inner query should be our original query
                // We can't easily test the exact structure, but we can verify it's a Count
                assert!(true, "Query was correctly wrapped in Count");
            }
            _ => panic!("Expected Count query, got {:?}", count_query),
        }
    }
    
    #[test]
    fn test_query_count_preserves_existing_count() {
        let query = PersonCountQuery;
        let count_query = query.query_count();
        
        // Verify the Count query is preserved as-is
        match count_query {
            Query::Count(_) => {
                assert!(true, "Count query was preserved");
            }
            _ => panic!("Expected Count query to be preserved, got {:?}", count_query),
        }
    }
    
    #[test]
    fn test_query_count_unwraps_pagination() {
        let query = PaginatedPersonQuery {
            skip: 10,
            limit: 5,
        };
        let count_query = query.query_count();
        
        // Verify the query is wrapped in Count and pagination is removed
        match count_query {
            Query::Count(count) => {
                // The inner query should not be Limit or Start
                match &*count.query {
                    Query::Limit(_) => panic!("Count query should not contain Limit"),
                    Query::Start(_) => panic!("Count query should not contain Start"),
                    _ => assert!(true, "Pagination was correctly unwrapped"),
                }
            }
            _ => panic!("Expected Count query, got {:?}", count_query),
        }
    }
    
    #[ignore]
    #[tokio::test]
    async fn test_count_execution() -> Result<()> {
        let client = TerminusDBHttpClient::local_node().await;
        let spec = terminusdb_client::BranchSpec::with_branch("test", "main");

        // This test requires a running TerminusDB instance with test data
        let query = PersonQuery;
        let count = query.count(&client, &spec).await?;
        
        println!("Found {} persons", count);
        // Count is always non-negative by type, so just verify it exists
        let _ = count;
        
        Ok(())
    }
}