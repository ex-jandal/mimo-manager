//! The command and key reference.
//!
//! One table drives command-mode autocomplete and the HELP workspace, so the
//! documented commands and the completable ones can never drift. A test keeps
//! this table and the parser in sync.

/// A `:` command, for completion and help.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandSpec {
    /// The canonical name, i.e. exactly what the parser accepts.
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    /// Argument placeholder for the usage line (`""` when argument-less).
    pub args: &'static str,
    pub help: &'static str,
}

/// Every command the interface understands.
pub static COMMANDS: &[CommandSpec] = &[
    CommandSpec {
        name: "set-default",
        aliases: &["set", "default"],
        args: "<app>",
        help: "Make <app> the default handler for the selected MIME type.",
    },
    CommandSpec {
        name: "add",
        aliases: &[],
        args: "<app>",
        help: "Add <app> as a non-default handler for the selected MIME type.",
    },
    CommandSpec {
        name: "remove",
        aliases: &["rm"],
        args: "<app>",
        help: "Remove the association with <app>.",
    },
    CommandSpec {
        name: "reset",
        aliases: &[],
        args: "",
        help: "Drop local overrides, reverting to the inherited defaults.",
    },
    CommandSpec {
        name: "filter",
        aliases: &["f"],
        args: "<text>",
        help: "Filter the list.",
    },
    CommandSpec {
        name: "ws",
        aliases: &["workspace"],
        args: "<name>",
        help: "Switch workspace (mime, apps, desktop, config, simple, help).",
    },
    CommandSpec {
        name: "open-config",
        aliases: &["config"],
        args: "",
        help: "Open the config file in $EDITOR.",
    },
    CommandSpec {
        name: "theme",
        aliases: &[],
        args: "<name>",
        help: "Switch theme (MELANGE DARK, GRUVBOX DARK, NORD, CATPPUCCIN MOCHA, TOKYO NIGHT).",
    },
    CommandSpec {
        name: "help",
        aliases: &["h", "?"],
        args: "",
        help: "Show the help workspace.",
    },
    CommandSpec {
        name: "quit",
        aliases: &["q"],
        args: "",
        help: "Quit.",
    },
];

/// A key binding, for the help workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeySpec {
    pub keys: &'static str,
    pub help: &'static str,
}

/// Normal-mode key bindings.
pub static KEYS: &[KeySpec] = &[
    KeySpec {
        keys: "j / Down",
        help: "Move down",
    },
    KeySpec {
        keys: "k / Up",
        help: "Move up",
    },
    KeySpec {
        keys: "gg",
        help: "Jump to the top",
    },
    KeySpec {
        keys: "G",
        help: "Jump to the bottom",
    },
    KeySpec {
        keys: "Ctrl+d / Ctrl+u",
        help: "Move half a page",
    },
    KeySpec {
        keys: "Tab",
        help: "Switch pane",
    },
    KeySpec {
        keys: "Enter",
        help: "Open detail; in detail, set as default",
    },
    KeySpec {
        keys: "/",
        help: "Search / filter",
    },
    KeySpec {
        keys: ":",
        help: "Command line",
    },
    KeySpec {
        keys: "u / Ctrl+r",
        help: "Undo / redo a change",
    },
    KeySpec {
        keys: "x",
        help: "Remove association (detail pane)",
    },
    KeySpec {
        keys: "R",
        help: "Reset overrides (detail pane)",
    },
    KeySpec {
        keys: "1..6",
        help: "Switch workspace",
    },
    KeySpec {
        keys: "q",
        help: "Quit",
    },
];

/// The completion remainder for a partial command name, if one is unambiguous.
///
/// Returns `None` when `typed` is empty, already complete, or has moved on to
/// an argument.
pub fn complete(typed: &str) -> Option<String> {
    if typed.is_empty() {
        return None;
    }
    for spec in COMMANDS {
        if spec.name == typed || spec.aliases.contains(&typed) {
            return None;
        }
        if spec.name.starts_with(typed) {
            return Some(spec.name[typed.len()..].to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completes_a_unique_prefix() {
        assert_eq!(complete("set-d").as_deref(), Some("efault"));
        assert_eq!(complete("set-default"), None);
    }

    #[test]
    fn a_full_alias_is_not_completed() {
        assert_eq!(complete("rm"), None);
        assert_eq!(complete(""), None);
    }

    #[test]
    fn unknown_prefix_has_no_completion() {
        assert_eq!(complete("zzz"), None);
    }
}
