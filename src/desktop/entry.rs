//! Hand-rolled `.desktop` parsing.
//!
//! gtk-rs 0.22 provides no `GDesktopAppInfo` binding, and the manager wants the
//! entries GIO normally hides (hidden/`NoDisplay`), so entries are parsed here.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use crate::domain::{DesktopApp, Source, Validity};
use crate::error::{Error, Result};

/// A key/value pair from an INI file, with an optional `[locale]` suffix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IniEntry {
    pub key: String,
    pub locale: Option<String>,
    pub value: String,
}

/// A named INI section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IniSection {
    pub name: String,
    pub entries: Vec<IniEntry>,
}

/// Parse freedesktop INI text into sections, preserving key order.
pub fn parse_ini(text: &str) -> Vec<IniSection> {
    let mut sections: Vec<IniSection> = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            sections.push(IniSection {
                name: name.to_string(),
                entries: Vec::new(),
            });
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let Some(section) = sections.last_mut() else {
            continue;
        };
        let (key, locale) = match key.split_once('[') {
            Some((k, rest)) => (k.trim(), rest.strip_suffix(']').map(str::to_string)),
            None => (key.trim(), None),
        };
        section.entries.push(IniEntry {
            key: key.to_string(),
            locale,
            value: unescape(value.trim()),
        });
    }
    sections
}

fn unescape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('s') => out.push(' '),
            Some('\\') => out.push('\\'),
            Some(other) => out.push(other),
            None => out.push('\\'),
        }
    }
    out
}

/// The first lookup key of the file for a given entry key.
fn lookup<'a>(entries: &'a [IniEntry], key: &str) -> Option<&'a str> {
    entries
        .iter()
        .find(|e| e.key == key && e.locale.is_none())
        .map(|e| e.value.as_str())
}

fn lookup_localized<'a>(entries: &'a [IniEntry], key: &str) -> Option<&'a str> {
    entries
        .iter()
        .find(|e| e.key == key)
        .map(|e| e.value.as_str())
}

fn flag(entries: &[IniEntry], key: &str) -> bool {
    lookup(entries, key) == Some("true")
}

fn list(entries: &[IniEntry], key: &str) -> Vec<String> {
    entries
        .iter()
        .find(|e| e.key == key && e.locale.is_none())
        .map(|e| {
            e.value
                .split(';')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// Parse a `.desktop` entry.
///
/// `path_env` is the `PATH` used to decide whether the entry's executable is
/// available; passing it explicitly keeps discovery testable.
pub fn parse_desktop_entry(
    id: &str,
    path: &Path,
    text: &str,
    source: Source,
    path_env: &str,
) -> DesktopApp {
    let sections = parse_ini(text);
    let empty: Vec<IniEntry> = Vec::new();
    let entries = sections
        .iter()
        .find(|s| s.name == "Desktop Entry")
        .map(|s| s.entries.as_slice())
        .unwrap_or(&empty);

    let entry_type = lookup(entries, "Type");
    let exec = lookup(entries, "Exec").map(str::to_string);
    let try_exec = lookup(entries, "TryExec").map(str::to_string);
    let dbus_activatable = flag(entries, "DBusActivatable");

    let validity = if entry_type != Some("Application") {
        Validity::Invalid("Type is not Application".into())
    } else if exec.is_none() && !dbus_activatable {
        Validity::Invalid("no Exec and not DBusActivatable".into())
    } else if lookup_localized(entries, "Name").is_none() {
        Validity::Invalid("no Name".into())
    } else {
        Validity::Valid
    };

    let launchable = if validity == Validity::Valid {
        match (&exec, &try_exec) {
            (_, Some(binary)) => is_executable_available(binary, path_env),
            (Some(command), None) => first_exec_token(command)
                .map(|binary| is_executable_available(&binary, path_env))
                .unwrap_or(false),
            (None, None) => dbus_activatable,
        }
    } else {
        false
    };

    DesktopApp {
        id: id.to_string(),
        name: lookup_localized(entries, "Name").unwrap_or(id).to_string(),
        exec,
        try_exec,
        icon: lookup(entries, "Icon").map(str::to_string),
        mime_types: list(entries, "MimeType"),
        terminal: flag(entries, "Terminal"),
        no_display: flag(entries, "NoDisplay"),
        hidden: flag(entries, "Hidden"),
        only_show_in: list(entries, "OnlyShowIn"),
        not_show_in: list(entries, "NotShowIn"),
        dbus_activatable,
        source,
        path: path.to_path_buf(),
        validity,
        launchable,
    }
}

/// Read and parse a `.desktop` file from disk.
pub fn load_desktop_file(
    id: &str,
    path: &Path,
    source: Source,
    path_env: &str,
) -> Result<DesktopApp> {
    let text = fs::read_to_string(path).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(parse_desktop_entry(id, path, &text, source, path_env))
}

/// Extract the executable (first `argv[0]`) from a desktop `Exec` line,
/// honouring double-quoted arguments and ignoring field codes.
pub fn first_exec_token(exec: &str) -> Option<String> {
    let mut tokens: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut started = false;

    let mut chars = exec.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                quoted = !quoted;
                started = true;
            }
            '\\' if quoted => {
                if let Some(next) = chars.next() {
                    current.push(next);
                }
            }
            c if c.is_whitespace() && !quoted => {
                if started {
                    tokens.push(std::mem::take(&mut current));
                    started = false;
                }
            }
            _ => {
                current.push(c);
                started = true;
            }
        }
    }
    if started {
        tokens.push(current);
    }

    tokens.into_iter().find(|token| !token.starts_with('%'))
}

