//! Association model and safe `mimeapps.list` editing.
//!
//! Rules enforced here:
//! * reads merge every `mimeapps.list` in XDG precedence order;
//! * writes go to exactly one file, `$XDG_CONFIG_HOME/mimeapps.list`;
//! * writes preserve comments, unknown sections and keys verbatim;
//! * the file is replaced atomically (write temp + `rename`), so a crash can
//!   never leave a truncated `mimeapps.list` behind.

use std::collections::BTreeMap;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::xdg::Paths;

pub const DEFAULT_SECTION: &str = "Default Applications";
pub const ADDED_SECTION: &str = "Added Associations";
pub const REMOVED_SECTION: &str = "Removed Associations";

/// A change to apply to the user's association file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssocEdit {
    /// Make `app` the default handler for `mime`.
    SetDefault { mime: String, app: String },
    /// Add `app` as a non-default handler for `mime`.
    Add { mime: String, app: String },
    /// Forget the association between `mime` and `app`.
    Remove { mime: String, app: String },
    /// Drop every local override for `mime`, reverting to inherited defaults.
    Reset { mime: String },
}

/// Merged, read-only view of the association state.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Associations {
    defaults: BTreeMap<String, Vec<String>>,
    added: BTreeMap<String, Vec<String>>,
    removed: BTreeMap<String, Vec<String>>,
}

impl Associations {
    /// Read every `mimeapps.list`, highest priority first.
    pub fn load(paths: &Paths) -> Result<Self> {
        let mut merged = Self::default();
        for path in paths.mimeapps_files() {
            let file = MimeappsFile::load(&path)?;
            let settings = file.settings();
            for (mime, apps) in settings.defaults {
                merged.defaults.entry(mime).or_insert(apps);
            }
            for (mime, apps) in settings.added {
                extend_unique(merged.added.entry(mime).or_default(), apps);
            }
            for (mime, apps) in settings.removed {
                extend_unique(merged.removed.entry(mime).or_default(), apps);
            }
        }
        Ok(merged)
    }

    /// The default application id for a MIME type, if any file sets one.
    pub fn default_app(&self, mime: &str) -> Option<&str> {
        self.defaults
            .get(mime)
            .and_then(|apps| apps.first())
            .map(String::as_str)
    }

