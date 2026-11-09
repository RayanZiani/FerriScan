//! FerriScan native shell (ADR-0005, GPUI).

mod format;
mod shell;
mod theme;

use gpui::{App, AppContext, WindowOptions};

fn main() {
    gpui_platform::application().run(|cx: &mut App| {
        cx.open_window(WindowOptions::default(), |_, cx| cx.new(shell::Shell::new))
            .expect("open window");
        cx.activate(true);
    });
}
