//! Discovery of `.desktop` entries across the XDG application directories.

use std::collections::HashSet;
use std::fs;
use std::path::Path;

use crate::desktop::entry::load_desktop_file;
use crate::domain::{DesktopApp, Source};
use crate::xdg::Paths;

/// Result of scanning the application directories.
#[derive(Debug, Clone, Default)]
pub struct DiscoveryReport {
    pub apps: Vec<DesktopApp>,
    /// Non-fatal problems (unreadable files, etc.) for diagnostics.
    pub warnings: Vec<String>,
}

/// Upper bound on directory nesting below `applications/`.
const MAX_DEPTH: usize = 3;

/// Discover every `.desktop` entry below the XDG application directories.
///
/// Directories are visited highest priority first and the first id wins, so a
/// user entry shadows the identically named system entry. `path_env` is used to
/// determine launchability.
pub fn discover(paths: &Paths, path_env: &str) -> DiscoveryReport {
    let mut report = DiscoveryReport::default();
    let mut seen: HashSet<String> = HashSet::new();

    for dir in paths.applications_dirs() {
        let source = if dir.starts_with(&paths.data_home) {
            Source::User
        } else {
            Source::System
        };
        walk(&dir, &dir, 0, &mut |full, rel| {
            let id = desktop_id(rel);
            if !seen.insert(id.clone()) {
                return;
            }
            match load_desktop_file(&id, full, source, path_env) {
                Ok(app) => report.apps.push(app),
                Err(error) => report.warnings.push(error.to_string()),
            }
        });
    }

    report.apps.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.id.cmp(&b.id))
    });
    report
}

fn walk(dir: &Path, root: &Path, depth: usize, visit: &mut impl FnMut(&Path, &Path)) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if name.starts_with('.') {
            continue;
        }
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            if depth < MAX_DEPTH {
                walk(&path, root, depth + 1, visit);
            }
        } else if path.extension().and_then(|e| e.to_str()) == Some("desktop")
            && let Ok(rel) = path.strip_prefix(root)
        {
            visit(&path, rel);
        }
    }
}

/// Build the desktop-file id from a path relative to `applications/`.
fn desktop_id(relative: &Path) -> String {
    relative
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("-")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write(path: &Path, contents: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let mut file = fs::File::create(path).unwrap();
        file.write_all(contents.as_bytes()).unwrap();
    }

    const ENTRY: &str = "[Desktop Entry]\nType=Application\nName=%NAME%\nExec=sh -c true\n";

    #[test]
    fn desktop_id_replaces_separators() {
        assert_eq!(desktop_id(Path::new("kde/foo.desktop")), "kde-foo.desktop");
        assert_eq!(desktop_id(Path::new("foo.desktop")), "foo.desktop");
    }

    #[test]
    fn discovery_prefers_first_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        let system = tmp.path().join("sys/share");

        let mut paths = Paths::rooted(&home);
        paths.data_dirs = vec![system.clone()];

        write(
            &home.join(".local/share/applications/dup.desktop"),
            &ENTRY.replace("%NAME%", "User"),
        );
        write(
            &system.join("applications/dup.desktop"),
            &ENTRY.replace("%NAME%", "System"),
        );
        write(
            &system.join("applications/nested/other.desktop"),
            &ENTRY.replace("%NAME%", "Other"),
        );

        let report = discover(&paths, "/bin:/usr/bin");
        assert!(report.warnings.is_empty());
        let names: Vec<_> = report.apps.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, vec!["Other", "User"]);
        assert!(report.apps.iter().any(|a| a.id == "nested-other.desktop"));
        assert!(report.apps.iter().all(|a| a.name != "System"));
    }

    #[test]
    fn discovery_skips_hidden_entries() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        let paths = Paths::rooted(&home);
        write(
            &home.join(".local/share/applications/.hidden.desktop"),
            &ENTRY.replace("%NAME%", "Hidden"),
        );
        write(
            &home.join(".local/share/applications/visible.desktop"),
            &ENTRY.replace("%NAME%", "Visible"),
        );
        let report = discover(&paths, "/bin:/usr/bin");
        assert_eq!(report.apps.len(), 1);
        assert_eq!(report.apps[0].name, "Visible");
    }
}
