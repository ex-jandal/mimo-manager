//! GTK4 + Relm4 user interface.
//!
//! The view is deliberately thin: it translates key events into
//! [`crate::interaction::Key`] values, feeds them to the state machine, and
//! carries out the resulting [`crate::interaction::Action`]s.

use std::process::Command as ProcCommand;

use relm4::RelmRemoveAllExt;
use relm4::gtk::prelude::*;
use relm4::gtk::{self, gdk, glib};
use relm4::prelude::*;

use crate::assoc::{AssocEdit, Associations, MimeappsFile};
use crate::commands;
use crate::config::Config;
use crate::desktop::discovery::{DiscoveryReport, discover};
use crate::interaction::{Action, Interaction, Key as UiKey, Mode, Workspace};
use crate::mime::MimeDatabase;
use crate::session::Session;
use crate::simple;
use crate::theme::{self, Modeline, ModelineFields};
use crate::xdg::Paths;

struct AppModel {
    paths: Paths,
    config: Config,
    db: MimeDatabase,
    apps: DiscoveryReport,
    assoc: Associations,
    session: Session,
    interaction: Interaction,
    list_items: Vec<String>,
    list_app_indices: Vec<usize>,
    rendered_list: Vec<String>,
    detail_items: Vec<String>,
    detail_ids: Vec<String>,
    rendered_detail: Vec<String>,
    load_error: Option<String>,
    modeline_fields: ModelineFields,

    window: gtk::ApplicationWindow,
    list_box: gtk::ListBox,
    list_scroll: gtk::ScrolledWindow,
    detail_box: gtk::ListBox,
    detail_scroll: gtk::ScrolledWindow,
    input_label: gtk::Label,
    detail_title: gtk::Label,
    modeline_left: gtk::Label,
    modeline_right: gtk::Label,
    workspace_tabs: gtk::Label,
}

#[derive(Debug)]
enum AppMsg {
    Key(UiKey),
}

#[relm4::component]
impl SimpleComponent for AppModel {
    type Init = ();
    type Input = AppMsg;
    type Output = ();

    view! {
        main_window = gtk::ApplicationWindow {
            set_title: Some("mimo_manager"),
            set_default_size: (1000, 640),
            set_decorated: false,
            add_css_class: "mono",

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,

                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_homogeneous: true,
                    set_hexpand: true,
                    set_vexpand: true,

                    #[name(list_scroll)]
                    gtk::ScrolledWindow {
                        set_hexpand: true,
                        set_vexpand: true,
                        set_policy: (gtk::PolicyType::Never, gtk::PolicyType::Automatic),
                        #[name(list_box)]
                        gtk::ListBox {
                            set_selection_mode: gtk::SelectionMode::Single,
                            add_css_class: "pane",
                        },
                    },

                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_hexpand: true,

                        #[name(detail_title)]
                        gtk::Label {
                            set_xalign: 0.0,
                            set_ellipsize: gtk::pango::EllipsizeMode::End,
                            set_max_width_chars: 1,
                            add_css_class: "detail-title",
                        },

                        #[name(detail_scroll)]
                        gtk::ScrolledWindow {
                            set_vexpand: true,
                            set_policy: (gtk::PolicyType::Never, gtk::PolicyType::Automatic),
                            #[name(detail_box)]
                            gtk::ListBox {
                                set_selection_mode: gtk::SelectionMode::Single,
                                add_css_class: "pane",
                            },
                        },
                    },
                },

                #[name(input_label)]
                gtk::Label {
                    set_xalign: 0.0,
                    set_use_markup: true,
                    add_css_class: "commandline",
                },

                gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    add_css_class: "modeline",

                    #[name(modeline_left)]
                    gtk::Label {
                        set_xalign: 0.0,
                        set_hexpand: true,
                        set_use_markup: true,
                    },

