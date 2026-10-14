//! Modal, keyboard-driven interaction state machine.
//!
//! This is intentionally free of any toolkit dependency: the GTK layer turns
//! key events into [`Key`] values, feeds them here, and performs the resulting
//! [`Action`]s. Keeping it pure makes the whole interaction testable without a
//! display.

/// The top-level views.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Workspace {
    MimeTypes,
    Applications,
    Desktop,
    Config,
}

impl Workspace {
    pub const ALL: [Workspace; 4] = [
        Workspace::MimeTypes,
        Workspace::Applications,
        Workspace::Desktop,
        Workspace::Config,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Workspace::MimeTypes => "MIME TYPES",
            Workspace::Applications => "APPLICATIONS",
            Workspace::Desktop => "DESKTOP",
            Workspace::Config => "CONFIG",
        }
    }

    fn parse(name: &str) -> Option<Self> {
        match name {
            "m" | "mime" | "mime-types" | "mimetypes" => Some(Workspace::MimeTypes),
            "a" | "apps" | "applications" => Some(Workspace::Applications),
            "d" | "desktop" => Some(Workspace::Desktop),
            "c" | "config" => Some(Workspace::Config),
            _ => None,
        }
    }

    fn index(self) -> usize {
        Workspace::ALL.iter().position(|w| *w == self).unwrap_or(0)
    }
}

/// Which pane has focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    List,
    Detail,
}

/// The current editing mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Command,
    Search,
    Confirm,
}

impl Mode {
    pub fn label(self) -> &'static str {
        match self {
            Mode::Normal => "NORMAL",
            Mode::Command => "COMMAND",
            Mode::Search => "SEARCH",
            Mode::Confirm => "CONFIRM",
        }
    }
}

/// A key press, already normalised by the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Char(char),
    Ctrl(char),
    Enter,
    Esc,
    Backspace,
    Tab,
    Up,
    Down,
}

/// Something the interface must carry out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Set the default app (command supplied an app id).
    SetDefault(String),
    /// Add an association (command supplied an app id).
    Add(String),
    /// Remove an association (command supplied an app id).
    Remove(String),
    /// Remove the association for the currently selected app.
    RemoveSelected,
    /// Reset overrides for the currently selected MIME type.
    Reset,
    Undo,
    Redo,
    /// Enter the detail pane for the selected MIME type.
    OpenDetail,
    /// Activate the selected row (e.g. apply set-default).
    Activate,
    OpenConfig,
    SetTheme(String),
    Quit,
}

/// The kind of destructive operation awaiting confirmation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DestructiveKind {
    Remove,
    Reset,
}

/// A parsed `:` command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    SetDefault(String),
    Add(String),
    Remove(String),
    Reset,
    Filter(String),
    Workspace(Workspace),
    OpenConfig,
    Theme(String),
    Quit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Pending {
    Selected(DestructiveKind),
    Command(Command),
}

/// The complete interaction state.
#[derive(Debug, Clone)]
pub struct Interaction {
    workspace: Workspace,
    mode: Mode,
    focus: Focus,
    /// Command or search input buffer.
    input: String,
    /// Committed filter applied to the list.
    filter: String,
    /// Filter saved when a search began, restored on cancel.
    filter_backup: Option<String>,
    list_selection: usize,
    detail_selection: usize,
    list_count: usize,
    detail_count: usize,
    pending_g: bool,
    pending: Option<Pending>,
    status: Option<String>,
    /// Ask before destructive operations (remove/reset).
    confirm_destructive: bool,
}

impl Default for Interaction {
    fn default() -> Self {
        Self {
            workspace: Workspace::MimeTypes,
            mode: Mode::Normal,
            focus: Focus::List,
            input: String::new(),
            filter: String::new(),
            filter_backup: None,
            list_selection: 0,
            detail_selection: 0,
            list_count: 0,
            detail_count: 0,
            pending_g: false,
            pending: None,
            status: None,
            confirm_destructive: true,
        }
    }
}

impl Interaction {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn workspace(&self) -> Workspace {
        self.workspace
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn focus(&self) -> Focus {
        self.focus
    }

    pub fn input(&self) -> &str {
        &self.input
    }

    pub fn filter(&self) -> &str {
        &self.filter
    }

    pub fn status(&self) -> Option<&str> {
        self.status.as_deref()
    }

    /// Replace the transient status message.
    pub fn set_status(&mut self, status: Option<String>) {
        self.status = status;
    }

    /// Whether destructive operations ask for confirmation first.
    pub fn set_confirm_destructive(&mut self, confirm: bool) {
        self.confirm_destructive = confirm;
    }

