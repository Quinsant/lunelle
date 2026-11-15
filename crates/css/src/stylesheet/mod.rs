use std::{fs, path::Path};

use crate::{CssDiagnostic, CssLoadError};

/// Parsed stylesheet rules, keyframes, and non-fatal parse diagnostics.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StyleSheet {
    /// Ordinary style rules in source order.
    pub rules: Vec<crate::StyleRule>,
    /// Parsed keyframe animations.
    pub keyframes: Vec<crate::Keyframes>,
    /// Non-fatal diagnostics produced while parsing.
    pub diagnostics: Vec<CssDiagnostic>,
}

impl StyleSheet {
    /// Loads a stylesheet beneath a resource root, rejecting path traversal.
    pub fn load(
        resource_root: impl AsRef<Path>,
        relative_path: impl AsRef<Path>,
    ) -> Result<Self, CssLoadError> {
        let relative_path = relative_path.as_ref();
        let root =
            lunelle_core::ResourceRoot::new(resource_root).map_err(|error| match error {
                lunelle_core::ResourceError::Io(source) => CssLoadError::Io(source),
                lunelle_core::ResourceError::InvalidPath
                | lunelle_core::ResourceError::OutsideResourceRoot(_) => {
                    CssLoadError::OutsideResourceRoot
                }
                other => CssLoadError::Resolve {
                    path: relative_path.to_path_buf(),
                    source: std::io::Error::other(other),
                },
            })?;
        let path = root.resolve(relative_path).map_err(|error| match error {
            lunelle_core::ResourceError::IoAt { source, .. } => CssLoadError::Resolve {
                path: relative_path.to_path_buf(),
                source,
            },
            lunelle_core::ResourceError::InvalidPath
            | lunelle_core::ResourceError::OutsideResourceRoot(_) => {
                CssLoadError::OutsideResourceRoot
            }
            lunelle_core::ResourceError::Io(source) => CssLoadError::Io(source),
            other => CssLoadError::Resolve {
                path: relative_path.to_path_buf(),
                source: std::io::Error::other(other),
            },
        })?;
        Ok(Self::parse(
            &fs::read_to_string(path).map_err(CssLoadError::Io)?,
        ))
    }

    /// Parses CSS source into rules, keyframes, and diagnostics.
    pub fn parse(source: &str) -> Self {
        crate::parser::parse_stylesheet(source)
    }
}
