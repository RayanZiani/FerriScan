#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! `FerriScan` native shell (ADR-0005, `GPUI`).

mod format;
mod results;
mod shell;
mod theme;
mod treemap;

use gpui::{App, AppContext, WindowOptions};

fn main() {
    gpui_platform::application().run(|cx: &mut App| {
        cx.open_window(WindowOptions::default(), |_, cx| cx.new(shell::Shell::new))
            .expect("open window");
        cx.activate(true);
    });
}
