//! Application configuration.
//!
//! Configuration is loaded once at startup from
//! `$XDG_CONFIG_HOME/mimo_manager/config.toml`. A missing file is not an error
//! (defaults apply); a present but malformed file is. The file is never written
//! by the application — the Home Manager module generates it.

use std::path::Path;

use serde::Deserialize;

use crate::error::{Error, Result};
use crate::theme::{self, ModelineFields, THEME_NAME};

/// The complete configuration.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Theme to render with. Must name a theme in [`crate::theme::THEMES`].
    pub theme: String,
    /// Ask before destructive operations (remove/reset).
    pub confirm_destructive: bool,
    /// Which modeline fields are shown.
    pub modeline: ModelineConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: THEME_NAME.to_string(),
            confirm_destructive: true,
            modeline: ModelineConfig::default(),
        }
    }
}

/// Which modeline fields are shown.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct ModelineConfig {
    pub mode: bool,
    pub workspace: bool,
    pub selection: bool,
    pub count: bool,
    pub default: bool,
    pub scope: bool,
    pub status: bool,
    pub theme: bool,
}

impl Default for ModelineConfig {
    fn default() -> Self {
        let fields = ModelineFields::default();
        Self {
            mode: fields.mode,
            workspace: fields.workspace,
            selection: fields.selection,
            count: fields.count,
            default: fields.default,
            scope: fields.scope,
            status: fields.status,
            theme: fields.theme,
        }
    }
}

impl From<ModelineConfig> for ModelineFields {
    fn from(config: ModelineConfig) -> Self {
        Self {
            mode: config.mode,
            workspace: config.workspace,
            selection: config.selection,
            count: config.count,
            default: config.default,
            scope: config.scope,
            status: config.status,
            theme: config.theme,
        }
    }
}

impl Config {
    /// Parse configuration from a TOML string.
    pub fn from_toml(source: &str, path: &Path) -> Result<Self> {
        let config: Self = toml::from_str(source).map_err(|error| Error::Parse {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;
        if !theme::is_known(&config.theme) {
            return Err(Error::Parse {
                path: path.to_path_buf(),
                message: format!(
                    "unknown theme {:?}; available themes: {}",
                    config.theme,
                    theme::THEMES.join(", ")
                ),
            });
        }
        Ok(config)
    }

    /// Load configuration from `path`, applying defaults when it is absent.
    pub fn load(path: &Path) -> Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(source) => Self::from_toml(&source, path),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(source) => Err(Error::Io {
                path: path.to_path_buf(),
                source,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_documented_behaviour() {
        let config = Config::default();
        assert_eq!(config.theme, THEME_NAME);
        assert!(config.confirm_destructive);
        assert!(config.modeline.mode && config.modeline.workspace);
        assert!(!config.modeline.theme, "theme field is off by default");
    }

    #[test]
    fn empty_document_yields_defaults() {
        let config = Config::from_toml("", Path::new("config.toml")).unwrap();
        assert_eq!(config, Config::default());
    }

    #[test]
    fn parses_partial_document() {
        let source = r#"
            theme = "MELANGE DARK"
            confirm_destructive = false

            [modeline]
            status = false
            theme = true
        "#;
        let config = Config::from_toml(source, Path::new("config.toml")).unwrap();
        assert_eq!(config.theme, "MELANGE DARK");
        assert!(!config.confirm_destructive);
        assert!(!config.modeline.status);
        assert!(config.modeline.theme);
        assert!(
            config.modeline.mode,
            "unspecified fields keep their default"
        );
    }

    #[test]
    fn unknown_theme_is_an_error() {
        let error = Config::from_toml("theme = \"MELANGE LIGHT\"", Path::new("config.toml"));
        assert!(matches!(error, Err(Error::Parse { .. })));
    }

    #[test]
    fn malformed_document_is_an_error() {
        let error = Config::from_toml("confirm_destructive = ", Path::new("config.toml"));
        assert!(matches!(error, Err(Error::Parse { .. })));
    }

    #[test]
    fn wrong_type_is_an_error() {
        let error = Config::from_toml("confirm_destructive = \"yes\"", Path::new("c.toml"));
        assert!(matches!(error, Err(Error::Parse { .. })));
    }

    #[test]
    fn missing_file_applies_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::load(&dir.path().join("mimo_manager/config.toml")).unwrap();
        assert_eq!(config, Config::default());
    }

    #[test]
    fn modeline_fields_round_trip() {
        let source = "[modeline]\nmode = false\nscope = false\n";
        let config = Config::from_toml(source, Path::new("config.toml")).unwrap();
        let fields: ModelineFields = config.modeline.into();
        assert!(!fields.mode);
        assert!(!fields.scope);
        assert!(fields.workspace);
    }
}
