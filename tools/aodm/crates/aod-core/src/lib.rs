//! Core of `aodm`, the android-on-debian phone image manager.
//!
//! No terminal I/O lives here: the CLI/TUI (`aodm`), the standalone phone runtime
//! (`aod-phone`) and a later GUI all build on this crate.

pub mod avd;
pub mod card;
pub mod env;
pub mod error;
pub mod ini;
pub mod log;
pub mod proc;
pub mod registry;

pub use error::{Error, Result};
