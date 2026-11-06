//! Typed errors, so front ends can react (e.g. offer "stop it first" on `Running`).

use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("{path}:{line}: {msg}")]
    Parse {
        path: PathBuf,
        line: usize,
        msg: String,
    },
    #[error("configuration: {0}")]
    Config(String),
    /// A value or key that would corrupt a file (e.g. a newline injecting a second key).
    #[error("invalid {what}: {detail}")]
    Invalid { what: &'static str, detail: String },
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// Wraps an I/O error with the path it happened on.
    pub fn io(path: impl Into<PathBuf>) -> impl FnOnce(std::io::Error) -> Error {
        let path = path.into();
        move |source| Error::Io { path, source }
    }
}
