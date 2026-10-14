pub mod discovery;
pub mod entry;

pub use discovery::{DiscoveryReport, discover};
pub use entry::{first_exec_token, parse_desktop_entry, parse_ini};
