use std::path::PathBuf;

/// Where a desktop entry was discovered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    User,
    System,
}

/// Whether a `.desktop` file can be used as an application for a MIME type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Validity {
    Valid,
    Invalid(String),
}

/// A parsed `.desktop` entry.
///
/// Entries that are hidden or `NoDisplay` are kept: the manager lists every
/// entry it can find and surfaces their state as tags.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopApp {
    /// Desktop-file id: the path under `applications/` with `/` replaced by `-`.
    pub id: String,
    pub name: String,
    pub exec: Option<String>,
    pub try_exec: Option<String>,
    pub icon: Option<String>,
    pub mime_types: Vec<String>,
    pub terminal: bool,
    pub no_display: bool,
    pub hidden: bool,
    pub only_show_in: Vec<String>,
    pub not_show_in: Vec<String>,
    pub dbus_activatable: bool,
    pub source: Source,
    pub path: PathBuf,
    pub validity: Validity,
    /// Whether the executable referenced by `Exec`/`TryExec` is available.
    pub launchable: bool,
}

impl DesktopApp {
    pub fn is_valid(&self) -> bool {
        self.validity == Validity::Valid
    }

    /// Whether this entry is offered in the given desktop environments.
    ///
    /// `current` holds the `XDG_CURRENT_DESKTOP` names (e.g. `["niri"]`),
    /// already lowercased and split.
    pub fn shows_in(&self, current: &[&str]) -> bool {
        if self
            .not_show_in
            .iter()
            .any(|d| current.iter().any(|c| d.eq_ignore_ascii_case(c)))
        {
            return false;
        }
        self.only_show_in.is_empty()
            || self
                .only_show_in
                .iter()
                .any(|d| current.iter().any(|c| d.eq_ignore_ascii_case(c)))
    }

    /// Short state labels for the modeline/list.
    pub fn tags(&self) -> Vec<&'static str> {
        let mut tags = Vec::new();
        match &self.validity {
            Validity::Valid => {}
            Validity::Invalid(_) => tags.push("invalid"),
        }
        if self.hidden {
            tags.push("hidden");
        }
        if self.no_display {
            tags.push("nodisplay");
        }
        if self.is_valid() && !self.launchable {
            tags.push("no-exec");
        }
        tags
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> DesktopApp {
        DesktopApp {
            id: "a.desktop".into(),
            name: "A".into(),
            exec: Some("a %U".into()),
            try_exec: None,
            icon: None,
            mime_types: Vec::new(),
            terminal: false,
            no_display: false,
            hidden: false,
            only_show_in: Vec::new(),
            not_show_in: Vec::new(),
            dbus_activatable: false,
            source: Source::User,
            path: PathBuf::from("/tmp/a.desktop"),
            validity: Validity::Valid,
            launchable: true,
        }
    }

    #[test]
    fn shows_in_respects_only_and_not() {
        assert!(app().shows_in(&["niri"]));

        let mut only = app();
        only.only_show_in = vec!["GNOME".into()];
        assert!(!only.shows_in(&["niri"]));
        assert!(only.shows_in(&["gnome"]));

        let mut not = app();
        not.not_show_in = vec!["niri".into()];
        assert!(!not.shows_in(&["Niri"]));
        assert!(not.shows_in(&["sway"]));
    }

    #[test]
    fn tags_describe_entry_state() {
        assert!(app().tags().is_empty());

        let mut a = app();
        a.no_display = true;
        a.launchable = false;
        assert_eq!(a.tags(), vec!["nodisplay", "no-exec"]);

        let mut b = app();
        b.validity = Validity::Invalid("no Exec".into());
        b.hidden = true;
        assert_eq!(b.tags(), vec!["invalid", "hidden"]);
    }
}