    /// The selection index of the focused pane.
    pub fn selection(&self) -> usize {
        match self.focus {
            Focus::List => self.list_selection,
            Focus::Detail => self.detail_selection,
        }
    }

    /// The row count of the focused pane.
    pub fn count(&self) -> usize {
        match self.focus {
            Focus::List => self.list_count,
            Focus::Detail => self.detail_count,
        }
    }

    pub fn list_selection(&self) -> usize {
        self.list_selection
    }

    pub fn detail_selection(&self) -> usize {
        self.detail_selection
    }

    /// Tell the interface how many rows are currently visible in each pane.
    pub fn set_counts(&mut self, list: usize, detail: usize) {
        self.list_count = list;
        self.detail_count = detail;
        self.list_selection = clamp(self.list_selection, list);
        self.detail_selection = clamp(self.detail_selection, detail);
    }

    /// Force focus back to the list (e.g. after switching MIME type).
    pub fn focus_list(&mut self) {
        self.focus = Focus::List;
        self.detail_selection = 0;
    }

    /// `SELECTION` modeline field, e.g. `3/128`.
    pub fn selection_label(&self) -> String {
        if self.count() == 0 {
            "0/0".to_string()
        } else {
            format!("{}/{}", self.selection() + 1, self.count())
        }
    }

    /// `COUNT` modeline field.
    pub fn count_label(&self) -> String {
        self.count().to_string()
    }

    /// A human-readable prompt for the pending confirmation, if any.
    pub fn prompt(&self) -> Option<String> {
        match self.pending.as_ref()? {
            Pending::Selected(DestructiveKind::Remove) => {
                Some("remove this association? (y/n)".into())
            }
            Pending::Selected(DestructiveKind::Reset) => {
                Some("reset overrides for this MIME type? (y/n)".into())
            }
            Pending::Command(Command::Remove(app)) => Some(format!("remove {app}? (y/n)")),
            Pending::Command(Command::Reset) => {
                Some("reset overrides for this MIME type? (y/n)".into())
            }
            Pending::Command(_) => None,
        }
    }

    /// Feed one key press and collect the resulting actions.
    pub fn handle_key(&mut self, key: Key) -> Vec<Action> {
        match self.mode {
            Mode::Normal => self.normal(key),
            Mode::Command => self.command(key),
            Mode::Search => self.search(key),
            Mode::Confirm => self.confirm(key),
        }
    }

    fn normal(&mut self, key: Key) -> Vec<Action> {
        self.status = None;
        let half_page = (self.count() / 2).max(1);

        match key {
            Key::Char(':') => {
                self.mode = Mode::Command;
                self.input.clear();
                self.pending_g = false;
            }
            Key::Char('/') => self.begin_search(),
            Key::Char('j') | Key::Down => {
                self.move_selection(1);
                self.pending_g = false;
            }
            Key::Char('k') | Key::Up => {
                self.move_selection(-1);
                self.pending_g = false;
            }
            Key::Char('g') => {
                if self.pending_g {
                    self.selection_mut(|s| *s = 0);
                    self.pending_g = false;
                } else {
                    self.pending_g = true;
                }
            }
            Key::Char('G') => {
                let last = self.count().saturating_sub(1);
                self.selection_mut(|s| *s = last);
                self.pending_g = false;
            }
            Key::Ctrl('d') => self.move_selection(half_page as isize),
            Key::Ctrl('u') => self.move_selection(-(half_page as isize)),
            Key::Tab => {
                if self.focus == Focus::List && self.detail_count > 0 {
                    self.focus = Focus::Detail;
                } else {
                    self.focus = Focus::List;
                }
            }
            Key::Enter => match self.focus {
                Focus::List => {
                    self.focus = Focus::Detail;
                    self.detail_selection = 0;
                    return vec![Action::OpenDetail];
                }
                Focus::Detail => return vec![Action::Activate],
            },
            Key::Esc => {
                if self.focus == Focus::Detail {
                    self.focus = Focus::List;
                } else if !self.filter.is_empty() {
                    self.filter.clear();
                }
            }
            Key::Char('x') if self.focus == Focus::Detail => {
                return self.destructive(
                    Pending::Selected(DestructiveKind::Remove),
                    Action::RemoveSelected,
                );
            }
            Key::Char('R') if self.focus == Focus::Detail => {
                return self.destructive(Pending::Selected(DestructiveKind::Reset), Action::Reset);
            }
            Key::Char('u') => return vec![Action::Undo],
            Key::Ctrl('r') => return vec![Action::Redo],
            Key::Char('q') => return vec![Action::Quit],
            Key::Char(c @ '1'..='4') => {
                let index = c as usize - '1' as usize;
                if let Some(workspace) = Workspace::ALL.get(index) {
                    self.change_workspace(*workspace);
                }
            }
            _ => {}
        }
        Vec::new()
    }