    pub fn default_apps(&self, mime: &str) -> &[String] {
        self.defaults.get(mime).map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn added_for(&self, mime: &str) -> &[String] {
        self.added.get(mime).map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn removed_for(&self, mime: &str) -> &[String] {
        self.removed.get(mime).map(Vec::as_slice).unwrap_or(&[])
    }
}

fn extend_unique(target: &mut Vec<String>, values: Vec<String>) {
    for value in values {
        if !target.contains(&value) {
            target.push(value);
        }
    }
}

/// Apply `edit` to `$XDG_CONFIG_HOME/mimeapps.list`, creating it if needed.
///
/// Returns the path that was written.
pub fn apply(paths: &Paths, edit: &AssocEdit) -> Result<PathBuf> {
    let target = write_target(paths);
    let mut file = MimeappsFile::load(&target)?;
    match edit {
        AssocEdit::SetDefault { mime, app } => file.set_default(mime, app),
        AssocEdit::Add { mime, app } => file.add_association(mime, app),
        AssocEdit::Remove { mime, app } => file.remove_association(mime, app),
        AssocEdit::Reset { mime } => file.reset(mime),
    }
    file.save(&target)?;
    Ok(target)
}

/// The single file the manager is allowed to write.
pub fn write_target(paths: &Paths) -> PathBuf {
    paths.config_home.join("mimeapps.list")
}

/// Parsed settings of one `mimeapps.list`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Settings {
    defaults: BTreeMap<String, Vec<String>>,
    added: BTreeMap<String, Vec<String>>,
    removed: BTreeMap<String, Vec<String>>,
}

/// A `mimeapps.list` file with enough structure to edit it without losing
/// user content.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MimeappsFile {
    preamble: Vec<String>,
    sections: Vec<Section>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Section {
    name: String,
    lines: Vec<Line>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Line {
    /// Comments, blanks and any line we do not model, preserved verbatim.
    Raw(String),
    Entry {
        key: String,
        values: Vec<String>,
        raw: String,
        changed: bool,
    },
}

impl MimeappsFile {
    /// Load a file; a missing file yields an empty model.
    pub fn load(path: &Path) -> Result<Self> {
        match fs::read_to_string(path) {
            Ok(text) => Ok(Self::parse(&text)),
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(source) => Err(Error::Io {
                path: path.to_path_buf(),
                source,
            }),
        }
    }

    pub fn parse(text: &str) -> Self {
        let mut file = Self::default();
        let mut current: Option<usize> = None;
        for raw in text.lines() {
            let line = raw.trim_end_matches('\r');
            let trimmed = line.trim();
            if let Some(name) = trimmed.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                file.sections.push(Section {
                    name: name.to_string(),
                    lines: Vec::new(),
                });
                current = Some(file.sections.len() - 1);
                continue;
            }
            match current {
                None => file.preamble.push(line.to_string()),
                Some(index) => {
                    let section = &mut file.sections[index];
                    section.push_line(trimmed, line);
                }
            }
        }
        file
    }

    /// Render the file back to text, ending with a newline when non-empty.
    pub fn render(&self) -> String {
        let mut out = String::new();
        for line in &self.preamble {
            out.push_str(line);
            out.push('\n');
        }
        for section in &self.sections {
            out.push('[');
            out.push_str(&section.name);
            out.push_str("]\n");
            for line in &section.lines {
                out.push_str(&line.render());
                out.push('\n');
            }
        }
        out
    }

    /// Extract the three modelled sections into a [`Settings`] view.
    fn settings(&self) -> Settings {
        let mut settings = Settings::default();
        for section in &self.sections {
            let target = match section.name.as_str() {
                DEFAULT_SECTION => &mut settings.defaults,
                ADDED_SECTION => &mut settings.added,
                REMOVED_SECTION => &mut settings.removed,
                _ => continue,
            };
            for line in &section.lines {
                if let Line::Entry { key, values, .. } = line
                    && !values.is_empty()
                {
                    target.entry(key.clone()).or_insert_with(|| values.clone());
                }
            }
        }
        settings
    }

    /// Replace `key` in a section, creating section and entry as needed.
    fn set(&mut self, section_name: &str, key: &str, values: Vec<String>) {
        self.ensure_section(section_name).set(key, values);
    }

    fn set_default(&mut self, mime: &str, app: &str) {
        self.set(DEFAULT_SECTION, mime, vec![app.to_string()]);
        self.remove_value(ADDED_SECTION, mime, app);
        self.remove_value(REMOVED_SECTION, mime, app);
    }

    fn add_association(&mut self, mime: &str, app: &str) {
        self.ensure_section(ADDED_SECTION).add_value(mime, app);
        self.remove_value(REMOVED_SECTION, mime, app);
    }

    fn remove_association(&mut self, mime: &str, app: &str) {
        self.remove_value(ADDED_SECTION, mime, app);
        self.remove_value(DEFAULT_SECTION, mime, app);
        self.ensure_section(REMOVED_SECTION).add_value(mime, app);
    }

    fn reset(&mut self, mime: &str) {
        for name in [DEFAULT_SECTION, ADDED_SECTION, REMOVED_SECTION] {
            if let Some(section) = self.section_mut(name) {
                section.remove_key(mime);
            }
        }
    }

    fn remove_value(&mut self, section_name: &str, key: &str, app: &str) {
        if let Some(section) = self.section_mut(section_name) {
            section.remove_value(key, app);
        }
    }

    fn section_mut(&mut self, name: &str) -> Option<&mut Section> {
        self.sections.iter_mut().find(|s| s.name == name)
    }

    /// Borrow a section, creating it (appended at the end) when absent.
    fn ensure_section(&mut self, name: &str) -> &mut Section {
        if let Some(index) = self.sections.iter().position(|s| s.name == name) {
            return &mut self.sections[index];
        }
        self.sections.push(Section {
            name: name.to_string(),
            lines: Vec::new(),
        });
        self.sections.last_mut().expect("section was just pushed")
    }

    /// Save atomically to `path`, creating parent directories.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| Error::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let temp = temp_path(path);
        let text = self.render();
        let write = || -> std::io::Result<()> {
            let mut file = fs::File::create(&temp)?;
            file.write_all(text.as_bytes())?;
            file.sync_all()?;
            fs::rename(&temp, path)?;
            Ok(())
        };
        if let Err(source) = write() {
            let _ = fs::remove_file(&temp);
            return Err(Error::Io {
                path: path.to_path_buf(),
                source,
            });
        }
        Ok(())
    }
}