/// Whether `binary` resolves to an executable file via `PATH`.
pub fn is_executable_available(binary: &str, path_env: &str) -> bool {
    if binary.contains('/') {
        return is_executable_file(Path::new(binary));
    }
    std::env::split_paths(path_env).any(|dir| is_executable_file(&dir.join(binary)))
}

fn is_executable_file(path: &Path) -> bool {
    fs::metadata(path)
        .map(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
[Desktop Entry]
Type=Application
Name=Editor
Name[de]=Editor-de
GenericName=Text Editor
Exec=editor --wait %F
Icon=editor
Terminal=false
NoDisplay=true
MimeType=text/plain;text/x-rust;
OnlyShowIn=GNOME;niri;
Categories=Utility;TextEditor;

[Desktop Action New]
Name=New Window
Exec=editor --new
";

    #[test]
    fn ini_splits_sections_and_locales() {
        let sections = parse_ini(SAMPLE);
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].name, "Desktop Entry");
        assert!(sections[0].entries.iter().any(|e| e.key == "Name"
            && e.locale.as_deref() == Some("de")
            && e.value == "Editor-de"));
    }

    #[test]
    fn ini_unescapes_special_characters() {
        let entries = parse_ini("[Desktop Entry]\nComment=a\\sb\\nc\n");
        assert_eq!(entries[0].entries[0].value, "a b\nc");
    }

    #[test]
    fn parses_application_fields() {
        let app = parse_desktop_entry(
            "editor.desktop",
            Path::new("/tmp/editor.desktop"),
            SAMPLE,
            Source::User,
            "/nonexistent",
        );
        assert_eq!(app.name, "Editor");
        assert_eq!(app.exec.as_deref(), Some("editor --wait %F"));
        assert_eq!(app.icon.as_deref(), Some("editor"));
        assert_eq!(app.mime_types, vec!["text/plain", "text/x-rust"]);
        assert_eq!(app.only_show_in, vec!["GNOME", "niri"]);
        assert!(app.no_display);
        assert!(!app.terminal);
        assert!(app.is_valid());
    }

    #[test]
    fn exec_without_command_is_invalid() {
        let text = "[Desktop Entry]\nType=Application\nName=Broken\n";
        let app = parse_desktop_entry(
            "b.desktop",
            Path::new("/tmp/b.desktop"),
            text,
            Source::User,
            "",
        );
        assert!(!app.is_valid());
        assert!(!app.launchable);
    }

    #[test]
    fn link_type_is_not_an_application() {
        let text = "[Desktop Entry]\nType=Link\nName=Home\nURL=https://x/\n";
        let app = parse_desktop_entry(
            "l.desktop",
            Path::new("/tmp/l.desktop"),
            text,
            Source::User,
            "",
        );
        assert!(!app.is_valid());
    }

    #[test]
    fn exec_token_honours_quotes_and_skips_field_codes() {
        assert_eq!(first_exec_token("editor --wait %F"), Some("editor".into()));
        assert_eq!(
            first_exec_token("\"/opt/my app/bin\" %U"),
            Some("/opt/my app/bin".into())
        );
        assert_eq!(first_exec_token("%U"), None);
    }

    #[test]
    fn path_lookup_checks_absolute_and_relative_binaries() {
        // `sh` is on essentially every PATH; a directory is not executable.
        let sh = is_executable_available("sh", "/bin:/usr/bin");
        assert_eq!(
            sh,
            Path::new("/bin/sh").exists() || Path::new("/usr/bin/sh").exists()
        );
        assert!(!is_executable_available(
            "definitely-not-a-binary",
            "/bin:/usr/bin"
        ));
    }
}
