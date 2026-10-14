use std::path::PathBuf;

use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("cannot parse {path}: {message}")]
    Parse { path: PathBuf, message: String },

    #[error("cannot determine a home directory: HOME is unset or not absolute")]
    NoHomeDir,
}