impl Section {
    fn push_line(&mut self, trimmed: &str, raw: &str) {
        if let Some((key, value)) = trimmed.split_once('=') {
            self.lines.push(Line::Entry {
                key: key.trim().to_string(),
                values: split_values(value),
                raw: raw.to_string(),
                changed: false,
            });
        } else {
            self.lines.push(Line::Raw(raw.to_string()));
        }
    }

    fn entry_index(&self, key: &str) -> Option<usize> {
        self.lines
            .iter()
            .position(|line| matches!(line, Line::Entry { key: k, .. } if k == key))
    }

    fn set(&mut self, key: &str, values: Vec<String>) {
        match self.entry_index(key) {
            Some(index) => {
                if let Line::Entry {
                    values: existing,
                    changed,
                    ..
                } = &mut self.lines[index]
                {
                    *existing = values;
                    *changed = true;
                }
            }
            None => self.lines.push(Line::Entry {
                key: key.to_string(),
                values,
                raw: String::new(),
                changed: true,
            }),
        }
    }

    fn add_value(&mut self, key: &str, value: &str) {
        match self.entry_index(key) {
            Some(index) => {
                if let Line::Entry {
                    values, changed, ..
                } = &mut self.lines[index]
                    && !values.iter().any(|v| v == value)
                {
                    values.push(value.to_string());
                    *changed = true;
                }
            }
            None => self.set(key, vec![value.to_string()]),
        }
    }

    fn remove_value(&mut self, key: &str, value: &str) {
        let Some(index) = self.entry_index(key) else {
            return;
        };
        if let Line::Entry {
            values, changed, ..
        } = &mut self.lines[index]
        {
            let before = values.len();
            values.retain(|v| v != value);
            if values.len() != before {
                *changed = true;
            }
            if values.is_empty() {
                self.lines.remove(index);
            }
        }
    }

    fn remove_key(&mut self, key: &str) {
        self.lines
            .retain(|line| !matches!(line, Line::Entry { key: k, .. } if k == key));
    }
}

impl Line {
    fn render(&self) -> String {
        match self {
            Line::Raw(raw) => raw.clone(),
            Line::Entry {
                key,
                values,
                raw,
                changed,
            } => {
                if !changed {
                    return raw.clone();
                }
                let mut rendered = String::from(key);
                rendered.push('=');
                for value in values {
                    rendered.push_str(value);
                    rendered.push(';');
                }
                rendered
            }
        }
    }
}

fn split_values(value: &str) -> Vec<String> {
    value
        .split(';')
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_string)
        .collect()
}

