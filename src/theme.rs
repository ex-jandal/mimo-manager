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

impl Palette {
    /// The GTK stylesheet for this palette.
    pub fn stylesheet(&self) -> String {
        format!(
            "window {{ background-color: {bg}; color: {fg}; }}\n\
             .mono, .mono label {{ font-family: \"CaskaydiaCove Nerd Font Mono\", monospace; font-size: 11pt; }}\n\
             .pane {{ background-color: {bg}; }}\n\
             .pane row:selected {{ background-color: {selection}; color: {fg}; }}\n\
             .commandline {{ background-color: {bg_alt}; color: {fg_alt}; padding: 2px 10px; }}\n\
             .modeline {{ background-color: {bg_alt}; padding: 3px 10px; }}\n\
             .modeline label {{ font-size: 10pt; }}\n\
             .detail-title {{ background-color: {bg_alt}; color: {yellow}; padding: 5px 10px; }}\n",
            bg = self.bg,
            bg_alt = self.bg_alt,
            selection = self.selection,
            fg = self.fg,
            fg_alt = self.fg_alt,
            yellow = self.yellow,
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

/// Render the modeline as Pango markup, honouring the enabled fields.
pub fn render_markup(fields: &ModelineFields, modeline: &Modeline) -> String {
    let palette = MELANGE_DARK;
    let mut parts: Vec<String> = Vec::new();
    let mut push = |enabled: bool, colour: &str, text: &str| {
        if enabled && !text.is_empty() {
            parts.push(span(colour, text));
        }
    };

    push(fields.mode, palette.green_bright, &modeline.mode);
    push(fields.workspace, palette.blue, &modeline.workspace);
    push(fields.selection, palette.fg, &modeline.selection);
    push(fields.count, palette.fg_dim, &modeline.count);
    push(fields.default, palette.yellow, &modeline.default);
    push(fields.scope, palette.cyan, &modeline.scope);
    push(fields.status, palette.red, &modeline.status);
    push(fields.theme, palette.magenta, &modeline.theme);

    let separator = span(palette.fg_dim, " | ");
    parts.join(&separator)
}

fn span(colour: &str, text: &str) -> String {
    format!("<span foreground=\"{colour}\">{}</span>", escape(text))
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
        let markup = render_markup(&ModelineFields::default(), &modeline());
        assert!(markup.contains("NORMAL"));
        assert!(markup.contains("MIME TYPES"));
        assert!(markup.contains("#85B695"), "mode uses the bright green");
        assert!(markup.contains("set default"));
    }

    #[test]
    fn disabled_fields_are_omitted() {
        let fields = ModelineFields {
            status: false,
            scope: false,
            ..ModelineFields::default()
        };
        let markup = render_markup(&fields, &modeline());
        assert!(!markup.contains("set default"));
        assert!(!markup.contains("SCOPE"));
        assert!(markup.contains("NORMAL"));
    }

    #[test]
    fn theme_field_is_off_by_default_but_renderable() {
        let off = render_markup(&ModelineFields::default(), &modeline());
        assert!(!off.contains(THEME_NAME));

        let fields = ModelineFields {
            theme: true,
            ..ModelineFields::default()
        };
        let on = render_markup(&fields, &modeline());
        assert!(on.contains(THEME_NAME));
    }

    #[test]
    fn markup_is_escaped() {
        let mut m = modeline();
        m.status = "<bad> & worse".into();
        let markup = render_markup(&ModelineFields::default(), &m);
        assert!(markup.contains("&lt;bad&gt; &amp; worse"));
    }

    #[test]
    fn stylesheet_uses_palette_colours() {
        let css = MELANGE_DARK.stylesheet();
        assert!(css.contains(MELANGE_DARK.bg));
        assert!(css.contains(MELANGE_DARK.bg_alt));
    }
}
