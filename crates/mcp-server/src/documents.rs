//! Document mutation handlers: insert / replace documents and delete classes.

use crate::handler::TerminusDBMcpHandler;
use crate::tools::*;
use anyhow::Result;
use serde_json::json;
use terminusdb_client::{BranchSpec, DocumentInsertArgs};
use tracing::{error, info};

impl TerminusDBMcpHandler {
    pub(crate) async fn handle_insert_document(
        &self,
        request: InsertDocumentTool,
    ) -> Result<serde_json::Value> {
        // Validate document has required fields
        if !request.document.is_object() {
            return Err(anyhow::anyhow!("Document must be a JSON object"));
        }

        let doc_obj = request.document.as_object().unwrap();
        if !doc_obj.contains_key("@id") || !doc_obj.contains_key("@type") {
            return Err(anyhow::anyhow!(
                "Document must contain @id and @type fields"
            ));
        }

        let config = self.get_connection_config(request.connection).await;
        let client = Self::create_client(&config).await?;

        // Set database in config
        let mut config = config;
        config.database = Some(request.database.clone());

        // Create branch spec
        let branch_spec = BranchSpec::with_branch(
            &request.database,
            &request.branch.unwrap_or_else(|| "main".to_string()),
        );

        // Create insert args
        let args = DocumentInsertArgs {
            spec: branch_spec,
            message: request
                .message
                .unwrap_or_else(|| "insert document".to_string()),
            author: request.author.unwrap_or_else(|| "system".to_string()),
            force: request.force,
            ..Default::default()
        };

        // Insert the document
        let doc_vec = vec![&request.document];
        let result = client.insert_documents(doc_vec, args).await?;

        // Extract results
        let mut response = serde_json::json!({
            "status": "success",
            "database": request.database,
            "results": {}
        });

        if let Some(results_map) = response.get_mut("results").and_then(|v| v.as_object_mut()) {
            for (id, insert_result) in result.iter() {
                let status = match insert_result {
                    terminusdb_client::TDBInsertInstanceResult::Inserted(_) => "inserted",
                    terminusdb_client::TDBInsertInstanceResult::AlreadyExists(_) => {
                        "already_exists"
                    }
                };
                results_map.insert(
                    id.clone(),
                    serde_json::json!({
                        "id": id,
                        "status": status
                    }),
                );
            }
        }

        // Add commit ID if available
        if let Some(commit_id) = result.extract_commit_id() {
            response["commit_id"] = serde_json::Value::String(commit_id.to_string());
        }

        Ok(response)
    }

    pub(crate) async fn handle_insert_documents(
        &self,
        request: InsertDocumentsTool,
    ) -> Result<serde_json::Value> {
        // Validate all documents have required fields
        for (idx, doc) in request.documents.iter().enumerate() {
            if !doc.is_object() {
                return Err(anyhow::anyhow!(
                    "Document at index {} must be a JSON object",
                    idx
                ));
            }

            let doc_obj = doc.as_object().unwrap();
            if !doc_obj.contains_key("@id") || !doc_obj.contains_key("@type") {
                return Err(anyhow::anyhow!(
                    "Document at index {} must contain @id and @type fields",
                    idx
                ));
            }
        }

        let config = self.get_connection_config(request.connection).await;
        let client = Self::create_client(&config).await?;

        // Set database in config
        let mut config = config;
        config.database = Some(request.database.clone());

        // Create branch spec
        let branch_spec = BranchSpec::with_branch(
            &request.database,
            &request.branch.unwrap_or_else(|| "main".to_string()),
        );

        // Create insert args
        let args = DocumentInsertArgs {
            spec: branch_spec,
            message: request
                .message
                .unwrap_or_else(|| "insert documents".to_string()),
            author: request.author.unwrap_or_else(|| "system".to_string()),
            force: request.force,
            ..Default::default()
        };

        // Convert documents to references
        let doc_refs: Vec<&serde_json::Value> = request.documents.iter().collect();

        // Insert the documents
        let result = client.insert_documents(doc_refs, args).await?;

        // Build response
        let mut response = serde_json::json!({
            "status": "success",
            "database": request.database,
            "total_documents": request.documents.len(),
            "results": {}
        });

        let mut inserted_count = 0;
        let mut already_exists_count = 0;

        if let Some(results_map) = response.get_mut("results").and_then(|v| v.as_object_mut()) {
            for (id, insert_result) in result.iter() {
                let status = match insert_result {
                    terminusdb_client::TDBInsertInstanceResult::Inserted(_) => {
                        inserted_count += 1;
                        "inserted"
                    }
                    terminusdb_client::TDBInsertInstanceResult::AlreadyExists(_) => {
                        already_exists_count += 1;
                        "already_exists"
                    }
                };
                results_map.insert(
                    id.clone(),
                    serde_json::json!({
                        "id": id,
                        "status": status
                    }),
                );
            }
        }

        response["summary"] = serde_json::json!({
            "inserted": inserted_count,
            "already_exists": already_exists_count
        });

        // Add commit ID if available
        if let Some(commit_id) = result.extract_commit_id() {
            response["commit_id"] = serde_json::Value::String(commit_id.to_string());
        }

        Ok(response)
    }

