//! Binary entry point. All logic lives in the library so integration tests can
//! build the app without spawning a process.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if let Err(err) = ferriscan_app::run() {
        eprintln!("FerriScan failed to start: {err}");
        std::process::exit(1);
    }
}
