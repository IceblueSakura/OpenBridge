//! Private configuration-file loading and relative OAuth locator resolution.

use std::{
    fs,
    path::{Path, PathBuf},
};

use super::{
    ConfiguredCredentialSource, UpstreamCredentialConfigFileError, UpstreamCredentialConfiguration,
};

impl UpstreamCredentialConfiguration {
    /// Resolves relative OAuth2 locators against the private TOML document directory.
    fn resolve_auth_json_files(&mut self, directory: &Path) {
        // Resolve only new relative locators; API keys and absolute paths remain unchanged.
        for source in self.pools.values_mut() {
            let ConfiguredCredentialSource::OAuth2AuthJsonFile(path) = source else {
                continue;
            };
            if path.is_relative() {
                *path = directory.join(&*path);
            }
        }
    }
}

/// Path to the upstream credential configuration file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpstreamCredentialConfigPath(PathBuf);

impl UpstreamCredentialConfigPath {
    /// Creates an upstream credential configuration locator for the bootstrap-specified path.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self(path.into())
    }

    /// Returns the upstream credential configuration file path.
    pub fn path(&self) -> &Path {
        &self.0
    }

    /// Reads and parses the upstream credential configuration file.
    pub fn load(
        &self,
    ) -> Result<UpstreamCredentialConfiguration, UpstreamCredentialConfigFileError> {
        // Read the private configuration file while preserving path context.
        let document = fs::read_to_string(&self.0).map_err(|source| {
            UpstreamCredentialConfigFileError::Read {
                path: self.0.clone(),
                source,
            }
        })?;
        // Validate the contents before resolving locators relative to this private document.
        let mut configuration = UpstreamCredentialConfiguration::from_toml(&document)
            .map_err(UpstreamCredentialConfigFileError::Invalid)?;
        let directory = self.0.parent().unwrap_or_else(|| Path::new("."));
        configuration.resolve_auth_json_files(directory);
        Ok(configuration)
    }
}
