//! Operating-system integration.
//!
//! This crate is the only place outside the Tauri shell where `unsafe` and
//! `cfg(target_os)` are allowed. It implements the engine traits
//! ([`ferriscan_core::fs::DirReader`]) and reports volumes, system theme and
//! shell actions.

#![deny(unsafe_op_in_unsafe_fn)]

pub mod dir_reader;
pub mod shell;
pub mod volumes;

mod sys;

pub use dir_reader::PlatformDirReader;
pub use sys::platform_name;
pub use volumes::{Volume, list_volumes};
