//! Melange Dark palette, stylesheet generation and modeline rendering.
//!
//! The palette mirrors upstream `savq/melange-nvim` (`lua/melange/palettes/
//! dark.lua`); keeping it in one place lets the GTK theme and the modeline
//! colours share a single source of truth.

/// A colour palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    pub bg: &'static str,
    pub bg_alt: &'static str,
    pub selection: &'static str,
    pub fg: &'static str,
    pub fg_alt: &'static str,
    pub fg_dim: &'static str,
    pub red: &'static str,
    pub red_bright: &'static str,
    pub yellow: &'static str,
    pub yellow_bright: &'static str,
    pub green: &'static str,
    pub green_bright: &'static str,
    pub blue: &'static str,
    pub blue_bright: &'static str,
    pub cyan: &'static str,
    pub cyan_bright: &'static str,
    pub magenta: &'static str,
    pub magenta_bright: &'static str,
}

/// The canonical Melange Dark palette.
pub const MELANGE_DARK: Palette = Palette {
    bg: "#292522",
    bg_alt: "#34302C",
    selection: "#403A36",
    fg: "#ECE1D7",
    fg_alt: "#C1A78E",
    fg_dim: "#867462",
    red: "#BD8183",
    red_bright: "#D47766",
    yellow: "#E49B5D",
    yellow_bright: "#EBC06D",
    green: "#78997A",
    green_bright: "#85B695",
    blue: "#7F91B2",
    blue_bright: "#A3A9CE",
    cyan: "#7B9695",
    cyan_bright: "#89B3B6",
    magenta: "#B380B0",
    magenta_bright: "#CF9BC2",
};

pub const THEME_NAME: &str = "MELANGE DARK";

/// The themes this build knows how to render.
pub const THEMES: &[&str] = &[THEME_NAME];

/// Whether `name` names a theme this build can render.
pub fn is_known(name: &str) -> bool {
    THEMES.contains(&name)
}

/// UI scale for text and rows, relative to the system default font size.
///
/// Expressed in `em` so it compounds with the user's system scaling rather
/// than overriding it.
pub const FONT_SCALE: &str = "1.35em";

impl Palette {
    /// The GTK stylesheet for this palette.
    pub fn stylesheet(&self) -> String {
        format!(
            "window {{ background-color: {bg}; color: {fg}; }}\n\
             .mono {{ font-family: \"CaskaydiaCove Nerd Font Mono\", monospace; font-size: {font_scale}; }}\n\
             .mono label {{ font-family: \"CaskaydiaCove Nerd Font Mono\", monospace; }}\n\
             .pane {{ background-color: {bg}; }}\n\
             .pane row:selected {{ background-color: {selection}; color: {fg}; }}\n\
             .commandline {{ background-color: {bg_alt}; color: {fg_alt}; padding: 4px 12px; }}\n\
             .modeline {{ background-color: {bg_alt}; padding: 5px 12px; }}\n\
             .workstabs {{ background-color: {bg}; padding: 4px 12px; border-top: 1px solid {selection}; }}\n\
             .detail-title {{ background-color: {bg_alt}; color: {yellow}; padding: 7px 12px; }}\n",
            bg = self.bg,
            bg_alt = self.bg_alt,
            selection = self.selection,
            fg = self.fg,
            fg_alt = self.fg_alt,
            yellow = self.yellow,
            font_scale = FONT_SCALE,
        )
    }
}

/// Which modeline fields are shown. All fields are toggleable via config.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelineFields {
    pub mode: bool,
    pub workspace: bool,
    pub selection: bool,
    pub count: bool,
    pub default: bool,
    pub scope: bool,
    pub status: bool,
    pub theme: bool,
}

impl Default for ModelineFields {
    fn default() -> Self {
        Self {
            mode: true,
            workspace: true,
            selection: true,
            count: true,
            default: true,
            scope: true,
            status: true,
            theme: false,
        }
    }
}

/// The values rendered into the modeline.
#[derive(Debug, Clone, Default)]
pub struct Modeline {
    pub mode: String,
    pub workspace: String,
    pub selection: String,
    pub count: String,
    pub default: String,
    pub scope: String,
    pub status: String,
    pub theme: String,
}

