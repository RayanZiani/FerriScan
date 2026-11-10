#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! `FerriScan` native shell (ADR-0005, `GPUI`).

mod a11y;
mod format;
mod launch;
mod results;
mod shell;
mod theme;
mod treemap;

use ferriscan_persist::{Settings, app_dirs};
use gpui::{App, AppContext, Bounds, WindowBounds, WindowOptions, point, px, size};

/// Smallest saved size that is restored; anything smaller is treated as corrupt.
const MIN_SIZE: (u32, u32) = (640, 420);
/// Saved positions farther than this from the origin are ignored (a removed monitor).
const MAX_OFFSET: i32 = 8000;

/// Window rectangle from the last run, when it is plausible.
fn saved_bounds() -> Option<WindowBounds> {
    let settings = Settings::load(&app_dirs("FerriScan").settings_file());
    let (x, y, w, h) = settings.window_geometry?;
    if w < MIN_SIZE.0 || h < MIN_SIZE.1 || x.abs() > MAX_OFFSET || y.abs() > MAX_OFFSET {
        return None;
    }
    Some(WindowBounds::Windowed(Bounds::new(
        point(px(x as f32), px(y as f32)),
        size(px(w as f32), px(h as f32)),
    )))
}

fn main() {
    gpui_platform::application().run(|cx: &mut App| {
        let options = WindowOptions {
            window_bounds: saved_bounds(),
            ..WindowOptions::default()
        };
        cx.open_window(options, |window, cx| {
            let launch = launch::Launch::parse(std::env::args().skip(1));
            let shell = cx.new(|cx| shell::Shell::new(launch, cx));
            shell.update(cx, |_, cx| {
                cx.observe_window_bounds(window, |this, window, _| {
                    this.remember_bounds(window.bounds());
                })
                .detach();
            });
            let closing = shell.clone();
            window.on_window_should_close(cx, move |_, cx| {
                closing.read(cx).save_settings();
                true
            });
            shell
        })
        .expect("open window");
        cx.activate(true);
    });
}
