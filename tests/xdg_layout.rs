//! End-to-end check of the core against a simulated XDG layout.
//!
//! Everything runs against temporary directories, so the host's real MIME
//! database and application entries are never touched.

use std::fs;
use std::path::{Path, PathBuf};

use mimo_manager::desktop::{discover, parse_desktop_entry};
use mimo_manager::domain::Source;
use mimo_manager::mime::MimeDatabase;
use mimo_manager::xdg::Paths;

fn write(path: &Path, contents: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

fn layout() -> (tempfile::TempDir, Paths) {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    let system: PathBuf = tmp.path().join("system");

    write(
        &system.join("mime/types"),
        "text/plain\ntext/x-csrc\napplication/pdf\n",
    );
    write(
        &system.join("mime/globs2"),
        "50:text/x-csrc:*.c\n10:application/pdf:*.pdf\n",
    );
    write(&system.join("mime/subclasses"), "text/x-csrc text/plain\n");
    write(
        &system.join("mime/aliases"),
        "application/x-pdf application/pdf\n",
    );

    // The user database overrides the same glob with a higher weight and adds a type.
    write(&home.join(".local/share/mime/types"), "text/x-rust\n");
    write(
        &home.join(".local/share/mime/globs2"),
        "80:text/x-csrc:*.c\n",
    );

    let paths = Paths {
        config_home: home.join(".config"),
        data_home: home.join(".local/share"),
        config_dirs: Vec::new(),
        data_dirs: vec![system],
    };
    (tmp, paths)
}

#[test]
fn loads_and_merges_the_mime_database() {
    let (_tmp, paths) = layout();
    let db = MimeDatabase::load(&paths).unwrap();

    // Union of both directories.
    assert!(db.get("text/x-rust").is_some());
    assert!(db.get("application/pdf").is_some());

    // User glob (weight 80) wins over the system glob (weight 50) for `*.c`.
    let csrc = db.get("text/x-csrc").unwrap();
    assert_eq!(csrc.extensions, vec!["c"]);

    // Transitive ancestry from `subclasses`.
    assert_eq!(db.ancestors("text/x-csrc"), vec!["text/plain".to_string()]);

    // Alias resolution.
    assert_eq!(
        db.resolve_alias("application/x-pdf"),
        Some("application/pdf")
    );

    // A pattern with no literal extension yields no extension.
    assert!(db.get("application/pdf").unwrap().extensions == vec!["pdf"]);
    assert_eq!(
        db.get("text/plain").unwrap().extensions,
        Vec::<String>::new()
    );
}

#[test]
fn discovers_entries_with_precedence() {
    let (_tmp, paths) = layout();
    write(
        &paths.data_home.join("applications/dup.desktop"),
        "[Desktop Entry]\nType=Application\nName=User Editor\nExec=sh -c true\nMimeType=text/plain;\n",
    );
    write(
        &paths.data_dirs[0].join("applications/dup.desktop"),
        "[Desktop Entry]\nType=Application\nName=System Editor\nExec=sh -c true\n",
    );

    let report = discover(&paths, "/bin:/usr/bin");
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
    assert_eq!(report.apps.len(), 1);
    assert_eq!(report.apps[0].name, "User Editor");
    assert_eq!(report.apps[0].source, Source::User);
    assert_eq!(report.apps[0].mime_types, vec!["text/plain"]);
}

#[test]
fn parses_a_realistic_entry() {
    let text = "\
[Desktop Entry]
Type=Application
Name=Files
Exec=nautilus %U
TryExec=nautilus
Icon=org.gnome.Nautilus
Terminal=false
MimeType=inode/directory;
";
    let app = parse_desktop_entry(
        "org.gnome.Nautilus.desktop",
        Path::new("/tmp/x.desktop"),
        text,
        Source::System,
        "/bin:/usr/bin",
    );
    assert_eq!(app.id, "org.gnome.Nautilus.desktop");
    assert_eq!(app.name, "Files");
    assert!(app.is_valid());
}
