use terminusdb_client::{ApiResponseError, ErrorResponse, TypedErrorResponse, ApiResponse};
use serde_json::json;

fn main() {
    // Example of how to handle the InsertedSubdocumentAsDocument error
    
    // This is what TerminusDB returns when you try to insert a subdocument as a top-level document
    let error_json = json!({
        "@type": "api:InsertDocumentErrorResponse",
        "api:error": {
            "@type": "api:InsertedSubdocumentAsDocument",
            "api:document": {
                "@type": "Address",
                "street": "123 Main St",
                "city": "Springfield",
            },
        },
        "api:message": "Attempted to insert a subdocument as a top-level document",
        "api:status": "api:failure",
    });
    
    // First, check if it deserializes to an ApiResponse correctly
    match serde_json::from_value::<ApiResponse<serde_json::Value>>(error_json.clone()) {
        Ok(ApiResponse::Error(typed_err)) => {
            println!("Successfully detected as an error response");
            
            // Now check which specific error type it is
            match typed_err {
                TypedErrorResponse::InsertDocumentError { error: err_response, .. } => {
                    println!("Error message: {}", err_response.api_message);
                    
                    if let Some(ApiResponseError::InsertedSubdocumentAsDocument(subdoc_err)) = err_response.api_error {
                        println!("This is a subdocument insertion error!");
                        println!("The subdocument type was: {}", subdoc_err.document["@type"]);
                        println!("Full document: {:#?}", subdoc_err.document);
                        
                        // Handle the error appropriately
                        println!("\n💡 To fix this:");
                        println!("- Subdocuments should only be inserted as properties of parent documents");
                        println!("- Make sure the model is not marked with #[tdb(subdocument = true)]");
                        println!("- Or insert it as part of a parent document's field");
                    }
                }
                _ => {
                    println!("Different type of error response");
                }
            }
        }
        Ok(ApiResponse::Success(_)) => {
            println!("Unexpectedly got a success response");
        }
        Err(e) => {
            println!("Failed to deserialize: {}", e);
        }
    }
}