fn temp_path(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "mimeapps.list".to_string());
    path.with_file_name(format!(".{name}.{}.tmp", std::process::id()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
# personal preferences
[Default Applications]
text/html=helium.desktop
text/plain=org.gnome.TextEditor.desktop;
image/png=org.gnome.eog.desktop

[Added Associations]
text/plain=org.gnome.TextEditor.desktop;

[Other]
Untouched=yes
";

    #[test]
    fn parse_and_render_is_lossless_when_unchanged() {
        let file = MimeappsFile::parse(SAMPLE);
        assert_eq!(file.render(), SAMPLE);
    }

    #[test]
    fn set_default_sets_default_and_preserves_unrelated() {
        let mut file = MimeappsFile::parse(SAMPLE);
        file.set_default("text/plain", "foot.desktop");
        let text = file.render();
        assert!(text.contains("text/plain=foot.desktop;"));
        // Existing non-default association is not the default and is untouched.
        assert!(text.contains("text/plain=org.gnome.TextEditor.desktop;"));
        assert!(text.contains("# personal preferences"));
        assert!(text.contains("image/png=org.gnome.eog.desktop"));
        assert!(text.contains("[Other]\nUntouched=yes"));
    }

    #[test]
    fn set_default_creates_sections_and_entries() {
        let mut file = MimeappsFile::default();
        file.set_default("image/png", "imv.desktop");
        assert_eq!(
            file.render(),
            "[Default Applications]\nimage/png=imv.desktop;\n"
        );
    }

    #[test]
    fn add_and_remove_association() {
        let mut file = MimeappsFile::parse(SAMPLE);
        file.add_association("image/png", "imv.desktop");
        assert!(file.render().contains("image/png=imv.desktop;"));

        file.remove_association("image/png", "imv.desktop");
        let settings = file.settings();
        assert!(!settings.added.contains_key("image/png"));
        assert_eq!(
            settings.removed.get("image/png"),
            Some(&vec!["imv.desktop".to_string()])
        );
        // The default for image/png is a different app and stays.
        assert_eq!(
            settings.defaults.get("image/png"),
            Some(&vec!["org.gnome.eog.desktop".to_string()])
        );
    }

    #[test]
    fn reset_removes_key_everywhere() {
        let mut file = MimeappsFile::parse(SAMPLE);
        file.reset("text/plain");
        let text = file.render();
        assert!(!text.contains("text/plain"));
        assert!(text.contains("text/html=helium.desktop"));

        let settings = file.settings();
        assert!(!settings.defaults.contains_key("text/plain"));
    }

    #[test]
    fn save_is_atomic_and_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mimeapps.list");
        let mut file = MimeappsFile::parse(SAMPLE);
        file.set_default("text/plain", "foot.desktop");
        file.save(&path).unwrap();

        let reloaded = MimeappsFile::load(&path).unwrap();
        assert_eq!(
            reloaded.settings().defaults.get("text/plain"),
            Some(&vec!["foot.desktop".to_string()])
        );
        // No temp files left behind.
        let leftovers: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .filter_map(std::result::Result::ok)
            .filter(|e| e.file_name().to_string_lossy().contains(".tmp"))
            .collect();
        assert!(leftovers.is_empty());
    }

    #[test]
    fn merged_associations_prefer_the_first_file() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("config");
        let home = dir.path().join("home");
        fs::create_dir_all(&config).unwrap();

        let target = config.join("mimeapps.list");
        fs::write(&target, "[Default Applications]\ntext/plain=user.desktop\n").unwrap();
        fs::write(
            dir.path().join("mimeapps.list"),
            "[Default Applications]\ntext/plain=system.desktop\n[Added Associations]\ntext/plain=extra.desktop;\n",
        )
        .unwrap();

        let paths = Paths {
            config_home: config,
            data_home: home.join(".local/share"),
            config_dirs: vec![dir.path().to_path_buf()],
            data_dirs: Vec::new(),
        };

        let merged = Associations::load(&paths).unwrap();
        assert_eq!(merged.default_app("text/plain"), Some("user.desktop"));
        assert_eq!(merged.added_for("text/plain"), ["extra.desktop"]);

        let written = apply(
            &paths,
            &AssocEdit::SetDefault {
                mime: "text/plain".into(),
                app: "new.desktop".into(),
            },
        )
        .unwrap();
        assert_eq!(written, target);
        assert!(
            fs::read_to_string(&target)
                .unwrap()
                .contains("text/plain=new.desktop;")
        );
    }
}
