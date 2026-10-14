//! mimo_manager — keyboard-centric MIME type and default-application manager.
//!
//! The crate is split into a framework-independent core (XDG paths, MIME
//! database, desktop entries, associations) and a GTK4/Relm4 user interface.
//! The core never touches GTK and is exercised directly by the test suite.

pub mod assoc;
pub mod desktop;
pub mod domain;
pub mod error;
pub mod mime;
pub mod queries;
pub mod ui;
pub mod xdg;

pub use error::{Error, Result};

/// Launch the graphical application.
pub fn run() {
    ui::run();
}
