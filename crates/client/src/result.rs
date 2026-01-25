use crate::{err::TypedErrorResponse, BranchSpec, CommitId, TerminusDBAdapterError};
use anyhow::anyhow;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::convert::identity;
use std::convert::{From, Into};
use std::error::Error;
use std::fmt::{Debug, Formatter};
use std::iter::FilterMap;
use std::slice::Iter;
use terminusdb_schema::{EntityIDFor, ToTDBSchema};
use terminusdb_woql_builder::prelude::vars;

use crate::TerminusDBAdapterError::Serde;
use crate::*;

/// Response from the squash endpoint
#[derive(Debug, Serialize, Deserialize)]
pub struct SquashResponse {
    #[serde(rename = "@type")]
    pub r#type: String,
    #[serde(rename = "api:commit")]
    pub commit: String,
    #[serde(rename = "api:old_commit")]
    pub old_commit: String,
    #[serde(rename = "api:status")]
    pub status: TerminusAPIStatus,
}

/// Commit information for operations that require author and message
#[derive(Debug, Serialize)]
pub struct CommitInfo {
    pub author: String,
    pub message: String,
}

/// Transparent wrapper that includes both the response data and relevant HTTP headers
/// Implements Deref so it can be used as a drop-in replacement for the wrapped type
#[derive(Debug, Clone)]
pub struct ResponseWithHeaders<T> {
    data: T,
    pub commit_id: Option<CommitId>,
}

impl<T> ResponseWithHeaders<T> {
    pub fn new(data: T, commit_id: Option<CommitId>) -> Self {
        Self { data, commit_id }
    }

    pub fn new_with_string(data: T, commit_id: Option<String>) -> Self {
        Self {
            data,
            commit_id: commit_id.map(CommitId::from),
        }
    }

    pub fn without_headers(data: T) -> Self {
        Self {
            data,
            commit_id: None,
        }
    }

    pub fn into_inner(self) -> T {
        self.data
    }

    pub fn as_inner(&self) -> &T {
        &self.data
    }

    pub fn as_inner_mut(&mut self) -> &mut T {
        &mut self.data
    }

    /// Extract the commit ID from the TerminusDB-Data-Version header
    /// Format is typically "branch:COMMIT_ID", this returns just the COMMIT_ID part
    pub fn extract_commit_id(&self) -> Option<CommitId> {
        self.commit_id.as_ref().and_then(|commit_id| {
            // Split on ':' and take the last part (the actual commit ID)
            commit_id.as_str().split(':').last().map(CommitId::from)
        })
    }
}

impl<T> std::ops::Deref for ResponseWithHeaders<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<T> std::ops::DerefMut for ResponseWithHeaders<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

impl<T> AsRef<T> for ResponseWithHeaders<T> {
    fn as_ref(&self) -> &T {
        &self.data
    }
}

impl<T> AsMut<T> for ResponseWithHeaders<T> {
    fn as_mut(&mut self) -> &mut T {
        &mut self.data
    }
}

// Implementation for the specific HashMap result type
impl ResponseWithHeaders<HashMap<String, TDBInsertInstanceResult>> {
    /// Filter and convert IDs of type T to versioned entity references.
    ///
    /// Only extracts IDs that successfully parse as `EntityIDFor<T>`.
    /// Since a single insert operation can contain multiple document types,
    /// this acts as a filter to extract only IDs matching the specified type T.
    ///
    /// Requires a commit ID to be present in the response headers.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let result = client.insert_documents(instances, args).await?;
    ///
    /// // Extract only Person IDs as versioned references
    /// let person_refs: Vec<VersionedEntityIDFor<Person>> =
    ///     result.into_versioned_refs()?;
    ///
    /// // IDs of other types (e.g., Company) are filtered out
    /// ```
    pub fn into_versioned_refs<T: ToTDBSchema>(
        self,
    ) -> anyhow::Result<Vec<VersionedEntityIDFor<T>>> {
        let commit_id = self
            .extract_commit_id()
            .ok_or_else(|| anyhow!("No commit ID in response headers"))?;

        // Filter: only include IDs that parse successfully as EntityIDFor<T>
        let refs: Vec<_> = self
            .data
            .into_iter()
            .filter_map(|(id_str, _result)| {
                EntityIDFor::<T>::new_unchecked(&id_str)
                    .ok()
                    .map(|entity_id| VersionedEntityIDFor::new(entity_id, commit_id.clone()))
            })
            .collect();

        Ok(refs)
    }
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum ApiResponse<R> {
    Error(TypedErrorResponse),
    Success(R),
}

// #[derive(Debug, Clone)]
// #[serde(untagged)]
// pub enum DocumentResult {
//     Error(TerminusDBAdapterError),
//     NodeURIs(Vec<String>),
// }

#[derive(Debug, Deserialize)]
#[serde(tag = "@type")]
pub enum DocumentError {
    #[serde(alias = "api:ReplaceDocumentErrorResponse")]
    ReplaceDocumentErrorResponse(ReplaceDocumentErrorResponse),
}

/// {
//   "@type":"api:ReplaceDocumentErrorResponse",
//   "api:error": {"@type":"api:JSONInvalid"},
//   "api:message":"Submitted JSON data is invalid",
//   "api:status":"api:failure",
//   "api:what":"illegal_json"
// }
#[derive(Debug, Deserialize)]
pub struct ReplaceDocumentErrorResponse {
    #[serde(rename = "api:error")]
    api_error: APIError,
    #[serde(rename = "api:message")]
    api_message: String,
    #[serde(rename = "api:status")]
    api_status: TerminusAPIStatus,

