use crate::TerminusAPIStatus;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::error::Error;
use std::fmt::Debug;
use std::fmt::{Display, Formatter, Write};

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(tag = "@type")]
pub enum ApiResponseError {
    #[serde(rename = "api:UnknownDatabase")]
    UnknownDatabase(UnknownDatabaseError),

    #[serde(rename = "api:DocumentNotFound")]
    DocumentNotFound(DocumentNotFoundError),

    #[serde(rename = "api:SchemaCheckFailure")]
    SchemaCheckFail(SchemaCheckFailError),

    #[serde(rename = "api:NotAllCapturesFound")]
    NotAllCapturesFound(NotAllCapturesFoundError),

    #[serde(rename = "vio:WOQLSyntaxError")]
    WOQLSyntaxError(WOQLSyntaxError),

    #[serde(rename = "api:DocumentIdAlreadyExists")]
    DocumentIdAlreadyExists(DocumentIdAlreadyExistsError),

    #[serde(rename = "api:UnresolvableAbsoluteDescriptor")]
    UnresolvableAbsoluteDescriptor(UnresolvableAbsoluteDescriptorError),

    #[serde(rename = "api:WOQLSyntaxError")]
    ApiWOQLSyntaxError(ApiWOQLSyntaxError),

    #[serde(rename = "api:InsertedSubdocumentAsDocument")]
    InsertedSubdocumentAsDocument(InsertedSubdocumentAsDocumentError),

    #[serde(rename = "api:InternalServerError")]
    InternalServerError(InternalServerErrorError),

    Other(Value),

    #[default]
    None,
}

impl Display for ApiResponseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiResponseError::SchemaCheckFail(error) => Display::fmt(error, f),
            ApiResponseError::DocumentIdAlreadyExists(error) => Display::fmt(error, f),
            ApiResponseError::UnresolvableAbsoluteDescriptor(error) => Display::fmt(error, f),
            ApiResponseError::ApiWOQLSyntaxError(error) => write!(f, "WOQL syntax error: {}", error.error_term),
            ApiResponseError::InsertedSubdocumentAsDocument(error) => Display::fmt(error, f),
            ApiResponseError::InternalServerError(error) => Display::fmt(error, f),
            _ => f.write_str(&format!("{:#?}", self)),
        }
    }
}

// #[derive(Serialize, Deserialize, Debug, Default)]
// #[serde(tag = "@type")]
// pub enum DocumentResponseError {
//     #[serde(rename = "api:DocumentNotFound")]
//     DocumentNotFound(DocumentNotFoundError),
//
//     Other(Value),
//
//     #[default]
//     None,
// }

impl Error for ApiResponseError {}

#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum TypedErrorResponse {
    // Try typed errors first - these must have @type field that matches
    DocumentError {
        #[serde(rename = "@type")]
        #[serde(deserialize_with = "expect_document_error_type")]
        error_type: String,
        #[serde(flatten)]
        error: ErrorResponse,
    },
    ReplaceDocumentError {
        #[serde(rename = "@type")]
        #[serde(deserialize_with = "expect_replace_document_error_type")]
        error_type: String,
        #[serde(flatten)]
        error: ErrorResponse,
    },
    WoqlError {
        #[serde(rename = "@type")]
        #[serde(deserialize_with = "expect_woql_error_type")]
        error_type: String,
        #[serde(flatten)]
        error: ErrorResponse,
    },
    InsertDocumentError {
        #[serde(rename = "@type")]
        #[serde(deserialize_with = "expect_insert_document_error_type")]
        error_type: String,
        #[serde(flatten)]
        error: ErrorResponse,
    },
    DeleteDocumentError {
        #[serde(rename = "@type")]
        #[serde(deserialize_with = "expect_delete_document_error_type")]
        error_type: String,
        #[serde(flatten)]
        error: ErrorResponse,
    },

    // Fallback to generic error (no @type field)
    GenericError(ErrorResponse),
}

// Helper functions to validate the @type field
fn expect_document_error_type<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    if s == "api:GetDocumentErrorResponse" {
        Ok(s)
    } else {
        Err(serde::de::Error::custom("not a document error"))
    }
}

fn expect_replace_document_error_type<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    if s == "api:ReplaceDocumentErrorResponse" {
        Ok(s)
    } else {
        Err(serde::de::Error::custom("not a replace document error"))
    }
}