    fn command(&mut self, key: Key) -> Vec<Action> {
        match key {
            Key::Char(c) => self.input.push(c),
            Key::Backspace => {
                self.input.pop();
            }
            Key::Esc => {
                self.mode = Mode::Normal;
                self.input.clear();
            }
            Key::Enter => {
                let input = std::mem::take(&mut self.input);
                self.mode = Mode::Normal;
                return match parse_command(&input) {
                    Ok(Some(command)) => self.apply_command(command),
                    Ok(None) => Vec::new(),
                    Err(message) => {
                        self.status = Some(message);
                        Vec::new()
                    }
                };
            }
            _ => {}
        }
        Vec::new()
    }

    fn search(&mut self, key: Key) -> Vec<Action> {
        match key {
            Key::Char(c) => {
                self.input.push(c);
                self.filter = self.input.clone();
                self.list_selection = 0;
            }
            Key::Backspace => {
                self.input.pop();
                self.filter = self.input.clone();
                self.list_selection = 0;
            }
            Key::Enter => {
                self.mode = Mode::Normal;
                self.filter = std::mem::take(&mut self.input);
                self.filter_backup = None;
            }
            Key::Esc => {
                self.mode = Mode::Normal;
                self.input.clear();
                self.filter = self.filter_backup.take().unwrap_or_default();
                self.list_selection = 0;
            }
            _ => {}
        }
        Vec::new()
    }

