//! Core of `aodm`, the android-on-debian phone image manager.
//!
//! No terminal I/O lives here: the CLI/TUI (`aodm`), the standalone phone runtime
//! (`aod-phone`) and a later GUI all build on this crate.

pub mod adb;
pub mod avd;
pub mod budget;
pub mod card;
pub mod cmd;
pub mod console;
pub mod emu;
pub mod env;
pub mod error;
pub mod ini;
pub mod lifecycle;
pub mod log;
pub mod ports;
pub mod proc;
pub mod registry;

pub use error::{Error, Result};
