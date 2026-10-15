use std::io;
use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ModelError {
    #[error("invalid id `{id}`: {reason}")]
    InvalidId { id: String, reason: &'static str },
    #[error("{}: no theme.ron or plugin.ron", .0.display())]
    NoManifest(PathBuf),
    #[error("{}: both theme.ron and plugin.ron; a package is one or the other", .0.display())]
    AmbiguousManifest(PathBuf),
    #[error("{path}:{line}:{column}: {message}")]
    Parse {
        path: String,
        line: usize,
        column: usize,
        message: String,
    },
    #[error("archive entry `{0}` escapes the package")]
    UnsafeArchivePath(String),
    #[error("{}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

impl ModelError {
    pub(crate) fn io(path: impl Into<PathBuf>) -> impl FnOnce(io::Error) -> Self {
        let path = path.into();
        move |source| Self::Io { path, source }
    }
}
