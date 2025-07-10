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
#[serde(tag = "@type")]
pub enum TypedErrorResponse {
    #[serde(rename = "api:GetDocumentErrorResponse")]
    DocumentError(ErrorResponse),

    #[serde(rename = "api:ReplaceDocumentErrorResponse")]
    ReplaceDocumentError(ErrorResponse),

    #[serde(rename = "api:WoqlErrorResponse")]
    WoqlError(ErrorResponse),

    #[serde(rename = "api:InsertDocumentErrorResponse")]
    InsertDocumentError(ErrorResponse),
}

impl Error for TypedErrorResponse {}

impl Display for TypedErrorResponse {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            TypedErrorResponse::DocumentError(e) => {
                write!(f, "{}\n\nDetailed error: {:#?}", e, e)
            }
            TypedErrorResponse::ReplaceDocumentError(e) => {
                write!(f, "{}\n\nDetailed error: {:#?}", e, e)
            }
            TypedErrorResponse::WoqlError(e) => {
                write!(f, "{}\n\nDetailed error: {:#?}", e, e)
            }
            TypedErrorResponse::InsertDocumentError(e) => {
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
pub struct UnresolvableAbsoluteDescriptorError {
    #[serde(rename = "api:absolute_descriptor")]
    pub absolute_descriptor: String,
}

impl Display for UnresolvableAbsoluteDescriptorError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Could not resolve descriptor: '{}'", self.absolute_descriptor)
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
        TypedErrorResponse::InsertDocumentError(err) => {
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
        TypedErrorResponse::WoqlError(err) => {
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
