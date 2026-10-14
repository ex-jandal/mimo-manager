//! Read-only queries backed by GIO.
//!
//! GIO is authoritative for "what does the running session consider the
//! default", which can differ from any single on-disk `mimeapps.list`. Writes
//! are deliberately *not* routed through GIO; see the associations layer.

use std::path::PathBuf;

use gio::prelude::*;

/// A compact view of an application returned by GIO queries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppSummary {
    /// Desktop-file id when the app is backed by a desktop entry.
    pub id: Option<String>,
    pub name: String,
    pub executable: Option<PathBuf>,
    pub visible: bool,
}

fn summarise(info: &gio::AppInfo) -> AppSummary {
    let executable = info.executable();
    AppSummary {
        id: info.id().map(|s| s.to_string()),
        name: info.name().to_string(),
        executable: (!executable.as_os_str().is_empty()).then_some(executable),
        visible: info.should_show(),
    }
}

/// Human-readable description of a MIME type.
pub fn content_description(mime: &str) -> Option<String> {
    let content_type = gio::content_type_from_mime_type(mime)?;
    Some(gio::content_type_get_description(&content_type).to_string())
}

/// The default application for a MIME type, if any.
pub fn default_app(mime: &str) -> Option<AppSummary> {
    gio::AppInfo::default_for_type(mime, false).map(|info| summarise(&info))
}

/// Applications GIO recommends for a MIME type.
pub fn recommended_apps(mime: &str) -> Vec<AppSummary> {
    gio::AppInfo::recommended_for_type(mime)
        .iter()
        .map(summarise)
        .collect()
}

/// Every application registered for a MIME type.
pub fn all_apps_for(mime: &str) -> Vec<AppSummary> {
    gio::AppInfo::all_for_type(mime)
        .iter()
        .map(summarise)
        .collect()
}

/// All content types registered with the running GIO, as MIME names.
pub fn registered_content_types() -> Vec<String> {
    gio::content_types_get_registered()
        .iter()
        .map(|name| name.to_string())
        .collect()
}