                    #[name(modeline_right)]
                    gtk::Label {
                        set_xalign: 1.0,
                        set_use_markup: true,
                    },
                },

                #[name(workspace_tabs)]
                gtk::Label {
                    set_xalign: 0.0,
                    set_use_markup: true,
                    add_css_class: "workstabs",
                },
            },
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        install_css();

        let widgets = view_output!();

        let (paths, config, db, apps, assoc, session, load_error) = load_world();

        let mut interaction = Interaction::new();
        interaction.set_confirm_destructive(config.confirm_destructive);

        let mut model = AppModel {
            paths,
            config,
            db,
            apps,
            assoc,
            session,
            interaction,
            list_items: Vec::new(),
            list_app_indices: Vec::new(),
            rendered_list: Vec::new(),
            detail_items: Vec::new(),
            detail_ids: Vec::new(),
            rendered_detail: Vec::new(),
            load_error,
            modeline_fields: ModelineFields::default(),
            window: widgets.main_window.clone(),
            list_box: widgets.list_box.clone(),
            list_scroll: widgets.list_scroll.clone(),
            detail_box: widgets.detail_box.clone(),
            detail_scroll: widgets.detail_scroll.clone(),
            input_label: widgets.input_label.clone(),
            detail_title: widgets.detail_title.clone(),
            modeline_left: widgets.modeline_left.clone(),
            modeline_right: widgets.modeline_right.clone(),
            workspace_tabs: widgets.workspace_tabs.clone(),
        };
        model.modeline_fields = model.config.modeline.clone().into();

        install_key_controller(&root, sender.clone());
        model.refresh();
        if let Some(error) = model.load_error.take() {
            model.interaction.set_status(Some(error));
            model.update_modeline();
        }

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, _sender: ComponentSender<Self>) {
        match msg {
            AppMsg::Key(key) => {
                let actions = self.interaction.handle_key(key);
                self.apply_actions(actions);
                self.refresh();
            }
        }
    }
}

impl AppModel {
    fn apply_actions(&mut self, actions: Vec<Action>) {
        for action in actions {
            match action {
                Action::SetDefault(app) => {
                    if let Some(mime) = self.selected_mime() {
                        self.edit(AssocEdit::SetDefault { mime, app });
                    }
                }
                Action::SetDefaultFor(app) => self.set_default_for(app),
                Action::Add(app) => {
                    if let Some(mime) = self.selected_mime() {
                        self.edit(AssocEdit::Add { mime, app });
                    }
                }
                Action::Remove(app) => {
                    if let Some(mime) = self.selected_mime() {
                        self.edit(AssocEdit::Remove { mime, app });
                    }
                }
                Action::RemoveSelected => {
                    if let (Some(mime), Some(app)) =
                        (self.selected_mime(), self.selected_detail_app())
                    {
                        self.edit(AssocEdit::Remove { mime, app });
                    }
                }
                Action::Reset => {
                    if let Some(mime) = self.selected_mime() {
                        self.edit(AssocEdit::Reset { mime });
                    }
                }
                Action::Undo => match self.session.undo() {
                    Ok(true) => self.after_write("undo"),
                    Ok(false) => self.interaction.set_status(Some("nothing to undo".into())),
                    Err(error) => self.fail(error),
                },
                Action::Redo => match self.session.redo() {
                    Ok(true) => self.after_write("redo"),
                    Ok(false) => self.interaction.set_status(Some("nothing to redo".into())),
                    Err(error) => self.fail(error),
                },
                Action::OpenDetail => {}
                Action::Activate => match self.interaction.workspace() {
                    Workspace::MimeTypes => {
                        if let (Some(mime), Some(app)) =
                            (self.selected_mime(), self.selected_detail_app())
                        {
                            self.edit(AssocEdit::SetDefault { mime, app });
                        }
                    }
                    Workspace::Categories => {
                        if let Some(app) = self.selected_detail_app() {
                            self.set_default_for(app);
                        }
                    }
                    _ => {}
                },
                Action::OpenConfig => self.open_config(),
                Action::SetTheme(_) => {
                    self.interaction
                        .set_status(Some("themes arrive in a later milestone".into()));
                }
                Action::Quit => self.window.close(),
            }
        }
    }

    fn edit(&mut self, edit: AssocEdit) {
        match self.session.apply(&edit) {
            Ok(()) => self.after_write(describe(&edit)),
            Err(error) => self.fail(error),
        }
    }

    fn after_write(&mut self, what: &str) {
        match Associations::load(&self.paths) {
            Ok(assoc) => self.assoc = assoc,
            Err(error) => return self.fail(error),
        }
        self.interaction.set_status(Some(what.to_string()));
    }

    fn fail(&mut self, error: crate::Error) {
        self.interaction.set_status(Some(format!("error: {error}")));
    }

    fn open_config(&mut self) {
        let path = self.paths.config_file();
        let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
        match ProcCommand::new(editor).arg(&path).spawn() {
            Ok(_) => self
                .interaction
                .set_status(Some(format!("opened {}", path.display()))),
            Err(error) => self
                .interaction
                .set_status(Some(format!("cannot open editor: {error}"))),
        }
    }

