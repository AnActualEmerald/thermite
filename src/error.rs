use std::{
    io,
    num::ParseIntError,
    path::{PathBuf, StripPrefixError},
};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ThermiteError {
    #[error("No such file {0:?}")]
    MissingFile(PathBuf),
    #[error(transparent)]
    IoError(#[from] io::Error),
    #[error("{0}")]
    MiscError(String),
    #[error("Error making network request: {0}")]
    NetworkError(#[from] ureq::Error),
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
