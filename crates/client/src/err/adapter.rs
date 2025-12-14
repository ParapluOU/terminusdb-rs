#[cfg(not(target_arch = "wasm32"))]
use glob::GlobError;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub enum TerminusDBAdapterError {
    #[cfg(not(target_arch = "wasm32"))]
    IO(std::io::Error),
    #[cfg(not(target_arch = "wasm32"))]
    HTTP(reqwest::Error),
    #[cfg(not(target_arch = "wasm32"))]
    HTTPErr(http::Error),
    #[cfg(not(target_arch = "wasm32"))]
    Cmd(subprocess::CaptureData),
    Serde(serde_json::Error),
    #[cfg(not(target_arch = "wasm32"))]
    Glob(glob::GlobError),
    UnexpectedVariableBinding(String),
    Other(String),
}

impl Display for TerminusDBAdapterError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            #[cfg(not(target_arch = "wasm32"))]
            TerminusDBAdapterError::IO(err) => err.fmt(f),
            #[cfg(not(target_arch = "wasm32"))]
            TerminusDBAdapterError::Cmd(err) => f.write_str(&format!(
                "there was an error running a system command: {:#?}",
                err
            )),
            TerminusDBAdapterError::Serde(err) => err.fmt(f),
            TerminusDBAdapterError::Other(err) => f.write_str(&err),
            TerminusDBAdapterError::UnexpectedVariableBinding(err) => f.write_str(&err),
            #[cfg(not(target_arch = "wasm32"))]
            TerminusDBAdapterError::Glob(err) => f.write_str(&err.to_string()),
            #[cfg(not(target_arch = "wasm32"))]
            TerminusDBAdapterError::HTTP(err) => f.write_str(&err.to_string()),
            #[cfg(not(target_arch = "wasm32"))]
            TerminusDBAdapterError::HTTPErr(err) => f.write_str(&err.to_string()),
        }
    }
}

// todo: use crate for auto From impl
#[cfg(not(target_arch = "wasm32"))]
impl From<std::io::Error> for TerminusDBAdapterError {
    fn from(err: std::io::Error) -> Self {
        Self::IO(err)
    }
}

impl From<serde_json::Error> for TerminusDBAdapterError {
    fn from(err: serde_json::Error) -> Self {
        Self::Serde(err)
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl From<glob::GlobError> for TerminusDBAdapterError {
    fn from(err: GlobError) -> Self {
        Self::Glob(err)
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl From<reqwest::Error> for TerminusDBAdapterError {
    fn from(err: reqwest::Error) -> Self {
        Self::HTTP(err)
    }
}

impl std::error::Error for TerminusDBAdapterError {}

impl Clone for TerminusDBAdapterError {
    fn clone(&self) -> Self {
        match self {
            #[cfg(not(target_arch = "wasm32"))]
            TerminusDBAdapterError::IO(err) => {
                TerminusDBAdapterError::Other(format!("Cloned IO Error: {}", err))
            }
            #[cfg(not(target_arch = "wasm32"))]
            TerminusDBAdapterError::HTTP(err) => {
                TerminusDBAdapterError::Other(format!("Cloned HTTP Error: {}", err))
            }
            #[cfg(not(target_arch = "wasm32"))]
            TerminusDBAdapterError::HTTPErr(err) => {
                TerminusDBAdapterError::Other(format!("Cloned HTTPErr Error: {}", err))
            }
            #[cfg(not(target_arch = "wasm32"))]
            TerminusDBAdapterError::Cmd(data) => {
                // CaptureData doesn't implement Clone, let's stringify it
                TerminusDBAdapterError::Other(format!("Cloned Cmd Error: {:?}", data))
            }
            TerminusDBAdapterError::Serde(err) => {
                // serde_json::Error doesn't implement Clone directly
                TerminusDBAdapterError::Other(format!("Cloned Serde Error: {}", err))
            }
            #[cfg(not(target_arch = "wasm32"))]
            TerminusDBAdapterError::Glob(err) => {
                // GlobError doesn't implement Clone
                TerminusDBAdapterError::Other(format!("Cloned Glob Error: {}", err))
            }
            TerminusDBAdapterError::UnexpectedVariableBinding(s) => {
                TerminusDBAdapterError::UnexpectedVariableBinding(s.clone())
            }
            TerminusDBAdapterError::Other(s) => TerminusDBAdapterError::Other(s.clone()),
        }
    }
}