    // todo: make enum
    #[serde(rename = "api:what")]
    #[serde(default)]
    api_what: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct APIError {
    #[serde(rename(deserialize = "@type"))]
    r#type: String,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "@type")]
pub enum QueryResult {
    #[serde(alias = "api:WoqlResponse")]
    WOQL(WOQLResult),
}

impl QueryResult {
    pub fn woql_result(self) -> WOQLResult {
        match self {
            QueryResult::WOQL(res) => res,
            _ => panic!("did not contain WOQL result"),
        }
    }
}

const FILEHASH_RESULT_FIXTURE: &str = r#" {
      "@type":"api:WoqlResponse",
      "api:status":"api:success",
      "api:variable_names": ["FileHash", "N" ],
      "bindings": [
        {
          "FileHash": {
            "@type":"xsd:string",
            "@value":"00018aa95c1b57dd2431402fee6049c0"
          },
          "N":"MSBFileMeta/c705f4638dbf9172d2b3244c514b4b7f2801b4d72d5983077f86aeab8ec35cd3"
        },
        {
          "FileHash": {
            "@type":"xsd:string",
            "@value":"00018bf9361908ec32d56c50fa386a1a"
          },
          "N":"MSBFileMeta/0ea91a3dcf53d4cfde9146f15345519f9cf1db748c21ebe5fbc13c3e59aa80a0"
        }
      ],
      "deletes":0,
      "inserts":0,
      "transaction_retry_count":0
    } "#;

#[test]
fn test_woql_response() {
    let res: QueryResult = serde_json::from_str(FILEHASH_RESULT_FIXTURE).unwrap();

    let res = res.woql_result();

    let values = res.get_variable_values("FileHash");
}

/// {
//   "@type":"api:WoqlResponse",
//   "api:status":"api:success",
//   "api:variable_names": ["Song" ],
//   "bindings": [
//     {
//       "Song":"SongTree/2ab27e184eacc9ba7e57d5e6ae9d6ad504567a2ded407b3ed8102b3b3be844bb"
//     }
//   ],
//   "deletes":0,
//   "inserts":0,
//   "transaction_retry_count":0
// }
#[derive(Debug, Serialize, Deserialize)]
pub struct WOQLResult<Binding = HashMap<String, QueryResultVariableBinding>> {
    // #[serde(rename(deserialize = "@type"))]
    // schema_type: String,
    #[serde(rename = "api:status")]
    pub api_status: TerminusAPIStatus,

    // todo: somehow typecheck these variables with preexisting variable types
    #[serde(rename = "api:variable_names")]
    pub api_variable_names: Vec<String>,

    // todo: somehow typecheck this map
    pub bindings: Vec<Binding>,
    pub deletes: usize,
    pub inserts: usize,
    pub transaction_retry_count: usize,
}

impl IWOQLQueryResult for WOQLResult {
    fn get_variable_values(&self, var: impl AsRef<str>) -> QueryResultVariableBindingValues {
        let values: Vec<&QueryResultVariableBinding> = self
            .bindings
            .iter()
            .filter_map(|b| b.get(var.as_ref()))
            .collect();

        QueryResultVariableBindingValues::from(values)
    }

    fn get_variable_first(&self, var: impl AsRef<str>) -> Option<&QueryResultVariableBinding> {
        self.bindings.iter().find_map(|b| b.get(var.as_ref()))
    }

    fn take_variable_first(&self, var: impl AsRef<str>) -> Option<QueryResultVariableBinding> {
        self.bindings
            .iter()
            .find_map(|b| b.get(var.as_ref()))
            .cloned()
    }
}

pub trait IWOQLQueryResult {
    fn get_variable_values(&self, var: impl AsRef<str>) -> QueryResultVariableBindingValues;

    fn get_variable_first(&self, var: impl AsRef<str>) -> Option<&QueryResultVariableBinding>;

    fn take_variable_first(&self, var: impl AsRef<str>) -> Option<QueryResultVariableBinding>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use terminusdb_schema_derive::TerminusDBModel;

    #[test]
    fn test_woql_response_typed_var() {
        let res: WOQLResult = serde_json::from_str(FILEHASH_RESULT_FIXTURE).unwrap();
    }

    #[derive(Debug, Clone, TerminusDBModel)]
    struct Person {
        name: String,
    }