fn expect_woql_error_type<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    if s == "api:WoqlErrorResponse" {
        Ok(s)
    } else {
        Err(serde::de::Error::custom("not a woql error"))
    }
}

fn expect_insert_document_error_type<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    if s == "api:InsertDocumentErrorResponse" {
        Ok(s)
    } else {
        Err(serde::de::Error::custom("not an insert document error"))
    }
}

fn expect_delete_document_error_type<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    if s == "api:DeleteDocumentErrorResponse" {
        Ok(s)
    } else {
        Err(serde::de::Error::custom("not a delete document error"))
    }
}

impl Error for TypedErrorResponse {}

impl Display for TypedErrorResponse {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            TypedErrorResponse::DocumentError { error, .. } => {
                write!(f, "{}\n\nDetailed error: {:#?}", error, error)
            }
            TypedErrorResponse::ReplaceDocumentError { error, .. } => {
                write!(f, "{}\n\nDetailed error: {:#?}", error, error)
            }
            TypedErrorResponse::WoqlError { error, .. } => {
                write!(f, "{}\n\nDetailed error: {:#?}", error, error)
            }
            TypedErrorResponse::InsertDocumentError { error, .. } => {
                write!(f, "{}\n\nDetailed error: {:#?}", error, error)
            }
            TypedErrorResponse::DeleteDocumentError { error, .. } => {
                write!(f, "{}\n\nDetailed error: {:#?}", error, error)
            }
            TypedErrorResponse::GenericError(e) => {
                write!(f, "{}\n\nDetailed error: {:#?}", e, e)
            }
        }
    }
}

#[test]
fn test_deser_doc_err() {
    let json = json!(
         {
            "@type": "api:DocumentNotFound",
            "api:document_id": "Activity/15656455483201944534",
        }
    );

    serde_json::from_value::<ApiResponseError>(json).unwrap();

    let json = json!(
         {
            "@type": "api:GetDocumentErrorResponse",
            "api:error":  {
                "@type": "api:DocumentNotFound",
                "api:document_id": "Activity/15656455483201944534",
            },
            "api:message": "Document not found: 'Activity/15656455483201944534'",
            "api:status": "api:not_found",
        }
    );

    serde_json::from_value::<TypedErrorResponse>(json).unwrap();
}

#[test]
fn deser_doc_replace_err() {
    serde_json::from_value::<TypedErrorResponse>(json!(
        {
            "@type": ("api:ReplaceDocumentErrorResponse"),
            "api:error":  {
                "@type": ("api:SchemaCheckFailure"),
                "api:witnesses":  [
                     {
                        "@type": ("references_untyped_object"),
                        "object": ("terminusdb:///data/User/19174012-69e1-4737-b3d4-418886d40497"),
                        "predicate": ("terminusdb:///schema#owner"),
                        "subject": ("terminusdb:///data/Activity/79821724-9d57-4326-bba7-0d23cf97f6e7"),
                    },
                ],
            },
            "api:message": ("Schema check failure"),
            "api:status": ("api:failure"),
        }
    )).unwrap();
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ErrorResponse<E = ApiResponseError> {
    #[serde(rename = "api:error")]
    #[serde(default)]
    pub api_error: Option<E>,

    #[serde(rename = "api:message")]
    pub api_message: String,

    #[serde(rename = "api:status")]
    pub api_status: TerminusAPIStatus,

    // todo: make enum
    #[serde(rename = "api:what")]
    #[serde(default)]
    pub api_what: Option<String>,

    #[serde(rename = "api:request_id")]
    #[serde(default)]
    pub api_request_id: Option<String>,
}

impl Display for ErrorResponse {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.api_message)?;

        if let Some(api_error) = &self.api_error {
            match api_error {
                ApiResponseError::SchemaCheckFail(schema_error) => {
                    write!(f, "\n\n{}", schema_error)?;
                }
                _ => {
                    write!(f, "\n\nError details: {:#?}", api_error)?;
                }
            }
        }

        Ok(())
    }
}

impl Error for ErrorResponse {}

#[test]
fn test_deserialize_err() {
    let err = json!(
        {
            "@type": ("api:GetDocumentErrorResponse"),
            "api:error": {
                "@type": ("api:DocumentNotFound"),
                "api:document_id": ("User/18282169179530546952"),
            },
            "api:message": ("Document not found: 'User/18282169179530546952'"),
            "api:status": ("api:not_found"),
        }
    );

    let res: ErrorResponse = serde_json::from_value(err.clone()).unwrap();
}