    /// The MIME type selected in the list (only in the MIME workspace).
    fn selected_mime(&self) -> Option<String> {
        if self.interaction.workspace() != Workspace::MimeTypes {
            return None;
        }
        self.list_items
            .get(self.interaction.list_selection())
            .cloned()
    }

    /// The desktop-entry id selected in the detail pane.
    fn selected_detail_app(&self) -> Option<String> {
        self.detail_ids
            .get(self.interaction.detail_selection())
            .cloned()
    }

    /// The simple-mode entry selected in the list.
    fn selected_simple(&self) -> Option<&'static simple::SimpleEntry> {
        if self.interaction.workspace() != Workspace::Categories {
            return None;
        }
        let label = self.list_items.get(self.interaction.list_selection())?;
        simple::entries().iter().find(|entry| entry.label == label)
    }

    /// Set `app` as the default for every member of the selected simple entry.
    fn set_default_for(&mut self, app: String) {
        let Some(entry) = self.selected_simple() else {
            return;
        };
        let edits: Vec<AssocEdit> = entry
            .members(&self.db)
            .into_iter()
            .map(|mime| AssocEdit::SetDefault {
                mime,
                app: app.clone(),
            })
            .collect();
        let what = format!("set default for {}", entry.label);
        match self.session.apply_batch(&edits) {
            Ok(()) => self.after_write(&what),
            Err(error) => self.fail(error),
        }
    }

    fn refresh(&mut self) {
        self.rebuild_list();
        self.rebuild_detail();

        let list_count = self.list_items.len();
        let detail_count = self.detail_items.len();
        self.interaction.set_counts(list_count, detail_count);

        // set_counts may have clamped the selection; re-select and scroll.
        self.rebuild_list();
        self.rebuild_detail();
        self.update_page_rows();
        self.update_modeline();
        self.update_input_line();
        self.update_workspace_tabs();
    }

    /// Render the workspace tab strip, highlighting the active workspace.
    fn update_workspace_tabs(&mut self) {
        let labels: Vec<&str> = Workspace::ALL.iter().map(|w| w.label()).collect();
        let active = self.interaction.workspace_index() - 1;
        self.workspace_tabs
            .set_markup(&theme::workspace_tabs(&labels, active));
    }

    /// Derive how many rows fit on screen so half-page jumps match the window.
    fn update_page_rows(&mut self) {
        let viewport = self.list_scroll.height();
        if viewport <= 0 {
            return;
        }
        let row_height = self
            .list_box
            .row_at_index(0)
            .and_then(|row| row.compute_bounds(&self.list_box))
            .map(|bounds| bounds.height() as f64)
            .filter(|height| *height > 0.0)
            .unwrap_or(24.0);
        let rows = (viewport as f64 / row_height).floor().max(1.0) as usize;
        self.interaction.set_page_rows(rows);
    }

    fn rebuild_list(&mut self) {
        let filter = self.interaction.filter().to_lowercase();
        self.list_items.clear();
        self.list_app_indices.clear();

        match self.interaction.workspace() {
            Workspace::MimeTypes => {
                for mime in self.db.all() {
                    if filter.is_empty() || mime.name.to_lowercase().contains(&filter) {
                        self.list_items.push(mime.name.clone());
                    }
                }
            }
            Workspace::Applications => {
                for (index, app) in self.apps.apps.iter().enumerate() {
                    if filter.is_empty() || app.name.to_lowercase().contains(&filter) {
                        self.list_items.push(app.name.clone());
                        self.list_app_indices.push(index);
                    }
                }
            }
            Workspace::Categories => {
                for entry in simple::entries() {
                    if filter.is_empty() || entry.label.to_lowercase().contains(&filter) {
                        self.list_items.push(entry.label.to_string());
                    }
                }
            }
            Workspace::Help => {
                if filter.is_empty() || "key bindings".contains(&filter) {
                    self.list_items.push("KEY BINDINGS".to_string());
                }
                for spec in commands::COMMANDS {
                    if filter.is_empty() || spec.name.contains(&filter) {
                        self.list_items.push(spec.name.to_string());
                    }
                }
            }
            Workspace::Desktop | Workspace::Config => {}
        }
        // ponytail: rebuilds all rows only when the item set changes; virtualize
        // with ListView if a huge list still lags.
        if self.list_items != self.rendered_list {
            fill_rows(&self.list_box, &self.list_items);
            self.rendered_list = self.list_items.clone();
        }
        select_and_scroll(
            &self.list_box,
            &self.list_scroll,
            self.interaction.list_selection(),
        );
    }

    fn rebuild_detail(&mut self) {
        self.detail_items.clear();
        self.detail_ids.clear();

        match self.interaction.workspace() {
            Workspace::MimeTypes => {
                if let Some(mime) = self
                    .list_items
                    .get(self.interaction.list_selection())
                    .cloned()
                {
                    let default = self.assoc.default_app(&mime).map(str::to_string);
                    for app in &self.apps.apps {
                        if app.mime_types.iter().any(|m| m.eq_ignore_ascii_case(&mime)) {
                            let marker = if default.as_deref() == Some(app.id.as_str()) {
                                "● "
                            } else {
                                "  "
                            };
                            self.detail_items
                                .push(format!("{marker}{}  [{}]", app.name, app.id));
                            self.detail_ids.push(app.id.clone());
                        }
                    }
                    if self.detail_items.is_empty() {
                        self.detail_items.push("(no applications)".into());
                    }
                    self.detail_title
                        .set_label(&format!("{mime}  ·  {} apps", self.detail_ids.len()));
                } else {
                    self.detail_title.set_label("(no MIME type selected)");
                }
            }
            Workspace::Applications => {
                if let Some(app) = self
                    .list_app_indices
                    .get(self.interaction.list_selection())
                    .and_then(|index| self.apps.apps.get(*index))
                {
                    self.detail_title.set_label(&app.name);
                    self.detail_items.push(format!("id:   {}", app.id));
                    self.detail_items
                        .push(format!("exec: {}", app.exec.as_deref().unwrap_or("-")));
                    self.detail_items.push(format!(
                        "type: {}",
                        match &app.validity {
                            crate::domain::Validity::Valid => "Application".to_string(),
                            crate::domain::Validity::Invalid(reason) =>
                                format!("invalid ({reason})"),
                        }
                    ));
                    self.detail_items.push(format!(
                        "tags: {}",
                        if app.tags().is_empty() {
                            "-".to_string()
                        } else {
                            app.tags().join(", ")
                        }
                    ));
                    self.detail_items
                        .push(format!("mime: {}", app.mime_types.join(", ")));
                    self.detail_items
                        .push(format!("path: {}", app.path.display()));
                } else {
                    self.detail_title.set_label("");
                }
            }
            Workspace::Categories => {
                if let Some(entry) = self.selected_simple() {
                    let members = entry.members(&self.db);
                    let default = self.assoc.default_app(entry.probe).map(str::to_string);
                    let mut seen = std::collections::HashSet::new();
                    for app in &self.apps.apps {
                        let applies = app
                            .mime_types
                            .iter()
                            .any(|m| members.iter().any(|member| member.eq_ignore_ascii_case(m)));
                        if applies && seen.insert(app.id.clone()) {
                            let marker = if default.as_deref() == Some(app.id.as_str()) {
                                "● "
                            } else {
                                "  "
                            };
                            self.detail_items
                                .push(format!("{marker}{}  [{}]", app.name, app.id));
                            self.detail_ids.push(app.id.clone());
                        }
                    }
                    if self.detail_items.is_empty() {
                        self.detail_items.push("(no applications)".into());
                    }
                    let kind = if entry.role {
                        "default role"
                    } else {
                        "content category"
                    };
                    let suffix = if entry.label == "Terminal" {
                        "  ·  some tools read $TERMINAL instead"
                    } else {
                        ""
                    };
                    self.detail_title
                        .set_label(&format!("{}  ·  {kind}{suffix}", entry.label));
                } else {
                    self.detail_title.set_label("(no entry selected)");
                }
            }
            Workspace::Help => {
                match self
                    .list_items
                    .get(self.interaction.list_selection())
                    .map(String::as_str)
                {
                    Some("KEY BINDINGS") => {
                        self.detail_title.set_label("Key bindings");
                        for key in commands::KEYS {
                            self.detail_items
                                .push(format!("{:<16}  {}", key.keys, key.help));
                        }
                    }
                    Some(name) => {
                        if let Some(spec) = commands::COMMANDS.iter().find(|s| s.name == name) {
                            self.detail_title.set_label(&format!(":{}", spec.name));
                            let usage = if spec.args.is_empty() {
                                format!(":{}", spec.name)
                            } else {
                                format!(":{} {}", spec.name, spec.args)
                            };
                            self.detail_items.push(format!("usage:   {usage}"));
                            if !spec.aliases.is_empty() {
                                self.detail_items
                                    .push(format!("aliases: {}", spec.aliases.join(", ")));
                            }
                            self.detail_items.push(String::new());
                            self.detail_items.push(spec.help.to_string());
                        } else {
                            self.detail_title.set_label("");
                        }
                    }
                    None => self.detail_title.set_label("Help"),
                }
            }
            Workspace::Desktop => {
                self.detail_title.set_label("Desktop entries");
                self.detail_items
                    .push("Browse and edit desktop entries (later milestone)".into());
            }
            Workspace::Config => {
                self.detail_title.set_label("Configuration");
                self.detail_items.push(format!(
                    "config file: {}",
                    self.paths.config_file().display()
                ));
            }
        }
        if self.detail_items != self.rendered_detail {
            fill_rows(&self.detail_box, &self.detail_items);
            self.rendered_detail = self.detail_items.clone();
        }
        select_and_scroll(
            &self.detail_box,
            &self.detail_scroll,
            self.interaction.detail_selection(),
        );
    }

    fn update_modeline(&mut self) {
        let it = &self.interaction;

        // The MIME key whose default represents the current selection: the
        // selected MIME type, or the probe of the selected simple entry.
        let key = self
            .selected_mime()
            .or_else(|| self.selected_simple().map(|entry| entry.probe.to_string()));

        let default = key
            .as_deref()
            .and_then(|mime| self.assoc.default_app(mime).map(str::to_string))
            .or_else(|| {
                key.as_deref()
                    .and_then(|mime| crate::queries::default_app(mime).map(|a| a.name))
            })
            .unwrap_or_else(|| "-".to_string());

        let scope = match key.as_deref() {
            Some(mime) if self.user_override(mime) => "user",
            Some(_) => "system",
            None => "-",
        };

        let status = it
            .prompt()
            .or_else(|| it.status().map(str::to_string))
            .unwrap_or_default();

        let modeline = Modeline {
            mode: it.mode().label().to_string(),
            workspace: it.workspace().label().to_string(),
            selection: it.selection_label(),
            count: it.count_label(),
            default: format!("DEFAULT {default}"),
            scope: format!("SCOPE {scope}"),
            status,
            theme: self.config.theme.clone(),
        };
        let (left, right) = theme::render(&self.modeline_fields, &modeline);
        self.modeline_left.set_markup(&left);
        self.modeline_right.set_markup(&right);
    }

    fn user_override(&self, mime: &str) -> bool {
        MimeappsFile::load(self.session.target())
            .map(|file| file.sets(mime))
            .unwrap_or(false)
    }

    fn update_input_line(&mut self) {
        let it = &self.interaction;
        let (text, ghost, visible) = match it.mode() {
            Mode::Command => (
                format!(":{}", it.input()),
                it.command_suggestion().unwrap_or_default(),
                true,
            ),
            Mode::Search => (format!("/{}", it.input()), String::new(), true),
            Mode::Confirm => (it.prompt().unwrap_or_default(), String::new(), true),
            Mode::Normal => (String::new(), String::new(), false),
        };

        if text.is_empty() && ghost.is_empty() {
            self.input_label.set_label("");
        } else {
            let typed = glib::markup_escape_text(&text);
            let suffix = if ghost.is_empty() {
                String::new()
            } else {
                format!(
                    "<span foreground=\"{}\">{}</span>",
                    theme::MELANGE_DARK.fg_dim,
                    glib::markup_escape_text(&ghost)
                )
            };
            self.input_label.set_markup(&format!("{typed}{suffix}"));
        }
        self.input_label.set_visible(visible);
    }
}

