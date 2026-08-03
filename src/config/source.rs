//! Locates and loads the bootstrap configuration file.

use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

use thiserror::Error;

use super::{BootstrapConfig, BootstrapConfigError, parse_bootstrap_config};

/// Default bootstrap configuration path.
pub const DEFAULT_BOOTSTRAP_PATH: &str = "config/bootstrap.toml";

/// Value object used to locate the bootstrap configuration file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BootstrapConfigPath(PathBuf);

impl BootstrapConfigPath {
    /// Creates a configuration locator for a caller-supplied path.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self(path.into())
    }

    /// Uses the environment variable only to select the startup file; it cannot register or modify a Provider.
    pub fn from_environment() -> Self {
        Self::new(
            env::var("OPENBRIDGE_CONFIG").unwrap_or_else(|_| DEFAULT_BOOTSTRAP_PATH.to_owned()),
        )
    }

    /// Returns the configuration file path.
    pub fn path(&self) -> &Path {
        &self.0
    }

    /// Reads and parses the bootstrap file.
    pub fn load(&self) -> Result<BootstrapConfig, BootstrapConfigFileError> {
        // Read the specified path while preserving path and source errors for startup diagnostics.
        let document =
            fs::read_to_string(&self.0).map_err(|source| BootstrapConfigFileError::Read {
                path: self.0.clone(),
                source,
            })?;
        // Parse the content and wrap errors as file-level failures.
        parse_bootstrap_config(&document).map_err(BootstrapConfigFileError::Invalid)
    }
}

impl Default for BootstrapConfigPath {
    fn default() -> Self {
        Self::new(DEFAULT_BOOTSTRAP_PATH)
    }
}

/// Bootstrap file reading or content validation failed.
#[derive(Debug, Error)]
pub enum BootstrapConfigFileError {
    /// The bootstrap file could not be read from the specified path.
    #[error("failed to read bootstrap configuration '{path}'")]
    Read {
        /// Path of the file that could not be read.
        path: PathBuf,
        #[source]
        /// Underlying filesystem error.
        source: io::Error,
    },
    /// The file was read, but its content failed bootstrap validation.
    #[error("bootstrap configuration validation failed")]
    Invalid(#[source] BootstrapConfigError),
}