    #[derive(Debug, Clone, TerminusDBModel)]
    struct Company {
        name: String,
    }

    #[test]
    fn test_into_versioned_refs_filters_by_type() {
        let mut results = HashMap::new();
        results.insert(
            "Person/123".to_string(),
            TDBInsertInstanceResult::Inserted("Person/123".to_string()),
        );
        results.insert(
            "Company/456".to_string(),
            TDBInsertInstanceResult::Inserted("Company/456".to_string()),
        );
        results.insert(
            "Person/789".to_string(),
            TDBInsertInstanceResult::Inserted("Person/789".to_string()),
        );

        let response = ResponseWithHeaders::new(results, Some(CommitId::new("branch:abc123")));

        // Extract only Person refs
        let person_refs: Vec<VersionedEntityIDFor<Person>> =
            response.clone().into_versioned_refs().unwrap();
        assert_eq!(person_refs.len(), 2); // Person/123 and Person/789

        // Extract only Company refs
        let company_refs: Vec<VersionedEntityIDFor<Company>> =
            response.into_versioned_refs().unwrap();
        assert_eq!(company_refs.len(), 1); // Company/456
    }

    #[test]
    fn test_into_versioned_refs_missing_commit_id() {
        let mut results = HashMap::new();
        results.insert(
            "Person/123".to_string(),
            TDBInsertInstanceResult::Inserted("Person/123".to_string()),
        );

        let response = ResponseWithHeaders::without_headers(results);

        let err = response.into_versioned_refs::<Person>().unwrap_err();
        assert!(err.to_string().contains("No commit ID"));
    }

    #[test]
    fn test_into_versioned_refs_empty_results() {
        let results = HashMap::new();
        let response = ResponseWithHeaders::new(results, Some(CommitId::new("branch:abc123")));

        let refs: Vec<VersionedEntityIDFor<Person>> = response.into_versioned_refs().unwrap();
        assert_eq!(refs.len(), 0);
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct QueryResultTypedValue {
    #[serde(rename(deserialize = "@type"))]
    pub r#type: String,
    // todo: deserialize using the schema type as defined in the 'type' field
    #[serde(rename(deserialize = "@value"))]
    pub value: serde_json::Value,
}

impl QueryResultTypedValue {
    pub fn parse<T: DeserializeOwned>(&self) -> TerminusDBResult<T> {
        serde_json::from_value(self.value.clone()).map_err(Serde)
    }

    pub fn parse_schema<T>(&self) -> TerminusDBResult<T> {
        todo!("parse according to xml schema type")
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum QueryResultVariableBinding {
    Value(QueryResultTypedValue),
    URI(String),
}

impl QueryResultVariableBinding {
    pub fn value(self) -> TerminusDBResult<QueryResultTypedValue> {
        match self {
            QueryResultVariableBinding::Value(v) => Ok(v),
            QueryResultVariableBinding::URI(uri) => {
                Err(TerminusDBAdapterError::UnexpectedVariableBinding(
                    format!("expected value but found URI: {}", uri).to_string(),
                ))
            }
        }
    }

    // todo: make trait
    // todo: properly deserialise objects with a @type field
    pub fn parse<T: DeserializeOwned>(&self) -> TerminusDBResult<T> {
        match self {
            QueryResultVariableBinding::Value(v) => v.parse(),
            QueryResultVariableBinding::URI(uri) => {
                serde_json::from_value(serde_json::Value::String(uri.clone())).map_err(Serde)
            }
        }
    }
}

#[derive(Debug)]
pub struct QueryResultVariableBindingValues<'a>(Vec<&'a QueryResultVariableBinding>);

impl<'a> QueryResultVariableBindingValues<'a> {
    // todo: make trait
    pub fn parse<T: DeserializeOwned>(&self) -> TerminusDBResult<Vec<T>> {
        self.0.iter().map(|bv| bv.parse::<T>()).collect()
    }
}

impl<'a> From<Vec<&'a QueryResultVariableBinding>> for QueryResultVariableBindingValues<'a> {
    fn from(vec: Vec<&'a QueryResultVariableBinding>) -> Self {
        Self(vec)
    }
}

// // todo: type checking against query input variables
// #[derive(Deserialize, Debug)]
// #[serde(try_from = "HashMap<String, String>")]
// pub struct QueryResultVariableBinding {
//     name: String,
//     value: String
// }
//
// impl TryFrom<HashMap<String, String>> for QueryResultVariableBinding {
//     type Error = TerminusDBError;
//
//     fn try_from(value: HashMap<String, String>) -> Result<Self, Self::Error> {
//         let (name, value) = value
//             .into_iter()
//             .collect::<Vec<(String, String)>>()
//             .first()
//             .ok_or(TerminusDBError::Other("could not deserialize variable bindings: empty map".to_string()))?
//             .to_owned();
//
//         Ok(Self {
//             name, value
//         })
//     }
// }