    fn confirm(&mut self, key: Key) -> Vec<Action> {
        match key {
            Key::Char('y') | Key::Char('Y') => {
                let pending = self.pending.take();
                self.mode = Mode::Normal;
                match pending {
                    Some(Pending::Selected(DestructiveKind::Remove)) => {
                        vec![Action::RemoveSelected]
                    }
                    Some(Pending::Selected(DestructiveKind::Reset)) => vec![Action::Reset],
                    Some(Pending::Command(Command::Remove(app))) => vec![Action::Remove(app)],
                    Some(Pending::Command(Command::Reset)) => vec![Action::Reset],
                    _ => Vec::new(),
                }
            }
            Key::Char('n') | Key::Char('N') | Key::Esc => {
                self.pending = None;
                self.mode = Mode::Normal;
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    fn apply_command(&mut self, command: Command) -> Vec<Action> {
        match command {
            Command::SetDefault(app) => vec![Action::SetDefault(app)],
            Command::Add(app) => vec![Action::Add(app)],
            Command::Remove(app) => {
                let pending = Pending::Command(Command::Remove(app.clone()));
                self.destructive(pending, Action::Remove(app))
            }
            Command::Reset => self.destructive(Pending::Command(Command::Reset), Action::Reset),
            Command::Filter(text) => {
                self.filter = text;
                self.list_selection = 0;
                Vec::new()
            }
            Command::Workspace(workspace) => {
                self.change_workspace(workspace);
                Vec::new()
            }
            Command::OpenConfig => vec![Action::OpenConfig],
            Command::Theme(name) => vec![Action::SetTheme(name)],
            Command::Quit => vec![Action::Quit],
        }
    }

    fn begin_search(&mut self) {
        self.mode = Mode::Search;
        self.input.clear();
        self.filter_backup = Some(self.filter.clone());
        self.filter.clear();
        self.pending_g = false;
    }

    fn start_confirm(&mut self, pending: Pending) {
        self.mode = Mode::Confirm;
        self.pending = Some(pending);
    }

    /// Either ask for confirmation or return the direct action, per config.
    fn destructive(&mut self, pending: Pending, direct: Action) -> Vec<Action> {
        if self.confirm_destructive {
            self.start_confirm(pending);
            Vec::new()
        } else {
            vec![direct]
        }
    }

    fn change_workspace(&mut self, workspace: Workspace) {
        self.workspace = workspace;
        self.focus = Focus::List;
        self.list_selection = 0;
        self.detail_selection = 0;
        self.pending_g = false;
    }

    fn move_selection(&mut self, delta: isize) {
        let count = self.count();
        if count == 0 {
            return;
        }
        self.selection_mut(|selection| {
            let next = (*selection as isize + delta).clamp(0, count as isize - 1);
            *selection = next as usize;
        });
    }

    fn selection_mut(&mut self, f: impl FnOnce(&mut usize)) {
        match self.focus {
            Focus::List => f(&mut self.list_selection),
            Focus::Detail => f(&mut self.detail_selection),
        }
    }

    /// Current workspace index (1-based) for the modeline/tests.
    pub fn workspace_index(&self) -> usize {
        self.workspace.index() + 1
    }
}

fn clamp(selection: usize, count: usize) -> usize {
    if count == 0 {
        0
    } else {
        selection.min(count - 1)
    }
}

/// Parse a `:` command line. `Ok(None)` means an empty command.
pub fn parse_command(input: &str) -> std::result::Result<Option<Command>, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let (name, arg) = match trimmed.split_once(char::is_whitespace) {
        Some((name, rest)) => (name, rest.trim()),
        None => (trimmed, ""),
    };
    let require_arg = |command: &str| -> std::result::Result<String, String> {
        if arg.is_empty() {
            Err(format!("{command}: missing argument"))
        } else {
            Ok(arg.to_string())
        }
    };
    let command = match name {
        "set" | "set-default" | "default" => Command::SetDefault(require_arg(name)?),
        "add" => Command::Add(require_arg(name)?),
        "remove" | "rm" => Command::Remove(require_arg(name)?),
        "reset" => Command::Reset,
        "filter" | "f" => Command::Filter(arg.to_string()),
        "ws" | "workspace" => {
            let workspace =
                Workspace::parse(arg).ok_or_else(|| format!("ws: unknown workspace '{arg}'"))?;
            Command::Workspace(workspace)
        }
        "open-config" | "config" => Command::OpenConfig,
        "theme" => Command::Theme(require_arg(name)?),
        "q" | "quit" => Command::Quit,
        _ => return Err(format!("unknown command: {name}")),
    };
    Ok(Some(command))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn interaction(count: usize) -> Interaction {
        let mut it = Interaction::new();
        it.set_counts(count, 0);
        it
    }

    fn keys(it: &mut Interaction, keys: &str) -> Vec<Action> {
        let mut actions = Vec::new();
        for c in keys.chars() {
            actions.extend(it.handle_key(Key::Char(c)));
        }
        actions
    }

    #[test]
    fn navigation_clamps_at_bounds() {
        let mut it = interaction(3);
        assert_eq!(it.selection(), 0);
        keys(&mut it, "j");
        assert_eq!(it.selection(), 1);
        keys(&mut it, "jjjj");
        assert_eq!(it.selection(), 2, "j past the end stays on the last row");
        keys(&mut it, "kkkk");
        assert_eq!(it.selection(), 0);

        keys(&mut it, "G");
        assert_eq!(it.selection(), 2);
        keys(&mut it, "gg");
        assert_eq!(it.selection(), 0);
    }

    #[test]
    fn navigation_is_a_noop_when_empty() {
        let mut it = interaction(0);
        keys(&mut it, "jjjG");
        assert_eq!(it.selection(), 0);
        assert_eq!(it.selection_label(), "0/0");
    }

    #[test]
    fn selection_label_is_one_based() {
        let mut it = interaction(10);
        keys(&mut it, "j");
        assert_eq!(it.selection_label(), "2/10");
        assert_eq!(it.count_label(), "10");
    }

    #[test]
    fn search_filters_live_and_cancel_restores() {
        let mut it = interaction(10);
        it.handle_key(Key::Char('/'));
        assert_eq!(it.mode(), Mode::Search);
        keys(&mut it, "png");
        assert_eq!(it.filter(), "png");
        assert_eq!(it.mode(), Mode::Search);

        it.handle_key(Key::Esc);
        assert_eq!(it.mode(), Mode::Normal);
        assert_eq!(it.filter(), "");
    }

    #[test]
    fn search_enter_commits_filter() {
        let mut it = interaction(10);
        keys(&mut it, "/text");
        it.handle_key(Key::Enter);
        assert_eq!(it.mode(), Mode::Normal);
        assert_eq!(it.filter(), "text");
    }

    #[test]
    fn tab_toggles_focus_only_to_a_nonempty_detail() {
        let mut it = Interaction::new();
        it.set_counts(5, 0);
        it.handle_key(Key::Tab);
        assert_eq!(it.focus(), Focus::List, "empty detail cannot be focused");

        it.set_counts(5, 3);
        it.handle_key(Key::Tab);
        assert_eq!(it.focus(), Focus::Detail);
        it.handle_key(Key::Tab);
        assert_eq!(it.focus(), Focus::List);
    }

    #[test]
    fn enter_opens_detail_then_activates() {
        let mut it = Interaction::new();
        it.set_counts(5, 4);
        assert_eq!(it.handle_key(Key::Enter), vec![Action::OpenDetail]);
        assert_eq!(it.focus(), Focus::Detail);
        assert_eq!(it.handle_key(Key::Enter), vec![Action::Activate]);
    }

    #[test]
    fn interactive_destructive_requires_confirmation() {
        let mut it = Interaction::new();
        it.set_counts(1, 1);
        it.handle_key(Key::Tab); // focus detail
        assert_eq!(it.focus(), Focus::Detail);

        assert!(keys(&mut it, "x").is_empty());
        assert_eq!(it.mode(), Mode::Confirm);
        assert!(it.prompt().unwrap().contains("remove"));
        assert_eq!(keys(&mut it, "y"), vec![Action::RemoveSelected]);

        keys(&mut it, "R");
        assert_eq!(it.mode(), Mode::Confirm);
        assert_eq!(keys(&mut it, "n"), Vec::<Action>::new());
        assert_eq!(it.mode(), Mode::Normal);
    }

    #[test]
    fn destructive_without_confirmation_acts_immediately() {
        let mut it = Interaction::new();
        it.set_confirm_destructive(false);
        it.set_counts(1, 1);
        it.handle_key(Key::Tab); // focus detail
        assert_eq!(it.focus(), Focus::Detail);

        assert_eq!(keys(&mut it, "x"), vec![Action::RemoveSelected]);
        assert_eq!(it.mode(), Mode::Normal);
        assert_eq!(keys(&mut it, "R"), vec![Action::Reset]);
        assert_eq!(it.mode(), Mode::Normal);

        let mut actions = Vec::new();
        for c in ":reset".chars() {
            actions.extend(it.handle_key(Key::Char(c)));
        }
        actions.extend(it.handle_key(Key::Enter));
        assert_eq!(actions, vec![Action::Reset]);
    }

    #[test]
    fn commands_parse_to_actions() {
        let mut it = interaction(3);
        keys(&mut it, ":set-default foot.desktop");
        it.handle_key(Key::Enter);
        assert_eq!(it.status(), None);
        // Re-run capturing returned actions via a fresh interaction.
        let mut it = interaction(3);
        let mut actions = Vec::new();
        for c in ":set foot.desktop".chars() {
            actions.extend(it.handle_key(Key::Char(c)));
        }
        actions.extend(it.handle_key(Key::Enter));
        assert_eq!(actions, vec![Action::SetDefault("foot.desktop".into())]);
    }

    #[test]
    fn unknown_command_sets_status_and_returns_to_normal() {
        let mut it = interaction(1);
        keys(&mut it, ":nonsense");
        it.handle_key(Key::Enter);
        assert_eq!(it.mode(), Mode::Normal);
        assert!(it.status().unwrap().contains("unknown command"));
    }

    #[test]
    fn remove_and_reset_commands_ask_for_confirmation() {
        let mut it = interaction(1);
        keys(&mut it, ":remove foo.desktop");
        it.handle_key(Key::Enter);
        assert_eq!(it.mode(), Mode::Confirm);
        assert_eq!(
            keys(&mut it, "y"),
            vec![Action::Remove("foo.desktop".into())]
        );

        keys(&mut it, ":reset");
        it.handle_key(Key::Enter);
        assert_eq!(it.mode(), Mode::Confirm);
        assert_eq!(keys(&mut it, "y"), vec![Action::Reset]);
    }

    #[test]
    fn undo_redo_and_quit_actions() {
        let mut it = interaction(1);
        assert_eq!(keys(&mut it, "u"), vec![Action::Undo]);
        assert_eq!(
            vec![it.handle_key(Key::Ctrl('r'))],
            vec![vec![Action::Redo]]
        );
        assert_eq!(keys(&mut it, "q"), vec![Action::Quit]);
    }

    #[test]
    fn workspaces_switch_by_number_and_command() {
        let mut it = interaction(1);
        keys(&mut it, "3");
        assert_eq!(it.workspace(), Workspace::Desktop);
        assert_eq!(it.focus(), Focus::List);

        keys(&mut it, ":ws apps");
        it.handle_key(Key::Enter);
        assert_eq!(it.workspace(), Workspace::Applications);
    }

    #[test]
    fn parse_command_surface() {
        assert_eq!(parse_command("").unwrap(), None);
        assert_eq!(
            parse_command("filter png").unwrap(),
            Some(Command::Filter("png".into()))
        );
        assert_eq!(
            parse_command("set-default x").unwrap(),
            Some(Command::SetDefault("x".into()))
        );
        assert!(parse_command("set-default").is_err());
        assert!(parse_command("ws bogus").is_err());
    }
}
