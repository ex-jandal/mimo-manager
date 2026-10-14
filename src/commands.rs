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
        help: "Switch workspace (mime, apps, config, simple, help).",
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
        keys: "Ctrl+j / Ctrl+k",
        help: "Next / previous workspace",
    },
    KeySpec {
        keys: "Ctrl+n / Ctrl+p",
        help: "In the : command list: next / previous (also Ctrl+j / Ctrl+k)",
    },
    KeySpec {
        keys: "1..5",
        help: "Switch workspace",
    },
    KeySpec {
        keys: "q",
        help: "Quit",
    },
];

/// Values for the argument of `:ws`, mirroring [`crate::interaction::Workspace`].
pub const WORKSPACES: &[&str] = &["mime", "apps", "config", "simple", "help"];

/// The completion candidates for a command-line input (the text after `:`).
///
/// Returns full re-insertable strings: a bare command name (with a trailing
/// space when it takes an argument), or `command value` for an enumerated
/// argument such as `:theme` or `:ws`.
pub fn candidates(typed: &str) -> Vec<String> {
    match typed.split_once(char::is_whitespace) {
        None => COMMANDS
            .iter()
            .filter(|spec| {
                spec.name.starts_with(typed)
                    || spec.aliases.iter().any(|alias| alias.starts_with(typed))
            })
            .map(|spec| {
                if spec.args.is_empty() {
                    spec.name.to_string()
                } else {
                    format!("{} ", spec.name)
                }
            })
            .collect(),
        Some((command, argument)) => argument_candidates(command, argument),
    }
}

fn argument_candidates(command: &str, argument: &str) -> Vec<String> {
    let Some(spec) = COMMANDS
        .iter()
        .find(|spec| spec.name == command || spec.aliases.contains(&command))
    else {
        return Vec::new();
    };
    let values: Vec<&str> = match spec.name {
        "theme" => crate::theme::theme_names(),
        "ws" => WORKSPACES.to_vec(),
        _ => return Vec::new(),
    };
    let prefix = argument.to_lowercase();
    values
        .into_iter()
        .filter(|value| value.to_lowercase().starts_with(&prefix))
        .map(|value| format!("{} {}", spec.name, value))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completes_a_command_name_prefix() {
        assert_eq!(candidates("set-d"), vec!["set-default "]);
        assert_eq!(candidates("rm"), vec!["remove "]);
        assert!(candidates("re").contains(&"reset".to_string()));
    }

    #[test]
    fn empty_input_lists_every_command() {
        assert_eq!(candidates("").len(), COMMANDS.len());
    }

    #[test]
    fn command_alias_prefix_completes_to_canonical_name() {
        assert!(candidates("wor").contains(&"ws ".to_string()));
    }

    #[test]
    fn theme_argument_lists_theme_names() {
        let all = candidates("theme ");
        assert_eq!(all.len(), crate::theme::theme_names().len());
        assert!(all.iter().all(|c| c.starts_with("theme ")));
        assert_eq!(candidates("theme NOR"), vec!["theme NORD"]);
        assert!(
            candidates("theme ")
                .iter()
                .any(|c| c == "theme GRUVBOX DARK")
        );
    }

    #[test]
    fn ws_argument_lists_workspaces() {
        assert_eq!(candidates("ws a"), vec!["ws apps"]);
        assert!(candidates("ws ").contains(&"ws help".to_string()));
    }

    #[test]
    fn unknown_prefix_or_argument_has_no_candidates() {
        assert!(candidates("zzz").is_empty());
        assert!(candidates("set-default foo").is_empty());
        assert!(candidates("nope x").is_empty());
    }
}
