// use glob::GlobError;
use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub enum WOQLError {
    IO(std::io::Error),
    HTTP(isahc::Error),
    HTTPErr(isahc::http::Error),
    // Cmd(subprocess::CaptureData),
    Serde(serde_json::Error),
    // Glob(glob::GlobError),
    UnexpectedVariableBinding(String),
    Other(String),
}

impl Display for WOQLError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            WOQLError::IO(err) => err.fmt(f),
            // WOQLError::Cmd(err) => f.write_str(&format!(
            //     "there was an error running a system command: {:#?}",
            //     err
            // )),
            WOQLError::Serde(err) => err.fmt(f),
            WOQLError::Other(err) => f.write_str(&err),
            WOQLError::UnexpectedVariableBinding(err) => f.write_str(&err),
            // WOQLError::Glob(err) => f.write_str(&err.to_string()),
            WOQLError::HTTP(err) => f.write_str(&err.to_string()),
            WOQLError::HTTPErr(err) => f.write_str(&err.to_string()),
        }
    }
}

// todo: use crate for auto From impl
impl From<std::io::Error> for WOQLError {
    fn from(err: std::io::Error) -> Self {
        Self::IO(err)
    }
}

impl From<serde_json::Error> for WOQLError {
    fn from(err: serde_json::Error) -> Self {
        Self::Serde(err)
    }
}

// impl From<glob::GlobError> for WOQLError {
//     fn from(err: GlobError) -> Self {
//         Self::Glob(err)
//     }
// }

impl From<isahc::Error> for WOQLError {
    fn from(err: isahc::Error) -> Self {
        Self::HTTP(err)
    }
}
