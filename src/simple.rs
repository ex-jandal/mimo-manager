//! Simple mode: curated defaults for common file kinds and desktop roles.
//!
//! The SIMPLE workspace offers one app picker per entry. A *role* (terminal,
//! file manager, …) maps to an explicit set of MIME keys; a *category*
//! (Audio, Video, …) matches by MIME prefix or exact name. Committing an app
//! sets it as the default for every member of the entry.

use crate::mime::MimeDatabase;

/// One selectable row in the SIMPLE workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SimpleEntry {
    pub label: &'static str,
    /// MIME type whose current default represents the entry in the modeline.
    pub probe: &'static str,
    /// Explicit member MIME keys.
    pub exact: &'static [&'static str],
    /// MIME name prefix matched as a category (`""` for none).
    pub prefix: &'static str,
    /// A desktop role rather than a file category.
    pub role: bool,
}

impl SimpleEntry {
    /// Whether `mime` belongs to this entry.
    pub fn matches(&self, mime: &str) -> bool {
        self.exact.iter().any(|m| m.eq_ignore_ascii_case(mime))
            || (!self.prefix.is_empty() && mime.starts_with(self.prefix))
    }

    /// The concrete MIME keys this entry would set a default for.
    ///
    /// Categories expand to file-typed members (types with at least one
    /// extension), so a category change does not sprawl into abstract types.
    pub fn members(&self, db: &MimeDatabase) -> Vec<String> {
        let mut members: Vec<String> = self.exact.iter().map(|m| (*m).to_string()).collect();
        if !self.role && !self.prefix.is_empty() {
            for mime in db.all() {
                if !mime.extensions.is_empty()
                    && mime.name.starts_with(self.prefix)
                    && !members.iter().any(|m| m == &mime.name)
                {
                    members.push(mime.name.clone());
                }
            }
        }
        members
    }
}

/// Every entry, roles first.
pub static ENTRIES: &[SimpleEntry] = &[
    SimpleEntry {
        label: "File manager",
        probe: "inode/directory",
        exact: &["inode/directory"],
        prefix: "",
        role: true,
    },
    SimpleEntry {
        label: "Terminal",
        probe: "x-scheme-handler/terminal",
        exact: &["x-scheme-handler/terminal"],
        prefix: "",
        role: true,
    },
    SimpleEntry {
        label: "Web browser",
        probe: "x-scheme-handler/http",
        exact: &[
            "x-scheme-handler/http",
            "x-scheme-handler/https",
            "text/html",
        ],
        prefix: "",
        role: true,
    },
    SimpleEntry {
        label: "Email client",
        probe: "x-scheme-handler/mailto",
        exact: &["x-scheme-handler/mailto"],
        prefix: "",
        role: true,
    },
    SimpleEntry {
        label: "Text editor",
        probe: "text/plain",
        exact: &["text/plain"],
        prefix: "",
        role: true,
    },
    SimpleEntry {
        label: "Audio",
        probe: "audio/mpeg",
        exact: &[],
        prefix: "audio/",
        role: false,
    },
    SimpleEntry {
        label: "Video",
        probe: "video/mp4",
        exact: &[],
        prefix: "video/",
        role: false,
    },
    SimpleEntry {
        label: "Images",
        probe: "image/png",
        exact: &[],
        prefix: "image/",
        role: false,
    },
    SimpleEntry {
        label: "Text",
        probe: "text/plain",
        exact: &[],
        prefix: "text/",
        role: false,
    },
    SimpleEntry {
        label: "Fonts",
        probe: "font/ttf",
        exact: &[],
        prefix: "font/",
        role: false,
    },
    SimpleEntry {
        label: "PDF",
        probe: "application/pdf",
        exact: &["application/pdf"],
        prefix: "",
        role: false,
    },
    SimpleEntry {
        label: "Archives",
        probe: "application/zip",
        exact: &[
            "application/zip",
            "application/x-tar",
            "application/gzip",
            "application/x-gzip",
            "application/x-7z-compressed",
            "application/vnd.rar",
            "application/x-rar",
            "application/x-rar-compressed",
            "application/x-bzip2",
            "application/x-xz",
            "application/zstd",
            "application/x-compressed-tar",
        ],
        prefix: "",
        role: false,
    },
    SimpleEntry {
        label: "Documents",
        probe: "application/vnd.oasis.opendocument.text",
        exact: &[
            "application/vnd.oasis.opendocument.text",
            "application/vnd.oasis.opendocument.spreadsheet",
            "application/vnd.oasis.opendocument.presentation",
            "application/msword",
            "application/vnd.ms-excel",
            "application/vnd.ms-powerpoint",
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            "application/vnd.openxmlformats-officedocument.presentationml.presentation",
            "application/rtf",
            "application/epub+zip",
        ],
        prefix: "",
        role: false,
    },
];

/// The entries to show, roles first.
pub fn entries() -> &'static [SimpleEntry] {
    ENTRIES
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> MimeDatabase {
        let dir = tempfile::tempdir().unwrap();
        let paths = crate::xdg::Paths::rooted(dir.path().join("home"));
        let mime = paths.data_home.join("mime");
        std::fs::create_dir_all(&mime).unwrap();
        std::fs::write(
            mime.join("types"),
            "audio/mpeg\naudio/x-wav\nimage/png\nimage/jpeg\napplication/pdf\napplication/zip\n",
        )
        .unwrap();
        std::fs::write(
            mime.join("globs2"),
            "50:audio/mpeg:*.mp3\n50:audio/x-wav:*.wav\n50:image/png:*.png\n50:application/pdf:*.pdf\n50:application/zip:*.zip\n",
        )
        .unwrap();
        std::fs::write(mime.join("subclasses"), "audio/mpeg audio/\n").unwrap();
        MimeDatabase::load(&paths).unwrap()
    }

    fn entry(label: &str) -> &'static SimpleEntry {
        entries().iter().find(|e| e.label == label).unwrap()
    }

    #[test]
    fn roles_use_their_explicit_keys() {
        assert_eq!(
            entry("File manager").members(&MimeDatabase::default()),
            ["inode/directory"]
        );
        assert_eq!(
            entry("Web browser").members(&MimeDatabase::default()),
            [
                "x-scheme-handler/http",
                "x-scheme-handler/https",
                "text/html"
            ]
        );
    }

    #[test]
    fn roles_come_first() {
        assert!(entries()[0].role);
        assert!(entries().iter().any(|e| !e.role));
    }

    #[test]
    fn categories_expand_to_file_typed_members() {
        let members = entry("Audio").members(&db());
        assert_eq!(members, ["audio/mpeg", "audio/x-wav"]);
    }

    #[test]
    fn categories_match_by_prefix_and_exact() {
        let audio = entry("Audio");
        assert!(audio.matches("audio/ogg") && !audio.matches("video/mp4"));

        let pdf = entry("PDF");
        assert!(pdf.matches("application/pdf") && !pdf.matches("application/postscript"));
    }

    #[test]
    fn probes_are_concrete_mime_types() {
        for e in entries() {
            assert!(e.probe.contains('/'), "{} probe looks wrong", e.label);
        }
    }
}