    pub(crate) async fn handle_replace_document(
        &self,
        request: ReplaceDocumentTool,
    ) -> Result<serde_json::Value> {
        // Validate document has required fields
        if !request.document.is_object() {
            return Err(anyhow::anyhow!("Document must be a JSON object"));
        }

        let doc_obj = request.document.as_object().unwrap();
        if !doc_obj.contains_key("@id") || !doc_obj.contains_key("@type") {
            return Err(anyhow::anyhow!(
                "Document must contain @id and @type fields"
            ));
        }

        let config = self.get_connection_config(request.connection).await;
        let client = Self::create_client(&config).await?;

        // Set database in config
        let mut config = config;
        config.database = Some(request.database.clone());

        // Create branch spec
        let branch_spec = BranchSpec::with_branch(
            &request.database,
            &request.branch.unwrap_or_else(|| "main".to_string()),
        );

        // Create insert args
        let args = DocumentInsertArgs {
            spec: branch_spec,
            message: request
                .message
                .unwrap_or_else(|| "replace document".to_string()),
            author: request.author.unwrap_or_else(|| "system".to_string()),
            force: false, // Never force for replace operation
            ..Default::default()
        };

        // Use put_documents which requires document to exist
        let doc_vec = vec![&request.document];
        let result = client.put_documents(doc_vec, args).await?;

        // Extract results
        let mut response = serde_json::json!({
            "status": "success",
            "database": request.database,
            "operation": "replace",
            "results": {}
        });

        if let Some(results_map) = response.get_mut("results").and_then(|v| v.as_object_mut()) {
            for (id, _) in result.iter() {
                results_map.insert(
                    id.clone(),
                    serde_json::json!({
                        "id": id,
                        "status": "replaced"
                    }),
                );
            }
        }

        // Add commit ID if available
        if let Some(commit_id) = result.extract_commit_id() {
            response["commit_id"] = serde_json::Value::String(commit_id.to_string());
        }

        Ok(response)
    }

