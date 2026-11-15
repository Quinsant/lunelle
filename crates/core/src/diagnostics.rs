use std::path::PathBuf;

use thiserror::Error;

use crate::TaskError;

#[derive(Debug, Error)]
pub enum ResourceError {
    #[error("resource I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("resource I/O error for {path:?}: {source}")]
    IoAt {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("resource path escapes the resource directory: {0:?}")]
    OutsideResourceRoot(PathBuf),
    #[error("resource image decode failed: {0}")]
    ImageDecode(#[from] image::ImageError),
    #[error("resource is not valid UTF-8: {0}")]
    InvalidEncoding(#[from] std::str::Utf8Error),
    #[error("resource load cancelled")]
    Cancelled,
    #[error("resource task failed: {0}")]
    Task(#[from] TaskError),
    #[error("resource cache lock was poisoned")]
    CachePoisoned,
    #[error("resource path must be relative")]
    InvalidPath,
}

#[derive(Debug, Error)]
pub enum UiError {
    #[error("component error: {0}")]
    Component(String),
    #[error("style error: {0}")]
    Style(String),
    #[error("layout error: {0}")]
    Layout(String),
    #[error("render error: {0}")]
    Render(String),
    #[error(transparent)]
    Resource(#[from] ResourceError),
    #[error("platform error: {0}")]
    Platform(#[source] Box<dyn std::error::Error>),
}