#[derive(Serialize, Deserialize, Debug)]
pub struct UnknownDatabaseError {
    #[serde(rename = "api:database_name")]
    pub database_name: String,

    #[serde(rename = "api:organization_name")]
    pub organization_name: String,
}

impl Display for UnknownDatabaseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!("{:#?}", self))
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct NotAllCapturesFoundError {
    #[serde(rename = "api:captures")]
    pub captures: Vec<String>,
}

impl Display for NotAllCapturesFoundError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!("{:#?}", self))
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct WOQLSyntaxError {
    #[serde(rename = "vio:path")]
    pub path: Vec<serde_json::Value>, // todo: type

    /// cannot strongly type Query because it might have invalid syntax
    #[serde(rename = "vio:query")]
    pub query: serde_json::Value,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ApiWOQLSyntaxError {
    #[serde(rename = "api:error_term")]
    pub error_term: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DocumentNotFoundError {
    #[serde(rename = "api:document_id")]
    pub document_id: String,
}

impl Display for DocumentNotFoundError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!("{:#?}", self))
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SchemaCheckFailError {
    #[serde(rename = "api:witnesses")]
    pub witnesses: Vec<Value>,
}

impl Display for SchemaCheckFailError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Schema check failure: {:#?}\n\n💡 Hint: Schema failures often occur when a model's structure was changed after inserting its schema.\nTo resolve this:\n  1. Re-insert the updated schema, or\n  2. Reset the database using client.reset_database()", self)
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DocumentIdAlreadyExistsError {
    #[serde(rename = "api:document_id")]
    pub document_id: String,

    #[serde(rename = "api:document")]
    pub document: Value,
}

impl Display for DocumentIdAlreadyExistsError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Document with ID '{}' already exists", self.document_id)
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct InsertedSubdocumentAsDocumentError {
    #[serde(rename = "api:document")]
    pub document: Value,
}

impl Display for InsertedSubdocumentAsDocumentError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Attempted to insert a subdocument as a top-level document")
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct UnresolvableAbsoluteDescriptorError {
    #[serde(rename = "api:absolute_descriptor")]
    pub absolute_descriptor: String,
}

impl Display for UnresolvableAbsoluteDescriptorError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Could not resolve descriptor: '{}'", self.absolute_descriptor)
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct InternalServerErrorError {
    // The api:InternalServerError type appears to have no additional fields
    // beyond what's in the parent ErrorResponse
}

impl Display for InternalServerErrorError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Internal server error")
    }
}

#[test]
fn test_deserialize_document_id_already_exists_error() {
    // Test the exact JSON from the user's error message
    let json = json!({
        "@type": "api:InsertDocumentErrorResponse",
        "api:error": {
            "@type": "api:DocumentIdAlreadyExists",
            "api:document": {
                "@capture": "WorkflowInstance/689e4fcd-0100-4f82-a7fd-389066361d5f",
                "@id": "WorkflowInstance/689e4fcd-0100-4f82-a7fd-389066361d5f",
                "@type": "WorkflowInstance",
                "created_at": "2025-07-08T16:59:20.320400+00:00",
                "id": "WorkflowInstance/689e4fcd-0100-4f82-a7fd-389066361d5f",
                "state": "WorkflowState/38d1a1ef-311c-4bd9-a945-e64c5498dc52",
                "updated_at": "2025-07-08T16:59:20.320402+00:00",
                "workflow": "Workflow/aws-main-document-workflow",
            },
            "api:document_id": "terminusdb:///data/WorkflowInstance/689e4fcd-0100-4f82-a7fd-389066361d5f",
        },
        "api:message": "Tried to insert a new document with id 'terminusdb:///data/WorkflowInstance/689e4fcd-0100-4f82-a7fd-389066361d5f', but an object with that id already exists",
        "api:status": "api:failure",
    });

    // Test deserialization into TypedErrorResponse
    let response: TypedErrorResponse = serde_json::from_value(json.clone()).unwrap();

    match response {
        TypedErrorResponse::InsertDocumentError { error: err, .. } => {
            assert_eq!(err.api_message, "Tried to insert a new document with id 'terminusdb:///data/WorkflowInstance/689e4fcd-0100-4f82-a7fd-389066361d5f', but an object with that id already exists");
            // Check status by matching instead of equality
            match err.api_status {
                TerminusAPIStatus::Failure => {} // expected
                _ => panic!("Expected Failure status"),
            }

            if let Some(ApiResponseError::DocumentIdAlreadyExists(doc_err)) = err.api_error {
                assert_eq!(
                    doc_err.document_id,
                    "terminusdb:///data/WorkflowInstance/689e4fcd-0100-4f82-a7fd-389066361d5f"
                );
                assert_eq!(
                    doc_err.document["@id"],
                    "WorkflowInstance/689e4fcd-0100-4f82-a7fd-389066361d5f"
                );
            } else {
                panic!("Expected DocumentIdAlreadyExists error");
            }
        }
        _ => panic!("Expected InsertDocumentError variant"),
    }

    // Test deserialization into ApiResponse
    use crate::result::ApiResponse;
    let api_response: ApiResponse<Value> = serde_json::from_value(json).unwrap();

    match api_response {
        ApiResponse::Error(_) => {
            // Success - it correctly identified this as an error
        }
        ApiResponse::Success(_) => {
            panic!("Expected error response, got success");
        }
    }
}

