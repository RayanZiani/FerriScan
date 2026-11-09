//! Spike M-G0 (ADR-0005): volumes, 100k-row virtual list, 5k-tile treemap, FPS counter.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Instant,
};

use ferriscan_core::layout::squarify::{Rect, squarify};
use gpui::{
    App, AppContext, Bounds, Context, FocusHandle, InteractiveElement, IntoElement, MouseButton,
    MouseDownEvent, MouseMoveEvent, ParentElement, Pixels, Render, Styled, Window, WindowOptions,
    canvas, div, fill, point, px, rgb, size, uniform_list,
};

const ROWS: usize = 100_000;
const TILES: usize = 5_000;

/// Layout cache keyed by drawing size, shared by paint and hit-testing.
type Layout = Rc<RefCell<(f32, f32, Vec<Rect>)>>;

fn layout(cache: &Layout, weights: &[f64], w: f32, h: f32) -> Vec<Rect> {
    let mut c = cache.borrow_mut();
    if c.0 != w || c.1 != h {
        *c = (
            w,
            h,
            squarify(
                weights,
                Rect {
                    x: 0.0,
                    y: 0.0,
                    w,
                    h,
                },
            ),
        );
    }
    c.2.clone()
}

struct Root {
    volumes: Vec<String>,
    weights: Rc<Vec<f64>>,
    cache: Layout,
    bounds: Rc<Cell<Bounds<Pixels>>>,
    focus: FocusHandle,
    hover: Option<usize>,
    selected: usize,
    last: Instant,
    fps: f32,
}

/// Deterministic pseudo-sizes, descending as `squarify` expects.
fn weights() -> Vec<f64> {
    (1..=TILES).map(|i| 1e9 / (i as f64).powf(1.1)).collect()
}

impl Root {
    /// Tile under a window-space point, using the same cached layout as paint.
    fn hit(&self, p: gpui::Point<Pixels>) -> Option<usize> {
        let b = self.bounds.get();
        let (x, y) = (f32::from(p.x - b.origin.x), f32::from(p.y - b.origin.y));
        let rects = layout(
            &self.cache,
            &self.weights,
            f32::from(b.size.width),
            f32::from(b.size.height),
        );
        rects
            .iter()
            .position(|r| x >= r.x && x < r.x + r.w && y >= r.y && y < r.y + r.h)
    }
}

impl Render for Root {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let now = Instant::now();
        let dt = now.duration_since(self.last).as_secs_f32().max(1e-4);
        self.fps = self.fps * 0.9 + (1.0 / dt) * 0.1;
        self.last = now;
        eprintln!("fps={:.0}", self.fps); // spike-only measurement
        // Continuous redraw so the counter measures real frame cost.
        window.request_animation_frame();
        let _ = cx;

        let weights = self.weights.clone();
        let cache = self.cache.clone();
        let bounds_cell = self.bounds.clone();
        let (hover, selected) = (self.hover, self.selected);
        div()
            .size_full()
            .bg(rgb(0x1e1e2e))
            .text_color(rgb(0xcdd6f4))
            .flex()
            .flex_col()
            .child(div().p(px(8.)).child(format!(
                "{:.0} fps | tile {} (hover {:?}) | {}",
                self.fps,
                self.selected,
                self.hover,
                self.volumes.join("  |  ")
            )))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .child(
                        uniform_list("rows", ROWS, |range, _, _| {
                            range
                                .map(|i| {
                                    div()
                                        .h(px(22.))
                                        .px(px(8.))
                                        .child(format!("file_{i:06}.bin"))
                                })
                                .collect::<Vec<_>>()
                        })
                        .w(px(260.))
                        .h_full(),
                    )
                    .child(
                        div()
                            .id("treemap")
                            .flex_1()
                            .h_full()
                            .track_focus(&self.focus)
                            .on_key_down(cx.listener(|this, ev: &gpui::KeyDownEvent, _, cx| {
                                let last = TILES - 1;
                                match ev.keystroke.key.as_str() {
                                    "right" | "down" => {
                                        this.selected = (this.selected + 1).min(last)
                                    }
                                    "left" | "up" => {
                                        this.selected = this.selected.saturating_sub(1)
                                    }
                                    "home" => this.selected = 0,
                                    "end" => this.selected = last,
                                    _ => return,
                                }
                                cx.notify();
                            }))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, ev: &MouseDownEvent, window, cx| {
                                    window.focus(&this.focus, cx);
                                    if let Some(i) = this.hit(ev.position) {
                                        this.selected = i;
                                    }
                                    cx.notify();
                                }),
                            )
                            .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, _, cx| {
                                let h = this.hit(ev.position);
                                if h != this.hover {
                                    this.hover = h;
                                    cx.notify();
                                }
                            }))
                            .child(
                                canvas(
                                    |_, _, _| (),
                                    move |bounds: Bounds<Pixels>, (), window, _| {
                                        bounds_cell.set(bounds);
                                        let rects = layout(
                                            &cache,
                                            &weights,
                                            f32::from(bounds.size.width),
                                            f32::from(bounds.size.height),
                                        );
                                        for (i, r) in rects.iter().enumerate() {
                                            let shade = (i * 37 % 255) as u32;
                                            let color = if Some(i) == hover || i == selected {
                                                rgb(0xf9e2af)
                                            } else {
                                                rgb(0x203040 + (shade << 8) + (shade / 2))
                                            };
                                            window.paint_quad(fill(
                                                Bounds::new(
                                                    bounds.origin + point(px(r.x), px(r.y)),
                                                    size(
                                                        px((r.w - 1.0).max(0.0)),
                                                        px((r.h - 1.0).max(0.0)),
                                                    ),
                                                ),
                                                color,
                                            ));
                                        }
                                    },
                                )
                                .size_full(),
                            ),
                    ),
            )
    }
}

fn main() {
    let volumes = ferriscan_platform::list_volumes()
        .into_iter()
        .map(|v| {
            let gib = |b: u64| b as f64 / (1u64 << 30) as f64;
            format!(
                "{} {:.0}/{:.0} GiB free",
                v.mount_point.display(),
                gib(v.available_bytes),
                gib(v.total_bytes)
            )
        })
        .collect();
    gpui_platform::application().run(move |cx: &mut App| {
        cx.open_window(WindowOptions::default(), |_, cx| {
            cx.new(|cx| Root {
                volumes,
                weights: Rc::new(weights()),
                cache: Layout::default(),
                bounds: Rc::default(),
                focus: cx.focus_handle(),
                hover: None,
                selected: 0,
                last: Instant::now(),
                fps: 60.0,
            })
        })
        .expect("open window");
        cx.activate(true);
    });
}
