use std::{
    io,
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
    #[error("Error downloading file: {0}")]
    DownloadError(#[from] reqwest::Error),
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
}
