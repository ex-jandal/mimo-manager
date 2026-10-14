//! Launching the user's editor.
//!
//! `$EDITOR` is usually a terminal editor (`nvim`, `vim`, …). Spawning it
//! directly from the graphical launcher leaves it without a terminal, so it
//! silently shows nothing. Terminal editors are therefore wrapped in a terminal
//! emulator; GUI editors are launched as-is.

use std::path::{Path, PathBuf};
use std::process::Stdio;

/// Editors that open their own window; launch them without a terminal.
const GUI_EDITORS: &[&str] = &[
    "code",
    "codium",
    "code-insiders",
    "codium-insiders",
    "gvim",
    "mvim",
    "nvim-qt",
    "zed",
    "subl",
    "sublime_text",
    "gedit",
    "kate",
    "gnome-text-editor",
    "mousepad",
    "xed",
    "leafpad",
    "emacs",
    "emacsclient",
];

/// Known terminal emulators and the flags that run a command inside them.
const TERMINALS: &[(&str, &[&str])] = &[
    ("xdg-terminal-exec", &[]),
    ("foot", &[]),
    ("kitty", &[]),
    ("alacritty", &["-e"]),
    ("ghostty", &["-e"]),
    ("wezterm", &["start", "--"]),
    ("konsole", &["-e"]),
    ("gnome-terminal", &["--"]),
    ("xfce4-terminal", &["-x"]),
    ("xterm", &["-e"]),
];

fn basename(program: &str) -> &str {
    Path::new(program)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(program)
}

fn is_gui_editor(name: &str) -> bool {
    GUI_EDITORS.contains(&name)
}

/// Whether an editor's flags force it to stay inside a terminal.
fn forces_terminal(args: &[&str]) -> bool {
    args.iter()
        .any(|a| matches!(*a, "-nw" | "--no-window-system" | "--nowindows"))
}

fn terminal_emulator(name: &str) -> Option<&'static [&'static str]> {
    TERMINALS
        .iter()
        .find(|(t, _)| *t == name)
        .map(|(_, flags)| *flags)
}

/// The editor argv that opens `path`, wrapping terminal editors in `terminal`.
///
/// `Err` reports an editor that needs a terminal when none could be found.
pub fn build_command(
    editor: &str,
    terminal: Option<&str>,
    path: &Path,
) -> Result<Vec<String>, String> {
    let mut words = editor.split_whitespace();
    let program = words.next().unwrap_or("vi");
    let args: Vec<&str> = words.collect();
    let name = basename(program);

    let mut argv = Vec::new();
    let run_editor = |argv: &mut Vec<String>| {
        argv.push(program.to_string());
        argv.extend(args.iter().map(|a| a.to_string()));
        argv.push(path.to_string_lossy().into_owned());
    };

    // A GUI editor (unless told to stay in a terminal) or an `$EDITOR` that is
    // itself a terminal emulator already opens its own window.
    if terminal_emulator(name).is_some() || (is_gui_editor(name) && !forces_terminal(&args)) {
        run_editor(&mut argv);
        return Ok(argv);
    }

    let Some(terminal) = terminal else {
        return Err(format!(
            "{program} needs a terminal but none was found (set $TERMINAL)"
        ));
    };
    argv.push(terminal.to_string());
    match terminal_emulator(basename(terminal)) {
        Some(flags) => argv.extend(flags.iter().map(|f| f.to_string())),
        None => argv.push("-e".to_string()),
    }
    run_editor(&mut argv);
    Ok(argv)
}

/// The terminal to wrap editors in: `$TERMINAL`, then `xdg-terminal-exec`, then
/// the first known emulator on `PATH`.
pub fn detect_terminal() -> Option<String> {
    if let Ok(term) = std::env::var("TERMINAL")
        && let Some(name) = term.split_whitespace().next()
    {
        return Some(name.to_string());
    }
    TERMINALS
        .iter()
        .map(|(name, _)| *name)
        .find(|name| which(name).is_some())
        .map(str::to_string)
}

/// Spawn the editor for `path`, returning the argv used (for status messages).
pub fn launch(path: &Path) -> Result<Vec<String>, String> {
    let editor = ["EDITOR", "VISUAL"]
        .iter()
        .find_map(|key| {
            std::env::var(key)
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
        .unwrap_or_else(|| "vi".to_string());
    let argv = build_command(&editor, detect_terminal().as_deref(), path)?;
    std::process::Command::new(&argv[0])
        .args(&argv[1..])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("cannot run {}: {error}", argv[0]))?;
    Ok(argv)
}

fn which(program: &str) -> Option<PathBuf> {
    if program.contains('/') {
        let path = Path::new(program);
        return path.is_file().then(|| path.to_path_buf());
    }
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(program))
        .find(|candidate| candidate.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cmd(editor: &str, terminal: Option<&str>) -> Result<Vec<String>, String> {
        build_command(editor, terminal, Path::new("/tmp/x.toml"))
    }

    #[test]
    fn wraps_terminal_editor_for_each_terminal() {
        assert_eq!(
            cmd("nvim", Some("foot")).unwrap(),
            vec!["foot", "nvim", "/tmp/x.toml"]
        );
        assert_eq!(
            cmd("nvim", Some("alacritty")).unwrap(),
            vec!["alacritty", "-e", "nvim", "/tmp/x.toml"]
        );
        assert_eq!(
            cmd("nvim", Some("wezterm")).unwrap(),
            vec!["wezterm", "start", "--", "nvim", "/tmp/x.toml"]
        );
        assert_eq!(
            cmd("nvim", Some("xdg-terminal-exec")).unwrap(),
            vec!["xdg-terminal-exec", "nvim", "/tmp/x.toml"]
        );
        assert_eq!(
            cmd("/usr/bin/nvim", Some("foot")).unwrap(),
            vec!["foot", "/usr/bin/nvim", "/tmp/x.toml"]
        );
    }

    #[test]
    fn gui_editors_run_directly_even_with_a_terminal() {
        assert_eq!(
            cmd("code", Some("foot")).unwrap(),
            vec!["code", "/tmp/x.toml"]
        );
        assert_eq!(
            cmd("gvim -f", None).unwrap(),
            vec!["gvim", "-f", "/tmp/x.toml"]
        );
    }

    #[test]
    fn forced_or_missing_terminal() {
        assert_eq!(
            cmd("emacs -nw", Some("foot")).unwrap(),
            vec!["foot", "emacs", "-nw", "/tmp/x.toml"]
        );
        assert!(cmd("nvim", None).is_err(), "no terminal available");
    }

    #[test]
    fn terminal_editor_is_not_double_wrapped() {
        assert_eq!(
            cmd("foot vim", None).unwrap(),
            vec!["foot", "vim", "/tmp/x.toml"]
        );
    }

    #[test]
    fn empty_editor_defaults_to_vi() {
        assert_eq!(
            cmd("   ", Some("foot")).unwrap(),
            vec!["foot", "vi", "/tmp/x.toml"]
        );
    }
}
