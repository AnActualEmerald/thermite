use std::{
    error::Error,
    io,
    num::{ParseIntError, TryFromIntError},
    path::{PathBuf, StripPrefixError},
};

use thiserror::Error;
use ureq::http::header::ToStrError;

pub type Result<T, E = ThermiteError> = std::result::Result<T, E>;

#[derive(Error, Debug)]
pub enum ThermiteError {
    #[error("No such file {0:?}")]
    MissingFile(Box<PathBuf>),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("{0}")]
    Unknown(String),
    #[error("Error making network request: {0}")]
    Network(#[from] ureq::Error),
    #[error(transparent)]
    Zip(#[from] zip::result::ZipError),
    #[error("Error parsing JSON: {0}")]
    Json(Box<dyn Error + Send + Sync + 'static>),
    #[error("Error resolving dependency {0}")]
    Dep(String),
    #[error("Error stripping directory prefix {0}\nIs the mod formatted correctly?")]
    Prefix(#[from] StripPrefixError),
    #[error("Sanity check failed: {0}")]
    Sanity(Box<dyn Error + Send + Sync + 'static>),
    #[error("Attempted to save a file but the path was None")]
    MissingPath,
    #[error("Error converting string to integer: {0}")]
    ParseInt(#[from] ParseIntError),
    #[error("Unable to convert integer: {0}")]
    IntConversion(#[from] TryFromIntError),
    #[error("Error parsing mod name: {0}")]
    Name(String),
    #[error("Expected string to be UTF8")]
    UTF8,
    #[error("Error converting header to string")]
    ToStr(#[from] ToStrError),
}

// ureq::Error is ~240 bytes so we store it in a box
// impl From<ureq::Error> for ThermiteError {
//     fn from(value: ureq::Error) -> Self {
//         Self::NetworkError(Box::new(value))
//     }
// }

impl From<json5::Error> for ThermiteError {
    fn from(value: json5::Error) -> Self {
        Self::Json(value.into())
    }
}

impl From<serde_json::Error> for ThermiteError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value.into())
    }
}

#[cfg(test)]
mod test {
    use ureq::Error;

    use super::ThermiteError;

    #[test]
    fn from_ureq() {
        let err = ureq::get("http://your_mother:8008")
            .call()
            .expect_err("How");

        let thermite_err = ThermiteError::from(err);

        if let ThermiteError::Network(u) = thermite_err {
            assert!(matches!(u, Error::Io(_)), "u was {u:?}");
        } else {
            panic!("Unexpected error type: {:?}", thermite_err);
        }
    }
}
