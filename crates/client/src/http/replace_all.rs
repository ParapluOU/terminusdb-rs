//! Replacing everything in a graph in one commit, against a known head.

use crate::{document::DocumentInsertArgs, CommitId};

/// The request and response header that carries a branch's data version
/// (`branch:<commit id>`).
const DATA_VERSION_HEADER: &str = "TerminusDB-Data-Version";

/// Why [`replace_all_documents`](super::client::TerminusDBHttpClient::replace_all_documents)
/// did not replace anything.
#[derive(Debug, thiserror::Error)]
pub enum ReplaceAllError {
    /// The branch's head is not the commit the replacement was made for: someone
    /// else committed in between. Nothing was changed.
    #[error("the branch head is {actual}, not {expected}: nothing was replaced")]
    HeadMoved {
        /// The data version the caller expected (`branch:<commit id>`).
        expected: String,
        /// The data version the branch has.
        actual: String,
    },
    /// Anything else: the request failed, or the server refused the documents.
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[cfg(not(target_arch = "wasm32"))]
impl super::client::TerminusDBHttpClient {
    /// Replace every document in the graph (`args.ty`) of `args.spec` with
    /// `documents`, in **one commit** carrying `args.author` and `args.message`
    /// (`POST /api/document?full_replace=true`). Returns the commit made.
    ///
    /// The server deletes what the graph holds and inserts `documents` in a single
    /// transaction: there is no commit at which the graph is empty, and a failed
    /// request leaves it as it was.
    ///
    /// With `expected_head`, the request carries it as its data version and the
    /// server refuses it — [`ReplaceAllError::HeadMoved`], nothing changed — unless
    /// that commit is still the branch's head when the transaction runs. That makes
    /// read-modify-write safe without a lock: compute the replacement from a commit,
    /// then replace only if nobody committed since.
    ///
    /// `args.force`, `args.skip_existence_check`, `args.merge_repeats` and
    /// `args.raw_json` do not apply and are ignored.
    pub async fn replace_all_documents(
        &self,
        documents: Vec<&serde_json::Value>,
        args: DocumentInsertArgs,
        expected_head: Option<&CommitId>,
    ) -> Result<CommitId, ReplaceAllError> {
        use anyhow::Context;

        let uri = self
            .build_url()
            .endpoint("document")
            .database(&args.spec)
            .query_encoded("author", &args.author)
            .query_encoded("message", &args.message)
            .query("graph_type", &args.ty.to_string())
            .query("full_replace", "true")
            .build();
        let body = serde_json::to_string(&documents).context("the documents are not JSON")?;

        let _permit = self.acquire_write_permit().await;
        let mut request = self
            .http
            .post(uri)
            .basic_auth(&self.user, Some(&self.pass))
            .header("Content-Type", "application/json")
            .body(body);
        let expected = expected_head.map(|head| format!("branch:{}", head.as_str()));
        if let Some(expected) = &expected {
            request = request.header(DATA_VERSION_HEADER, expected);
        }
        if let Some(timeout) = args.timeout {
            request = request.timeout(timeout);
        }
        let response = request.send().await.context("the replace request was not answered")?;

        let status = response.status();
        let data_version = match response.headers().get(DATA_VERSION_HEADER) {
            Some(value) => Some(
                value
                    .to_str()
                    .context("the data version header of the answer is not text")?
                    .to_string(),
            ),
            None => None,
        };
        let text = response.text().await.context("the answer to the replace request could not be read")?;

        if !status.is_success() {
            // An answer that is not JSON is reported as it came.
            if let Ok(answer) = serde_json::from_str::<serde_json::Value>(&text) {
                let error = &answer["api:error"];
                if error["@type"] == "api:DataVersionMismatch" {
                    let said = |key: &str| -> Result<String, ReplaceAllError> {
                        Ok(error[key]
                            .as_str()
                            .with_context(|| format!("a data version mismatch without `{key}`: {text}"))?
                            .to_string())
                    };
                    return Err(ReplaceAllError::HeadMoved {
                        expected: said("api:requested_data_version")?,
                        actual: said("api:actual_data_version")?,
                    });
                }
            }
            return Err(anyhow::anyhow!("replacing the documents failed ({status}): {text}").into());
        }

        let data_version = data_version
            .with_context(|| format!("the server replaced the documents but named no commit ({status}): {text}"))?;
        let commit = data_version
            .strip_prefix("branch:")
            .with_context(|| format!("the data version `{data_version}` is not a branch's"))?;
        Ok(CommitId::new(commit))
    }
}