fn describe(edit: &AssocEdit) -> &'static str {
    match edit {
        AssocEdit::SetDefault { .. } => "set default",
        AssocEdit::Add { .. } => "added association",
        AssocEdit::Remove { .. } => "removed association",
        AssocEdit::Reset { .. } => "reset overrides",
    }
}

fn load_world() -> (
    Paths,
    Config,
    MimeDatabase,
    DiscoveryReport,
    Associations,
    Session,
    Option<String>,
) {
    let paths = match Paths::from_env() {
        Ok(paths) => paths,
        Err(error) => {
            let paths = Paths::rooted("/");
            let session = Session::new(paths.clone());
            return (
                paths,
                Config::default(),
                MimeDatabase::default(),
                DiscoveryReport::default(),
                Associations::default(),
                session,
                Some(error.to_string()),
            );
        }
    };

    let mut error = None;
    let config = match Config::load(&paths.config_file()) {
        Ok(config) => config,
        Err(e) => {
            error = Some(e.to_string());
            Config::default()
        }
    };
    let db = match MimeDatabase::load(&paths) {
        Ok(db) => db,
        Err(e) => {
            error = Some(e.to_string());
            MimeDatabase::default()
        }
    };
    let apps = discover(&paths, &std::env::var("PATH").unwrap_or_default());
    let assoc = Associations::load(&paths).unwrap_or_default();
    let session = Session::new(paths.clone());

    (paths, config, db, apps, assoc, session, error)
}