/// Render the modeline as two Pango markup strings: the left group (state and
/// buffer context) and the right group (position and status). The interface
/// justifies them to opposite edges, Doom-style.
pub fn render(fields: &ModelineFields, modeline: &Modeline) -> (String, String) {
    let palette = MELANGE_DARK;

    fn push(parts: &mut Vec<String>, enabled: bool, colour: &str, text: &str) {
        if enabled && !text.is_empty() {
            parts.push(span(colour, text));
        }
    }

    let mut left = Vec::new();
    push(&mut left, fields.mode, palette.green_bright, &modeline.mode);
    push(
        &mut left,
        fields.workspace,
        palette.blue,
        &modeline.workspace,
    );
    push(&mut left, fields.default, palette.yellow, &modeline.default);
    push(&mut left, fields.scope, palette.cyan, &modeline.scope);

    let mut right = Vec::new();
    push(
        &mut right,
        fields.selection,
        palette.fg,
        &modeline.selection,
    );
    push(&mut right, fields.count, palette.fg_dim, &modeline.count);
    push(&mut right, fields.status, palette.red, &modeline.status);
    push(&mut right, fields.theme, palette.magenta, &modeline.theme);

    let separator = span(palette.fg_dim, " | ");
    (left.join(&separator), right.join(&separator))
}

fn span(colour: &str, text: &str) -> String {
    format!("<span foreground=\"{colour}\">{}</span>", escape(text))
}

/// Render the workspace tab strip below the modeline, numbering each tab and
/// highlighting the active one.
pub fn workspace_tabs(labels: &[&str], active: usize) -> String {
    let palette = MELANGE_DARK;
    labels
        .iter()
        .enumerate()
        .map(|(index, label)| {
            let (colour, weight) = if index == active {
                (palette.green_bright, "bold")
            } else {
                (palette.fg_dim, "normal")
            };
            format!(
                "<span foreground=\"{colour}\" weight=\"{weight}\">{} {}</span>",
                index + 1,
                escape(label)
            )
        })
        .collect::<Vec<_>>()
        .join("   ")
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn modeline() -> Modeline {
        Modeline {
            mode: "NORMAL".into(),
            workspace: "MIME TYPES".into(),
            selection: "3/128".into(),
            count: "128".into(),
            default: "DEFAULT editor.desktop".into(),
            scope: "SCOPE user".into(),
            status: "set default".into(),
            theme: THEME_NAME.into(),
        }
    }

    #[test]
    fn default_fields_include_the_core_set() {
        let fields = ModelineFields::default();
        assert!(fields.mode && fields.workspace && fields.selection && fields.count);
        assert!(fields.default && fields.scope && fields.status);
    }

    #[test]
    fn renders_enabled_fields_with_colour() {
        let (left, right) = render(&ModelineFields::default(), &modeline());
        assert!(left.contains("NORMAL"));
        assert!(left.contains("MIME TYPES"));
        assert!(left.contains("#85B695"), "mode uses the bright green");
        assert!(right.contains("set default"));
    }

    #[test]
    fn splits_state_left_and_position_right() {
        let (left, right) = render(&ModelineFields::default(), &modeline());
        assert!(left.contains("NORMAL") && left.contains("MIME TYPES"));
        assert!(!right.contains("NORMAL"));
        assert!(right.contains("3/128") && right.contains("128"));
        assert!(!left.contains("3/128"));
    }

    #[test]
    fn disabled_fields_are_omitted() {
        let fields = ModelineFields {
            status: false,
            scope: false,
            ..ModelineFields::default()
        };
        let (left, right) = render(&fields, &modeline());
        let all = format!("{left}{right}");
        assert!(!all.contains("set default"));
        assert!(!all.contains("SCOPE"));
        assert!(all.contains("NORMAL"));
    }

    #[test]
    fn theme_field_is_off_by_default_but_renderable() {
        let (_, right) = render(&ModelineFields::default(), &modeline());
        assert!(!right.contains(THEME_NAME));

        let fields = ModelineFields {
            theme: true,
            ..ModelineFields::default()
        };
        let (_, right) = render(&fields, &modeline());
        assert!(right.contains(THEME_NAME));
    }

    #[test]
    fn markup_is_escaped() {
        let mut m = modeline();
        m.status = "<bad> & worse".into();
        let (_, right) = render(&ModelineFields::default(), &m);
        assert!(right.contains("&lt;bad&gt; &amp; worse"));
    }

    #[test]
    fn stylesheet_uses_palette_colours() {
        let css = MELANGE_DARK.stylesheet();
        assert!(css.contains(MELANGE_DARK.bg));
        assert!(css.contains(MELANGE_DARK.bg_alt));
    }

    #[test]
    fn workspace_tabs_highlight_the_active_one() {
        let tabs = workspace_tabs(&["MIME TYPES", "SIMPLE"], 1);
        assert!(tabs.contains("1 MIME TYPES") && tabs.contains("2 SIMPLE"));
        assert!(
            tabs.contains(MELANGE_DARK.green_bright),
            "the active tab is bright"
        );
        assert!(tabs.contains(MELANGE_DARK.fg_dim), "the others are dim");
    }
}