    pub(crate) async fn handle_delete_classes(&self, request: DeleteClassesTool) -> Result<serde_json::Value> {
        info!("Deleting classes: {:?}", request.class_names);

        let config = self.get_connection_config(request.connection).await;
        let client = Self::create_client(&config).await?;

        // Create branch spec
        let branch_name = request.branch.as_deref().unwrap_or("main");
        let branch_spec = BranchSpec::with_branch(&request.database, branch_name);

        // For each class, we need to delete all its schema triples
        // This includes the class definition itself and all its properties
        let mut deleted_classes = Vec::new();
        let mut errors = Vec::new();

        for class_name in &request.class_names {
            // Build a WOQL query to delete all triples where the subject is the class
            // or where the domain is the class (for properties)
            let class_uri = format!("@schema:{}", class_name);

            // Create a JSON-LD query to delete all schema triples for this class
            let delete_query = json!({
                "@type": "And",
                "and": [
                    // Delete the class definition itself
                    {
                        "@type": "DeleteTriple",
                        "subject": {
                            "@type": "NodeValue",
                            "node": &class_uri
                        },
                        "predicate": {
                            "@type": "NodeValue",
                            "variable": "Predicate1"
                        },
                        "object": {
                            "@type": "Value",
                            "variable": "Object1"
                        },
                        "graph": "schema"
                    },
                    // Delete all properties that have this class as domain
                    {
                        "@type": "DeleteTriple",
                        "subject": {
                            "@type": "NodeValue",
                            "variable": "Property"
                        },
                        "predicate": {
                            "@type": "NodeValue",
                            "node": "rdfs:domain"
                        },
                        "object": {
                            "@type": "Value",
                            "node": &class_uri
                        },
                        "graph": "schema"
                    },
                    // Delete all other triples related to those properties
                    {
                        "@type": "DeleteTriple",
                        "subject": {
                            "@type": "NodeValue",
                            "variable": "Property"
                        },
                        "predicate": {
                            "@type": "NodeValue",
                            "variable": "Predicate2"
                        },
                        "object": {
                            "@type": "Value",
                            "variable": "Object2"
                        },
                        "graph": "schema"
                    }
                ]
            });

            // First, find all the triples to delete
            let _find_query = json!({
                "@type": "And",
                "and": [
                    // Find class triples
                    {
                        "@type": "Triple",
                        "subject": {
                            "@type": "NodeValue",
                            "node": &class_uri
                        },
                        "predicate": {
                            "@type": "NodeValue",
                            "variable": "Predicate1"
                        },
                        "object": {
                            "@type": "Value",
                            "variable": "Object1"
                        },
                        "graph": "schema"
                    },
                    // Find properties with this class as domain
                    {
                        "@type": "Triple",
                        "subject": {
                            "@type": "NodeValue",
                            "variable": "Property"
                        },
                        "predicate": {
                            "@type": "NodeValue",
                            "node": "rdfs:domain"
                        },
                        "object": {
                            "@type": "Value",
                            "node": &class_uri
                        },
                        "graph": "schema"
                    },
                    // Find all triples for those properties
                    {
                        "@type": "Triple",
                        "subject": {
                            "@type": "NodeValue",
                            "variable": "Property"
                        },
                        "predicate": {
                            "@type": "NodeValue",
                            "variable": "Predicate2"
                        },
                        "object": {
                            "@type": "Value",
                            "variable": "Object2"
                        },
                        "graph": "schema"
                    }
                ]
            });

            // Execute the deletion query
            match client
                .query_string::<serde_json::Value>(
                    Some(branch_spec.clone()),
                    &serde_json::to_string(&delete_query)?,
                    None,
                )
                .await
            {
                Ok(_) => {
                    deleted_classes.push(class_name.clone());
                    info!("Successfully deleted class: {}", class_name);
                }
                Err(e) => {
                    let error_msg = format!("Failed to delete class {}: {}", class_name, e);
                    error!("{}", error_msg);
                    errors.push(error_msg);
                }
            }
        }

        let response = if errors.is_empty() {
            serde_json::json!({
                "status": "success",
                "message": format!("Successfully deleted {} classes", deleted_classes.len()),
                "deleted_classes": deleted_classes,
                "database": request.database,
                "branch": request.branch.unwrap_or_else(|| "main".to_string())
            })
        } else {
            serde_json::json!({
                "status": "partial_success",
                "message": format!("Deleted {} classes, {} failed", deleted_classes.len(), errors.len()),
                "deleted_classes": deleted_classes,
                "errors": errors,
                "database": request.database,
                "branch": branch_name
            })
        };

        Ok(response)
    }
}
