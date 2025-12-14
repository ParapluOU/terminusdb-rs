use std::fmt::{Display, Formatter};
use glob::GlobError;

#[derive(Debug)]
pub enum TerminusDBError {
    IO(std::io::Error),
    HTTP(isahc::Error),
    HTTPErr(isahc::http::Error),
    Cmd(subprocess::CaptureData),
    Serde(serde_json::Error),
    Glob(glob::GlobError),
    UnexpectedVariableBinding(String),
    Other(String),
}

impl Display for TerminusDBError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            TerminusDBError::IO(err) => {
                err.fmt(f)
            }
            TerminusDBError::Cmd(err) => {
                f.write_str(&format!("there was an error running a system command: {:#?}", err))
            }
            TerminusDBError::Serde(err) => {
                err.fmt(f)
            }
            TerminusDBError::Other(err) => {
                f.write_str(&err)
            }
            TerminusDBError::UnexpectedVariableBinding(err) => {
                f.write_str(&err)
            }
            TerminusDBError::Glob(err) => {
                f.write_str(&err.to_string())
            }
            TerminusDBError::HTTP(err) => {
                f.write_str(&err.to_string())
            }
            TerminusDBError::HTTPErr(err) => {
                f.write_str(&err.to_string())
            }
        }
    }
}

// todo: use crate for auto From impl
impl From<std::io::Error> for TerminusDBError {
    fn from(err: std::io::Error) -> Self {
        Self::IO(err)
    }
}

impl From<serde_json::Error> for TerminusDBError {
    fn from(err: serde_json::Error) -> Self {
        Self::Serde(err)
    }
}

impl From<glob::GlobError> for TerminusDBError {
    fn from(err: GlobError) -> Self {
        Self::Glob(err)
    }
}

impl From<isahc::Error> for TerminusDBError {
    fn from(err: isahc::Error) -> Self {
        Self::HTTP(err)
    }
}