#[test]
fn test_deserialize_unresolvable_absolute_descriptor_error() {
    // Test the exact JSON from the user's error message
    let json = json!({
        "@type": "api:WoqlErrorResponse",
        "api:error": {
            "@type": "api:UnresolvableAbsoluteDescriptor",
            "api:absolute_descriptor": "admin/test/local/branch/main",
        },
        "api:message": "The following descriptor could not be resolved to a resource: 'admin/test/local/branch/main'",
        "api:status": "api:not_found",
    });

    // Test deserialization into TypedErrorResponse
    let response: TypedErrorResponse = serde_json::from_value(json.clone()).unwrap();

    match response {
        TypedErrorResponse::WoqlError { error: err, .. } => {
            assert_eq!(
                err.api_message,
                "The following descriptor could not be resolved to a resource: 'admin/test/local/branch/main'"
            );
            // Check status
            match err.api_status {
                TerminusAPIStatus::NotFound => {} // expected
                _ => panic!("Expected NotFound status"),
            }

            if let Some(ApiResponseError::UnresolvableAbsoluteDescriptor(desc_err)) =
                err.api_error
            {
                assert_eq!(
                    desc_err.absolute_descriptor,
                    "admin/test/local/branch/main"
                );
            } else {
                panic!("Expected UnresolvableAbsoluteDescriptor error");
            }
        }
        _ => panic!("Expected WoqlError variant"),
    }

    // Test deserialization into ApiResponse
    use crate::result::ApiResponse;
    let api_response: ApiResponse<Value> = serde_json::from_value(json).unwrap();

    match api_response {
        ApiResponse::Error(_) => {
            // Success - it correctly identified this as an error
        }
        ApiResponse::Success(_) => {
            panic!("Expected error response, got success");
        }
    }
}


#[test]
fn test_deserialize_api_woql_syntax_error() {
    // Test the exact JSON from the user's error message
    let json = json!({
        "@type": "api:WoqlErrorResponse",
        "api:error": {
            "@type": "api:WOQLSyntaxError",
            "api:error_term": "unresolvable_prefix(rdf,type)",
        },
        "api:message": "Unknown syntax error in WOQL: \"unresolvable_prefix(rdf,type)\"",
        "api:status": "api:failure",
    });

    // Test deserialization into TypedErrorResponse
    let response: TypedErrorResponse = serde_json::from_value(json.clone()).unwrap();

    match response {
        TypedErrorResponse::WoqlError { error: err, .. } => {
            assert_eq!(
                err.api_message,
                "Unknown syntax error in WOQL: \"unresolvable_prefix(rdf,type)\""
            );
            // Check status
            match err.api_status {
                TerminusAPIStatus::Failure => {} // expected
                _ => panic!("Expected Failure status"),
            }

            if let Some(ApiResponseError::ApiWOQLSyntaxError(syntax_err)) = err.api_error {
                assert_eq!(syntax_err.error_term, "unresolvable_prefix(rdf,type)");
            } else {
                panic!("Expected ApiWOQLSyntaxError error");
            }
        }
        _ => panic!("Expected WoqlError variant"),
    }

    // Test deserialization into ApiResponse
    use crate::result::ApiResponse;
    let api_response: ApiResponse<Value> = serde_json::from_value(json).unwrap();

    match api_response {
        ApiResponse::Error(_) => {
            // Success - it correctly identified this as an error
        }
        ApiResponse::Success(_) => {
            panic!("Expected error response, got success");
        }
    }
}

