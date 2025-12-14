#[cfg(test)]
mod tests {
    use terminusdb_client::debug::{OperationEntry, OperationType, OperationLog};
    use terminusdb_woql2::prelude::Query;
    use terminusdb_woql_builder::prelude::*;

    #[test]
    fn test_operation_log_get_last_query() {
        let log = OperationLog::new(10);
        
        // Initially no query
        assert!(log.get_last_query().is_none());
        
        // Add a non-query operation
        let insert_op = OperationEntry::new(
            OperationType::Insert,
            "/api/db/test/document".to_string()
        );
        log.push(insert_op);
        
        // Still no query
        assert!(log.get_last_query().is_none());
        
        // Add a query operation
        let query1 = WoqlBuilder::new()
            .select(vec![vars!("X")])
            .triple("v:X", "rdf:type", "owl:Class")
            .finalize();
            
        let query_op1 = OperationEntry::new(
            OperationType::Query,
            "/api/woql/test".to_string()
        ).with_query(query1.clone());
        log.push(query_op1);
        
        // Now we should get the query
        let last_query = log.get_last_query();
        assert!(last_query.is_some());
        
        // Add another non-query operation
        let delete_op = OperationEntry::new(
            OperationType::Delete,
            "/api/db/test/document/123".to_string()
        );
        log.push(delete_op);
        
        // Should still get the same query
        let last_query2 = log.get_last_query();
        assert!(last_query2.is_some());
        
        // Add a second query
        let query2 = WoqlBuilder::new()
            .select(vec![vars!("Y")])
            .triple("v:Y", "name", "v:Name")
            .finalize();
            
        let query_op2 = OperationEntry::new(
            OperationType::Query,
            "/api/woql/test".to_string()
        ).with_query(query2.clone());
        log.push(query_op2);
        
        // Should now get the second query
        let last_query3 = log.get_last_query();
        assert!(last_query3.is_some());
    }
}