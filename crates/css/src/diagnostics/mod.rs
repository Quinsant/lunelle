use std::path::PathBuf;
use thiserror::Error;

/// Recoverable issue found while parsing or computing CSS.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
#[error("CSS parse error at rule {rule}: {message}")]
pub struct CssDiagnostic {
    /// Zero-based stylesheet rule index associated with the issue.
    pub rule: usize,
    /// Human-readable diagnostic message.
    pub message: String,
}

/// Failure to load a stylesheet from a resource root.
#[derive(Debug, Error)]
pub enum CssLoadError {
    /// Reading the stylesheet failed.
    #[error("failed to load stylesheet: {0}")]
    Io(#[from] std::io::Error),
    /// The requested path escaped the resource root.
    #[error("stylesheet path escapes the resource directory")]
    OutsideResourceRoot,
    /// Resolving the requested stylesheet path failed.
    #[error("failed to resolve stylesheet path {path:?}: {source}")]
    Resolve {
        /// Path that could not be resolved.
        path: PathBuf,
        #[source]
        /// Underlying path resolution or I/O error.
        source: std::io::Error,
    },
}
