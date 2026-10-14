//! XDG Base Directory resolution.
//!
//! Paths are resolved from the environment for real use, but every consumer
//! takes an explicit [`Paths`] value so tests can point at a temporary layout
//! instead of the host's desktop configuration.

use std::path::PathBuf;

use crate::error::{Error, Result};

/// The XDG base directories relevant to MIME management.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    pub config_home: PathBuf,
    pub data_home: PathBuf,
    pub config_dirs: Vec<PathBuf>,
    pub data_dirs: Vec<PathBuf>,
}

impl Paths {
    /// Resolve paths from the environment, applying the XDG defaults.
    pub fn from_env() -> Result<Self> {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .ok_or(Error::NoHomeDir)?;

        Ok(Self {
            config_home: absolute_env("XDG_CONFIG_HOME").unwrap_or_else(|| home.join(".config")),
            data_home: absolute_env("XDG_DATA_HOME").unwrap_or_else(|| home.join(".local/share")),
            config_dirs: env_list("XDG_CONFIG_DIRS", &["/etc/xdg"]),
            data_dirs: env_list("XDG_DATA_DIRS", &["/usr/local/share", "/usr/share"]),
        })
    }

    /// Build a layout rooted at `home` with no system directories.
    ///
    /// Intended for tests and other callers that supply their own layout.
    pub fn rooted(home: impl Into<PathBuf>) -> Self {
        let home = home.into();
        Self {
            config_home: home.join(".config"),
            data_home: home.join(".local/share"),
            config_dirs: Vec::new(),
            data_dirs: Vec::new(),
        }
    }

    /// MIME database directories, highest priority first.
    pub fn mime_dirs(&self) -> Vec<PathBuf> {
        let mut dirs = vec![self.data_home.join("mime")];
        dirs.extend(self.data_dirs.iter().map(|d| d.join("mime")));
        dirs
    }

    /// Application directories holding `.desktop` entries, highest priority first.
    pub fn applications_dirs(&self) -> Vec<PathBuf> {
        let mut dirs = vec![self.data_home.join("applications")];
        dirs.extend(self.data_dirs.iter().map(|d| d.join("applications")));
        dirs
    }

    /// `mimeapps.list` files in the XDG precedence order, highest priority first.
    pub fn mimeapps_files(&self) -> Vec<PathBuf> {
        let mut files = vec![self.config_home.join("mimeapps.list")];
        files.extend(self.config_dirs.iter().map(|d| d.join("mimeapps.list")));
        files.push(self.data_home.join("applications/mimeapps.list"));
        files.extend(
            self.data_dirs
                .iter()
                .map(|d| d.join("applications/mimeapps.list")),
        );
        files
    }

    /// Location of the configuration file.
    pub fn config_file(&self) -> PathBuf {
        self.config_home.join("mimo_manager/config.toml")
    }
}

fn absolute_env(key: &str) -> Option<PathBuf> {
    let value = std::env::var_os(key)?;
    if value.is_empty() {
        return None;
    }
    let path = PathBuf::from(value);
    path.is_absolute().then_some(path)
}

fn env_list(key: &str, defaults: &[&str]) -> Vec<PathBuf> {
    match std::env::var_os(key) {
        Some(value) if !value.is_empty() => split_list(&value.to_string_lossy()),
        _ => defaults.iter().map(PathBuf::from).collect(),
    }
}

fn split_list(value: &str) -> Vec<PathBuf> {
    value
        .split(':')
        .filter(|part| !part.is_empty())
        .map(PathBuf::from)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_list_ignores_empty_entries() {
        assert_eq!(
            split_list("/a:/b::/c:"),
            vec![
                PathBuf::from("/a"),
                PathBuf::from("/b"),
                PathBuf::from("/c")
            ]
        );
    }

    #[test]
    fn rooted_uses_xdg_defaults_under_home() {
        let paths = Paths::rooted("/home/u");
        assert_eq!(paths.config_home, PathBuf::from("/home/u/.config"));
        assert_eq!(paths.data_home, PathBuf::from("/home/u/.local/share"));
        assert!(paths.data_dirs.is_empty());
    }

    #[test]
    fn derived_directories_are_priority_ordered() {
        let mut paths = Paths::rooted("/home/u");
        paths.data_dirs = vec![PathBuf::from("/usr/share")];

        assert_eq!(
            paths.mime_dirs(),
            vec![
                PathBuf::from("/home/u/.local/share/mime"),
                PathBuf::from("/usr/share/mime"),
            ]
        );
        assert_eq!(
            paths.mimeapps_files(),
            vec![
                PathBuf::from("/home/u/.config/mimeapps.list"),
                PathBuf::from("/home/u/.local/share/applications/mimeapps.list"),
                PathBuf::from("/usr/share/applications/mimeapps.list"),
            ]
        );
    }
}
