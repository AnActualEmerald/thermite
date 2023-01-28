use std::{
    io,
    num::ParseIntError,
    path::{PathBuf, StripPrefixError},
};

use thiserror::Error;

pub type Result<T> = std::result::Result<T, ThermiteError>;

#[derive(Error, Debug)]
pub enum ThermiteError {
    #[error("No such file {0:?}")]
    MissingFile(Box<PathBuf>),
    #[error(transparent)]
    IoError(#[from] io::Error),
    #[error("{0}")]
    MiscError(String),
    #[error("Error making network request: {0}")]
    NetworkError(Box<ureq::Error>),
    #[error(transparent)]
    ZipError(#[from] zip::result::ZipError),
    #[error("Error parsing JSON: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error("Error resolving dependency {0}")]
    DepError(String),
    #[error("Error stripping directory prefix {0}\nIs the mod formatted correctly?")]
    PrefixError(#[from] StripPrefixError),
    #[error("Sanity check failed")]
    SanityError,
    #[error("Attempted to save a file but the path was None")]
    MissingPath,
    #[error("Error converting string to integer: {0}")]
    ParseIntError(#[from] ParseIntError),
}

// ureq::Error is ~240 bytes so we store it in a box
impl From<ureq::Error> for ThermiteError {
    fn from(value: ureq::Error) -> Self {
        Self::NetworkError(Box::new(value))
    }
}