#[test]
fn test_deserialize_inserted_subdocument_as_document_error() {
    // Test the exact JSON from the user's error message
    let json = json!({
        "@type": "api:InsertDocumentErrorResponse",
        "api:error": {
            "@type": "api:InsertedSubdocumentAsDocument",
            "api:document": {
                "@type": "IdAndTitle",
                "id": "/document/a0bdd8c0-25a8-48b6-b8b9-bebba4cc3fc6",
                "title": null,
            },
        },
        "api:message": "Attempted to insert a subdocument as a top-level document",
        "api:status": "api:failure",
    });

    // Test deserialization into TypedErrorResponse
    let response: TypedErrorResponse = serde_json::from_value(json.clone()).unwrap();

    match response {
        TypedErrorResponse::InsertDocumentError { error: err, .. } => {
            if let Some(ApiResponseError::InsertedSubdocumentAsDocument(subdoc_err)) = err.api_error {
                // Check that the document field was properly deserialized
                assert_eq!(subdoc_err.document["@type"], "IdAndTitle");
                assert_eq!(subdoc_err.document["id"], "/document/a0bdd8c0-25a8-48b6-b8b9-bebba4cc3fc6");
                assert_eq!(subdoc_err.document["title"], Value::Null);
            } else {
                panic!("Expected InsertedSubdocumentAsDocument error");
            }
        }
        _ => panic!("Expected InsertDocumentError variant"),
    }

    // Test deserialization into ApiResponse
    use crate::result::ApiResponse;
    let api_response: ApiResponse<Value> = serde_json::from_value(json).unwrap();

    match api_response {
        ApiResponse::Error(_) => {
            // Success - it correctly identified this as an error
        }
        ApiResponse::Success(_) => {
            panic!("Expected error response, got success");
        }
    }
}

#[test]
fn test_deserialize_generic_error() {
    // Test the new generic error case from the user's error message
    let json = json!({
        "api:message": "Unexpected failure in request handler",
        "api:status": "api:failure",
    });

    // Test deserialization into TypedErrorResponse
    let response: TypedErrorResponse = serde_json::from_value(json.clone()).unwrap();

    match response {
        TypedErrorResponse::GenericError(err) => {
            assert_eq!(err.api_message, "Unexpected failure in request handler");
            // Check status
            match err.api_status {
                TerminusAPIStatus::Failure => {} // expected
                _ => panic!("Expected Failure status"),
            }
            // Generic error should not have api_error field
            assert!(err.api_error.is_none());
        }
        _ => panic!("Expected GenericError variant"),
    }

    // Test deserialization into ApiResponse
    use crate::result::ApiResponse;
    let api_response: ApiResponse<Value> = serde_json::from_value(json).unwrap();

    match api_response {
        ApiResponse::Error(TypedErrorResponse::GenericError(err)) => {
            assert_eq!(err.api_message, "Unexpected failure in request handler");
        }
        _ => panic!("Expected error response with GenericError variant"),
    }
}

