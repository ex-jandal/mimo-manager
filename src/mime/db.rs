//! Enumeration of the local MIME database.
//!
//! GLib/GIO expose rich *queries*, but not the raw type list; the manager reads
//! the freedesktop shared-mime-info files (`types`, `globs2`, `subclasses`,
//! `aliases`) directly. Directories are visited lowest priority first so that a
//! user override silently wins over the system definition.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

use crate::domain::MimeType;
use crate::error::{Error, Result};
use crate::xdg::Paths;

#[derive(Debug, Clone, Default)]
pub struct MimeDatabase {
    types: BTreeMap<String, MimeType>,
}

impl MimeDatabase {
    /// Load every MIME database reachable from `paths`.
    ///
    /// Missing files are not errors; unreadable files are. Malformed lines are
    /// skipped so a single bad line cannot take down the whole database.
    pub fn load(paths: &Paths) -> Result<Self> {
        let mut names: BTreeSet<String> = BTreeSet::new();
        // pattern -> (weight, mime); highest weight wins, later dir breaks ties.
        let mut globs: HashMap<String, (i32, String)> = HashMap::new();
        let mut parents: HashMap<String, BTreeSet<String>> = HashMap::new();
        let mut aliases: HashMap<String, String> = HashMap::new();

        for dir in paths.mime_dirs().iter().rev() {
            for name in read_lines(&dir.join("types"), parse_types)? {
                names.insert(name);
            }
            for glob in read_lines(&dir.join("globs2"), parse_globs2)? {
                names.insert(glob.mime.clone());
                match globs.get(&glob.pattern) {
                    Some((weight, _)) if *weight > glob.weight => {}
                    _ => {
                        globs.insert(glob.pattern, (glob.weight, glob.mime));
                    }
                }
            }
            for (child, parent) in read_lines(&dir.join("subclasses"), parse_subclasses)? {
                names.insert(child.clone());
                names.insert(parent.clone());
                parents.entry(child).or_default().insert(parent);
            }
            for (alias, target) in read_lines(&dir.join("aliases"), parse_aliases)? {
                names.insert(alias.clone());
                names.insert(target.clone());
                aliases.insert(alias, target);
            }
        }

        let mut extensions: HashMap<String, BTreeSet<String>> = HashMap::new();
        for (pattern, (_, mime)) in &globs {
            if let Some(ext) = extension_from_glob(pattern) {
                extensions.entry(mime.clone()).or_default().insert(ext);
            }
        }

        let mut alias_names: HashMap<String, BTreeSet<String>> = HashMap::new();
        for (alias, target) in &aliases {
            alias_names
                .entry(target.clone())
                .or_default()
                .insert(alias.clone());
        }

        let mut types = BTreeMap::new();
        for name in names {
            let mut mime = MimeType::new(name.clone());
            if let Some(exts) = extensions.get(&name) {
                mime.extensions = exts.iter().cloned().collect();
            }
            if let Some(parent_set) = parents.get(&name) {
                mime.parents = parent_set.iter().cloned().collect();
            }
            if let Some(alias_set) = alias_names.get(&name) {
                mime.aliases = alias_set.iter().cloned().collect();
            }
            types.insert(name, mime);
        }

        Ok(Self { types })
    }

    pub fn get(&self, name: &str) -> Option<&MimeType> {
        self.types.get(name)
    }

    pub fn all(&self) -> impl Iterator<Item = &MimeType> {
        self.types.values()
    }

    pub fn len(&self) -> usize {
        self.types.len()
    }

    pub fn is_empty(&self) -> bool {
        self.types.is_empty()
    }

    /// All superclasses of `name`, transitively, excluding `name` itself.
    pub fn ancestors(&self, name: &str) -> Vec<String> {
        let mut result = BTreeSet::new();
        let mut queue: Vec<String> = self
            .get(name)
            .map(|m| m.parents.clone())
            .unwrap_or_default();
        while let Some(parent) = queue.pop() {
            if result.insert(parent.clone())
                && let Some(mime) = self.get(&parent)
            {
                queue.extend(mime.parents.iter().cloned());
            }
        }
        result.into_iter().collect()
    }

    /// All types that declare `name` among their parents, transitively.
    pub fn descendants(&self, name: &str) -> Vec<String> {
        let mut result = BTreeSet::new();
        let mut queue = vec![name.to_string()];
        while let Some(current) = queue.pop() {
            for mime in self.types.values() {
                if mime.parents.iter().any(|p| p == &current) && result.insert(mime.name.clone()) {
                    queue.push(mime.name.clone());
                }
            }
        }
        result.into_iter().collect()
    }

    /// Resolve an alias to its canonical type, if one is defined.
    pub fn resolve_alias(&self, name: &str) -> Option<&str> {
        self.types
            .values()
            .find(|m| m.aliases.iter().any(|a| a == name))
            .map(|m| m.name.as_str())
    }
}

fn read_lines<T>(path: &Path, parse: impl FnOnce(&str) -> Vec<T>) -> Result<Vec<T>> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(parse(&text)),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(source) => Err(Error::Io {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn is_comment(line: &str) -> bool {
    line.is_empty() || line.starts_with('#')
}

pub(crate) fn parse_types(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !is_comment(line))
        .map(str::to_string)
        .collect()
}

/// One parsed `globs2` line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Glob {
    pub weight: i32,
    pub mime: String,
    pub pattern: String,
}

