use std::{
    error::Error,
    io,
    num::{ParseIntError, TryFromIntError},
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
    UnknownError(String),
    #[error("Error making network request: {0}")]
    NetworkError(Box<ureq::Error>),
    #[error(transparent)]
    ZipError(#[from] zip::result::ZipError),
    #[error("Error parsing JSON: {0}")]
    JsonError(Box<dyn Error + Send + Sync + 'static>),
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
    #[error("Unable to convert integer: {0}")]
    IntConversionError(#[from] TryFromIntError),
}

// ureq::Error is ~240 bytes so we store it in a box
impl From<ureq::Error> for ThermiteError {
    fn from(value: ureq::Error) -> Self {
        Self::NetworkError(Box::new(value))
    }
}

impl From<json5::Error> for ThermiteError {
    fn from(value: json5::Error) -> Self {
        Self::JsonError(value.into())
    }
}

impl From<serde_json::Error> for ThermiteError {
    fn from(value: serde_json::Error) -> Self {
        Self::JsonError(value.into())
    }
}