#[test]
fn test_deserialize_internal_server_error() {
    // Test the first internal server error case
    let json = json!({
        "@type": "api:ErrorResponse",
        "api:error": {
            "@type": "api:InternalServerError",
        },
        "api:message": "Processing error: rust_io_error('StorageFull',\"No space left on device (os error 28)\")",
        "api:request_id": "27195e18-c159-11f0-bb21-43c5d7caa8ab",
        "api:status": "api:server_error",
    });

    // Test deserialization into TypedErrorResponse
    let response: TypedErrorResponse = serde_json::from_value(json.clone()).unwrap();

    match response {
        TypedErrorResponse::GenericError(err) => {
            assert_eq!(err.api_message, "Processing error: rust_io_error('StorageFull',\"No space left on device (os error 28)\")");
            // Check status
            match err.api_status {
                TerminusAPIStatus::ServerError => {} // expected
                _ => panic!("Expected ServerError status"),
            }

            if let Some(ApiResponseError::InternalServerError(_)) = err.api_error {
                // Success - the error was properly deserialized
            } else {
                panic!("Expected InternalServerError error, got: {:?}", err.api_error);
            }
        }
        _ => panic!("Expected GenericError variant for api:ErrorResponse"),
    }

    // Test deserialization into ApiResponse
    use crate::result::ApiResponse;
    let api_response: ApiResponse<Value> = serde_json::from_value(json).unwrap();

    match api_response {
        ApiResponse::Error(_) => {
            // Success - it correctly identified this as an error
        }
        ApiResponse::Success(_) => {
            panic!("Expected error response, got success");
        }
    }

    // Test the second internal server error case (panic message)
    let json2 = json!({
        "@type": "api:ErrorResponse",
        "api:error": {
            "@type": "api:InternalServerError",
        },
        "api:message": "Processing error: rust_error(panic(\"Expected rollup file to have two lines but was unable to skip to the second line\"))",
        "api:request_id": "3dcf65c6-c15e-11f0-a3a9-22dfe8ec24d5",
        "api:status": "api:server_error",
    });

    // Test deserialization into TypedErrorResponse
    let response2: TypedErrorResponse = serde_json::from_value(json2.clone()).unwrap();

    match response2 {
        TypedErrorResponse::GenericError(err) => {
            assert_eq!(err.api_message, "Processing error: rust_error(panic(\"Expected rollup file to have two lines but was unable to skip to the second line\"))");
            assert!(err.api_request_id.is_some());
            assert_eq!(err.api_request_id.unwrap(), "3dcf65c6-c15e-11f0-a3a9-22dfe8ec24d5");

            if let Some(ApiResponseError::InternalServerError(_)) = err.api_error {
                // Success
            } else {
                panic!("Expected InternalServerError error");
            }
        }
        _ => panic!("Expected GenericError variant"),
    }

    // Test deserialization into ApiResponse
    let api_response2: ApiResponse<Value> = serde_json::from_value(json2).unwrap();

    match api_response2 {
        ApiResponse::Error(_) => {
            // Success
        }
        ApiResponse::Success(_) => {
            panic!("Expected error response, got success");
        }
    }
}

#[test]
fn test_deserialize_delete_document_error() {
    // Test the exact JSON from the user's error message
    let json = json!({
        "@type": "api:DeleteDocumentErrorResponse",
        "api:error": {
            "@type": "api:DocumentNotFound",
            "api:document_id": "terminusdb:///data/AwsDBReviewSession/c6f57192-a6f1-498b-989c-d3e9fa856362"
        },
        "api:message": "Document not found: \"terminusdb:///data/AwsDBReviewSession/c6f57192-a6f1-498b-989c-d3e9fa856362\"",
        "api:request_id": "4bc5e800-c4c2-11f0-b9fb-2ebc58a00d09",
        "api:status": "api:not_found"
    });

    // Test deserialization into TypedErrorResponse
    let response: TypedErrorResponse = serde_json::from_value(json.clone()).unwrap();

    match response {
        TypedErrorResponse::DeleteDocumentError { error: err, .. } => {
            assert_eq!(
                err.api_message,
                "Document not found: \"terminusdb:///data/AwsDBReviewSession/c6f57192-a6f1-498b-989c-d3e9fa856362\""
            );
            // Check status
            match err.api_status {
                TerminusAPIStatus::NotFound => {} // expected
                _ => panic!("Expected NotFound status"),
            }

            if let Some(ApiResponseError::DocumentNotFound(doc_err)) = err.api_error {
                assert_eq!(
                    doc_err.document_id,
                    "terminusdb:///data/AwsDBReviewSession/c6f57192-a6f1-498b-989c-d3e9fa856362"
                );
            } else {
                panic!("Expected DocumentNotFound error");
            }

            // Verify request_id is captured
            assert!(err.api_request_id.is_some());
            assert_eq!(err.api_request_id.unwrap(), "4bc5e800-c4c2-11f0-b9fb-2ebc58a00d09");
        }
        _ => panic!("Expected DeleteDocumentError variant"),
    }

    // Test deserialization into ApiResponse
    use crate::result::ApiResponse;
    let api_response: ApiResponse<Value> = serde_json::from_value(json).unwrap();

    match api_response {
        ApiResponse::Error(_) => {
            // Success - it correctly identified this as an error
        }
        ApiResponse::Success(_) => {
            panic!("Expected error response, got success");
        }
    }
}
