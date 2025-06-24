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

    Other(Value),

    #[default]
    None,
}

impl Display for ApiResponseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!("{:#?}", self))
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
}

impl Error for TypedErrorResponse {}

impl Display for TypedErrorResponse {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            TypedErrorResponse::DocumentError(e) => Display::fmt(e, f),
            TypedErrorResponse::ReplaceDocumentError(e) => Display::fmt(e, f),
            TypedErrorResponse::WoqlError(e) => Display::fmt(e, f),
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
        f.write_str(&format!("{:#?}", self))
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
        f.write_str(&format!("{:#?}", self))
    }
}