pub(crate) fn parse_globs2(text: &str) -> Vec<Glob> {
    text.lines()
        .map(str::trim)
        .filter(|line| !is_comment(line))
        .filter_map(|line| {
            let mut parts = line.splitn(3, ':');
            let weight = parts.next()?.parse().ok()?;
            let mime = parts.next()?;
            let rest = parts.next()?;
            let pattern = strip_glob_flags(rest)?;
            if mime.is_empty() || pattern.is_empty() {
                return None;
            }
            Some(Glob {
                weight,
                mime: mime.to_string(),
                pattern,
            })
        })
        .collect()
}

/// Drop the trailing `:flags` field of a glob, preserving any `:` in the pattern.
fn strip_glob_flags(rest: &str) -> Option<String> {
    match rest.rsplit_once(':') {
        Some((pattern, flags)) if looks_like_flags(flags) => Some(pattern.to_string()),
        _ => Some(rest.to_string()),
    }
}

fn looks_like_flags(flags: &str) -> bool {
    !flags.is_empty() && flags.len() <= 4 && flags.chars().all(|c| c.is_ascii_lowercase())
}

pub(crate) fn parse_subclasses(text: &str) -> Vec<(String, String)> {
    text.lines()
        .map(str::trim)
        .filter(|line| !is_comment(line))
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let child = parts.next()?;
            let parent = parts.next()?;
            Some((child.to_string(), parent.to_string()))
        })
        .collect()
}

pub(crate) fn parse_aliases(text: &str) -> Vec<(String, String)> {
    parse_subclasses(text)
}

/// Derive a plain extension from a glob pattern.
///
/// Only simple `*.ext` patterns are considered; character classes, numeric
/// ranges and multi-segment patterns without a literal extension yield `None`.
pub(crate) fn extension_from_glob(pattern: &str) -> Option<String> {
    let ext = pattern.strip_prefix("*.")?;
    if ext.is_empty() || !ext.chars().all(is_extension_char) {
        return None;
    }
    Some(ext.to_ascii_lowercase())
}

fn is_extension_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '+' | '-')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn types_skips_blanks_and_comments() {
        let text = "text/plain\n\n# comment\nimage/png\n";
        assert_eq!(parse_types(text), vec!["text/plain", "image/png"]);
    }

    #[test]
    fn globs2_parses_weight_mime_pattern() {
        let globs = parse_globs2("50:text/x-csrc:*.c\n10:image/png:*.png\n");
        assert_eq!(
            globs,
            vec![
                Glob {
                    weight: 50,
                    mime: "text/x-csrc".into(),
                    pattern: "*.c".into()
                },
                Glob {
                    weight: 10,
                    mime: "image/png".into(),
                    pattern: "*.png".into()
                },
            ]
        );
    }

    #[test]
    fn globs2_strips_flags_and_skips_malformed() {
        let globs = parse_globs2("50:text/x-csrc:*.c:cs\nnot-a-weight:text/plain:*.txt\n");
        assert_eq!(globs.len(), 1);
        assert_eq!(globs[0].pattern, "*.c");
    }

    #[test]
    fn subclasses_and_aliases_parse_pairs() {
        assert_eq!(
            parse_subclasses("text/x-c++ text/plain\n"),
            vec![("text/x-c++".into(), "text/plain".into())]
        );
        assert_eq!(
            parse_aliases("application/x-pdf application/pdf\n"),
            vec![("application/x-pdf".into(), "application/pdf".into())]
        );
    }

    #[test]
    fn extension_rules() {
        assert_eq!(extension_from_glob("*.c"), Some("c".into()));
        assert_eq!(extension_from_glob("*.tar.gz"), Some("tar.gz".into()));
        assert_eq!(extension_from_glob("*.C"), Some("c".into()));
        assert_eq!(extension_from_glob("Makefile"), None);
        assert_eq!(extension_from_glob("*.so.[0-9]*"), None);
        assert_eq!(extension_from_glob("core"), None);
    }

    #[test]
    fn ancestors_are_transitive_and_deduped() {
        let text_types = "\
audio/mpeg
audio/x-mp3
media
application/ogg
";
        let subclasses = "audio/mpeg audio/x-mp3\naudio/x-mp3 media\naudio/mpeg media\n";
        let mut names: BTreeSet<String> = BTreeSet::new();
        for name in parse_types(text_types) {
            names.insert(name);
        }
        let mut parents: HashMap<String, BTreeSet<String>> = HashMap::new();
        for (child, parent) in parse_subclasses(subclasses) {
            names.insert(child.clone());
            names.insert(parent.clone());
            parents.entry(child).or_default().insert(parent);
        }
        let types = names
            .into_iter()
            .map(|name| {
                let mut mime = MimeType::new(name.clone());
                if let Some(set) = parents.get(&name) {
                    mime.parents = set.iter().cloned().collect();
                }
                (name, mime)
            })
            .collect();
        let db = MimeDatabase { types };

        assert_eq!(
            db.ancestors("audio/mpeg"),
            vec!["audio/x-mp3".to_string(), "media".to_string()]
                .into_iter()
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>()
        );
        assert!(db.descendants("media").contains(&"audio/mpeg".to_string()));
    }
}