fn fill_rows(list_box: &gtk::ListBox, items: &[String]) {
    list_box.remove_all();
    for item in items {
        let row = gtk::ListBoxRow::new();
        row.set_activatable(false);
        let label = gtk::Label::new(Some(item));
        label.set_xalign(0.0);
        label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        label.set_margin_top(5);
        label.set_margin_bottom(5);
        label.set_margin_start(12);
        label.set_margin_end(12);
        row.set_child(Some(&label));
        list_box.append(&row);
    }
}

/// Select a row and scroll it into view (vim-style cursor follows the window).
fn select_and_scroll(list_box: &gtk::ListBox, scroll: &gtk::ScrolledWindow, index: usize) {
    let Some(row) = list_box.row_at_index(index as i32) else {
        return;
    };
    list_box.select_row(Some(&row));
    if let Some(bounds) = row.compute_bounds(list_box) {
        let adjustment = scroll.vadjustment();
        let top = bounds.y() as f64;
        adjustment.clamp_page(top, top + bounds.height() as f64);
    }
}

fn install_key_controller(root: &gtk::ApplicationWindow, sender: ComponentSender<AppModel>) {
    let controller = gtk::EventControllerKey::new();
    controller.connect_key_pressed(move |_, key, _code, state| {
        if let Some(key) = translate_key(key, state) {
            sender.input(AppMsg::Key(key));
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    });
    root.add_controller(controller);
}

fn translate_key(key: gdk::Key, state: gdk::ModifierType) -> Option<UiKey> {
    let ctrl = state.contains(gdk::ModifierType::CONTROL_MASK);
    match key.name().as_deref() {
        Some("Return") | Some("KP_Enter") => return Some(UiKey::Enter),
        Some("Escape") => return Some(UiKey::Esc),
        Some("BackSpace") => return Some(UiKey::Backspace),
        Some("Tab") | Some("ISO_Left_Tab") => return Some(UiKey::Tab),
        Some("Up") => return Some(UiKey::Up),
        Some("Down") => return Some(UiKey::Down),
        _ => {}
    }
    let ch = key.to_unicode()?;
    if ctrl {
        Some(UiKey::Ctrl(ch.to_ascii_lowercase()))
    } else {
        Some(UiKey::Char(ch))
    }
}

fn install_css() {
    let Some(display) = gdk::Display::default() else {
        return;
    };
    let provider = gtk::CssProvider::new();
    provider.load_from_data(&theme::MELANGE_DARK.stylesheet());
    gtk::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}

/// Run the application.
pub fn run() {
    let app = RelmApp::new("dev.mimo_manager.MimoManager");
    app.run::<AppModel>(());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translate_control_letters() {
        assert_eq!(
            translate_key(
                gdk::Key::from_name("d").unwrap(),
                gdk::ModifierType::CONTROL_MASK
            ),
            Some(UiKey::Ctrl('d'))
        );
    }

    #[test]
    fn translate_special_keys() {
        assert_eq!(
            translate_key(
                gdk::Key::from_name("Return").unwrap(),
                gdk::ModifierType::empty()
            ),
            Some(UiKey::Enter)
        );
        assert_eq!(
            translate_key(
                gdk::Key::from_name("Escape").unwrap(),
                gdk::ModifierType::empty()
            ),
            Some(UiKey::Esc)
        );
    }

    #[test]
    fn translate_plain_char() {
        assert_eq!(
            translate_key(
                gdk::Key::from_name("j").unwrap(),
                gdk::ModifierType::empty()
            ),
            Some(UiKey::Char('j'))
        );
    }
}
