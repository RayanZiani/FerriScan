//! Application shell: toolbar, volume list, status bar (parity with `AppShell.svelte`).

use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

use ferriscan_core::{
    model::NodeId,
    model::{Snapshot, SnapshotStatus},
    scan::{ScanOptions, controller::CancelToken, progress::Progress, scan},
};
use ferriscan_i18n::Catalog;
use ferriscan_persist::{Settings, app_dirs};
use ferriscan_platform::volumes::{Volume, VolumeKind};
use gpui::{
    Context, InteractiveElement, IntoElement, ParentElement, Render, StatefulInteractiveElement,
    Styled, Window, div, px, relative, uniform_list,
};

use gpui::prelude::FluentBuilder as _;

use crate::{format, results::Results, theme::Theme};

/// What the status bar shows about the scan.
enum ScanUi {
    Idle,
    Running { started: Instant },
    Done(Arc<Snapshot>),
    Failed(String),
}

/// A transient message; removed after [`TOAST_LIFETIME`] or when clicked.
struct Toast {
    id: u64,
    text: String,
    error: bool,
}

const TOAST_LIFETIME: Duration = Duration::from_secs(5);

/// Root view. Holds everything the shell renders; no OS calls happen in `render`.
pub struct Shell {
    settings: Settings,
    catalog: Catalog,
    volumes: Vec<Volume>,
    loading: bool,
    scan: ScanUi,
    toasts: Vec<Toast>,
    next_toast: u64,
    results: Option<Results>,
    show_results: bool,
    progress: Arc<Progress>,
    cancel: Arc<CancelToken>,
}

impl Shell {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let dirs = app_dirs("FerriScan");
        let mut shell = Self {
            settings: Settings::load(&dirs.settings_file()),
            catalog: Catalog::new(),
            volumes: Vec::new(),
            loading: false,
            scan: ScanUi::Idle,
            toasts: Vec::new(),
            next_toast: 0,
            results: None,
            show_results: false,
            progress: Arc::new(Progress::new()),
            cancel: Arc::new(CancelToken::new()),
        };
        shell.refresh(cx);
        shell
    }

    /// Re-reads volumes off the UI thread; the list is replaced atomically when done.
    fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        self.loading = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let volumes = cx
                .background_executor()
                .spawn(async { ferriscan_platform::list_volumes() })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.volumes = volumes;
                this.loading = false;
                cx.notify();
            });
        })
        .detach();
    }
}

impl Shell {
    /// Scans `root` on a worker thread; the UI polls the lock-free counters at 10 Hz.
    fn start_scan(&mut self, root: PathBuf, cx: &mut Context<Self>) {
        if matches!(self.scan, ScanUi::Running { .. }) {
            return;
        }
        self.progress.reset();
        self.cancel.reset();
        self.scan = ScanUi::Running {
            started: Instant::now(),
        };
        cx.notify();

        let options = ScanOptions {
            include_hidden: self.settings.include_hidden,
            same_filesystem_only: self.settings.same_filesystem_only,
            threads: usize::from(self.settings.threads),
            excludes: self.settings.excludes.clone(),
            ..ScanOptions::default()
        };
        let (progress, cancel) = (Arc::clone(&self.progress), Arc::clone(&self.cancel));
        let task = cx.background_executor().spawn(async move {
            let reader = ferriscan_platform::PlatformDirReader::new();
            scan(&reader, &root, &options, &progress, &cancel)
        });
        cx.spawn(async move |this, cx| {
            let ticker = cx.spawn({
                let this = this.clone();
                async move |cx| {
                    while this.update(cx, |_, cx| cx.notify()).is_ok() {
                        cx.background_executor()
                            .timer(Duration::from_millis(100))
                            .await;
                    }
                }
            });
            let result = task.await;
            drop(ticker);
            let _ = this.update(cx, |this, cx| {
                let (text, error) = match &result {
                    Ok(s) if s.status == SnapshotStatus::Cancelled => {
                        (this.catalog.t("scan.cancelled").to_string(), false)
                    }
                    Ok(_) => (this.catalog.t("scan.ready").to_string(), false),
                    Err(e) => (format!("{}: {e}", this.catalog.t("scan.failed")), true),
                };
                this.scan = match result {
                    Ok(snapshot) => {
                        let snapshot = Arc::new(snapshot);
                        if snapshot.status != SnapshotStatus::Cancelled {
                            this.results = Some(Results::new(Arc::clone(&snapshot)));
                            this.show_results = true;
                        }
                        ScanUi::Done(snapshot)
                    }
                    Err(err) => ScanUi::Failed(err.to_string()),
                };
                this.push_toast(text, error, cx);
            });
        })
        .detach();
    }

    /// Shows `text` for [`TOAST_LIFETIME`]; the timer is dropped with the view.
    fn push_toast(&mut self, text: String, error: bool, cx: &mut Context<Self>) {
        let id = self.next_toast;
        self.next_toast += 1;
        self.toasts.push(Toast { id, text, error });
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(TOAST_LIFETIME).await;
            let _ = this.update(cx, |this, cx| this.dismiss_toast(id, cx));
        })
        .detach();
    }

    fn open_node(&mut self, id: NodeId, cx: &mut Context<Self>) {
        if let Some(r) = &mut self.results {
            r.open(id);
            cx.notify();
        }
    }

    fn go_up(&mut self, cx: &mut Context<Self>) {
        if let Some(r) = &mut self.results {
            r.up();
            cx.notify();
        }
    }

    fn render_results(&self, th: Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let units = self.settings.units;
        let row_h = if self.settings.compact_density {
            px(24.)
        } else {
            px(30.)
        };
        let t = |key: &str| self.catalog.t(key).to_string();
        let (path, at_root, count) = self.results.as_ref().map_or_else(
            || (String::new(), true, 0),
            |r| (r.path().display().to_string(), r.at_root(), r.rows.len()),
        );
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .p(px(16.))
            .gap(px(8.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .id("up")
                            .px(px(8.))
                            .py(px(2.))
                            .rounded(px(4.))
                            .bg(th.surface_3)
                            .when(!at_root, |d| {
                                d.cursor_pointer()
                                    .on_click(cx.listener(|this, _, _, cx| this.go_up(cx)))
                            })
                            .when(at_root, |d| d.opacity(0.4))
                            .child(t("table.up")),
                    )
                    .child(path),
            )
            .child(
                div()
                    .flex()
                    .px(px(8.))
                    .text_color(th.text_muted)
                    .child(div().flex_1().child(t("table.name")))
                    .child(div().w(px(100.)).child(t("table.size")))
                    .child(div().w(px(160.)).child(""))
                    .child(div().w(px(80.)).child(t("table.items"))),
            )
            .when(count == 0, |d| {
                d.child(div().text_color(th.text_muted).child(t("table.empty")))
            })
            .child(
                uniform_list(
                    "rows",
                    count,
                    cx.processor(move |this, range: std::ops::Range<usize>, _, cx| {
                        let Some(r) = &this.results else {
                            return Vec::new();
                        };
                        let store = &r.snapshot.store;
                        let total = store.get(r.current).size.max(1);
                        range
                            .filter_map(|i| r.rows.get(i).copied())
                            .map(|id| {
                                let rec = store.get(id);
                                let is_dir = rec.flags().is_dir();
                                let frac = rec.size as f32 / total as f32;
                                div()
                                    .id(("row", id.raw() as usize))
                                    .flex()
                                    .items_center()
                                    .h(row_h)
                                    .px(px(8.))
                                    .rounded(px(4.))
                                    .when(is_dir, |d| {
                                        d.cursor_pointer().hover(|d| d.bg(th.surface_2)).on_click(
                                            cx.listener(move |this, _, _, cx| {
                                                this.open_node(id, cx);
                                            }),
                                        )
                                    })
                                    .child(div().flex_1().child(format!(
                                        "{}{}",
                                        if is_dir { "\u{25B8} " } else { "" },
                                        store.name(id)
                                    )))
                                    .child(div().w(px(100.)).child(format::bytes(rec.size, units)))
                                    .child(
                                        div().w(px(160.)).pr(px(12.)).child(
                                            div()
                                                .h(px(6.))
                                                .w_full()
                                                .rounded(px(3.))
                                                .bg(th.surface_3)
                                                .child(
                                                    div()
                                                        .h_full()
                                                        .w(relative(frac.clamp(0.0, 1.0)))
                                                        .rounded(px(3.))
                                                        .bg(th.accent),
                                                ),
                                        ),
                                    )
                                    .child(div().w(px(80.)).child(if is_dir {
                                        rec.child_count.to_string()
                                    } else {
                                        String::new()
                                    }))
                            })
                            .collect()
                    }),
                )
                .flex_1(),
            )
    }

    fn dismiss_toast(&mut self, id: u64, cx: &mut Context<Self>) {
        self.toasts.retain(|t| t.id != id);
        cx.notify();
    }

    fn status_text(&self) -> String {
        let c = &self.catalog;
        let units = self.settings.units;
        match &self.scan {
            ScanUi::Idle => c.t_args(
                "status.volumes",
                &[("count", &self.volumes.len().to_string())],
            ),
            ScanUi::Running { started } => {
                let p = self.progress.snapshot();
                let secs = started.elapsed().as_secs_f64().max(0.001);
                let rate = format::bytes((p.bytes as f64 / secs) as u64, units);
                format!(
                    "{} · {} · {} · {}",
                    c.t("scan.scanning"),
                    c.t_args("scan.progress.items", &[("count", &p.items.to_string())]),
                    c.t_args(
                        "scan.progress.bytes",
                        &[("size", &format::bytes(p.bytes, units))]
                    ),
                    c.t_args("scan.progress.rate", &[("rate", &rate)]),
                )
            }
            ScanUi::Done(s) => {
                let m = &s.meta;
                format!(
                    "{}: {} · {}: {} · {}: {} · {}: {:.1}s · {}: {}",
                    c.t("scan.summary.files"),
                    m.file_count,
                    c.t("scan.summary.folders"),
                    m.dir_count,
                    c.t("scan.summary.size"),
                    format::bytes(m.logical_bytes, units),
                    c.t("scan.summary.duration"),
                    m.duration.as_secs_f64(),
                    c.t("scan.summary.errors"),
                    m.error_count,
                )
            }
            ScanUi::Failed(e) => format!("{}: {e}", c.t("scan.failed")),
        }
    }
}

fn kind_key(kind: VolumeKind) -> &'static str {
    match kind {
        VolumeKind::Fixed => "kind.fixed",
        VolumeKind::Removable => "kind.removable",
        VolumeKind::Network => "kind.network",
        VolumeKind::Optical => "kind.optical",
        VolumeKind::Ram => "kind.ram",
        VolumeKind::Unknown => "kind.unknown",
    }
}

impl Render for Shell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let th = Theme::resolve(self.settings.theme, window.appearance());
        let units = self.settings.units;
        let row_h = if self.settings.compact_density {
            px(24.)
        } else {
            px(30.)
        };
        let t = |key: &str| self.catalog.t(key).to_string();

        let rows: Vec<_> = self
            .volumes
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let root = v.mount_point.clone();
                let used = v.total_bytes.saturating_sub(v.available_bytes);
                let frac = if v.total_bytes == 0 {
                    0.0
                } else {
                    used as f32 / v.total_bytes as f32
                };
                let free = self.catalog.t_args(
                    "volumes.free_of",
                    &[
                        ("free", &format::bytes(v.available_bytes, units)),
                        ("total", &format::bytes(v.total_bytes, units)),
                    ],
                );
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .p(px(12.))
                    .min_h(row_h)
                    .rounded(px(8.))
                    .bg(th.surface)
                    .border_1()
                    .border_color(th.border)
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .child(format!("{}  {}", v.mount_point.display(), v.label))
                            .child(
                                div()
                                    .flex()
                                    .gap(px(12.))
                                    .items_center()
                                    .child(
                                        div().text_color(th.text_muted).child(t(kind_key(v.kind))),
                                    )
                                    .child(
                                        div()
                                            .id(("scan", i))
                                            .px(px(10.))
                                            .py(px(2.))
                                            .rounded(px(4.))
                                            .bg(th.accent)
                                            .text_color(th.text_inverse)
                                            .cursor_pointer()
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.start_scan(root.clone(), cx);
                                            }))
                                            .child(t("volumes.scan")),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .h(px(6.))
                            .w_full()
                            .rounded(px(3.))
                            .bg(th.surface_3)
                            .child(
                                div()
                                    .h_full()
                                    .w(relative(frac))
                                    .rounded(px(3.))
                                    .bg(if frac > 0.9 { th.danger } else { th.accent }),
                            ),
                    )
                    .child(div().text_color(th.text_muted).child(free))
            })
            .collect();

        let status = self.status_text();
        let running = matches!(self.scan, ScanUi::Running { .. });
        let show_results = self.show_results && self.results.is_some();
        let toasts: Vec<_> = self
            .toasts
            .iter()
            .map(|toast| {
                let id = toast.id;
                div()
                    .id(("toast", id))
                    .px(px(12.))
                    .py(px(8.))
                    .rounded(px(8.))
                    .bg(th.surface)
                    .border_1()
                    .border_color(if toast.error { th.danger } else { th.border })
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| this.dismiss_toast(id, cx)))
                    .child(toast.text.clone())
            })
            .collect();

        div()
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(th.bg)
            .text_color(th.text)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px(px(16.))
                    .py(px(8.))
                    .bg(th.surface)
                    .border_b_1()
                    .border_color(th.border)
                    .child(t("app.name"))
                    .when(self.results.is_some(), |d| {
                        let tab = |id: &'static str, label: String, active: bool, results: bool| {
                            div()
                                .id(id)
                                .px(px(10.))
                                .py(px(2.))
                                .rounded(px(4.))
                                .cursor_pointer()
                                .when(active, |d| d.bg(th.surface_3))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.show_results = results;
                                    cx.notify();
                                }))
                                .child(label)
                        };
                        d.child(
                            div()
                                .flex()
                                .gap(px(4.))
                                .child(tab(
                                    "tab-volumes",
                                    t("nav.volumes"),
                                    !self.show_results,
                                    false,
                                ))
                                .child(tab(
                                    "tab-results",
                                    t("nav.results"),
                                    self.show_results,
                                    true,
                                )),
                        )
                    })
                    .child(
                        div()
                            .id("refresh")
                            .px(px(12.))
                            .py(px(4.))
                            .rounded(px(4.))
                            .bg(th.accent)
                            .text_color(th.text_inverse)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, _, cx| this.refresh(cx)))
                            .child(t("volumes.refresh")),
                    ),
            )
            .child(if show_results {
                self.render_results(th, cx).into_any_element()
            } else {
                div()
                    .id("volumes")
                    .flex_1()
                    .overflow_y_scroll()
                    .p(px(16.))
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(div().child(t("volumes.title")))
                    .when(self.volumes.is_empty() && !self.loading, |d| {
                        d.child(div().text_color(th.text_muted).child(t("volumes.empty")))
                    })
                    .when(self.loading && self.volumes.is_empty(), |d| {
                        d.child(t("common.loading"))
                    })
                    .children(rows)
                    .into_any_element()
            })
            .child(
                div()
                    .px(px(16.))
                    .py(px(4.))
                    .bg(th.surface_2)
                    .border_t_1()
                    .border_color(th.border)
                    .text_color(th.text_muted)
                    .flex()
                    .justify_between()
                    .child(status)
                    .when(running, |d| {
                        d.child(
                            div()
                                .id("cancel")
                                .cursor_pointer()
                                .text_color(th.danger)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.cancel.cancel();
                                    cx.notify();
                                }))
                                .child(t("scan.cancel")),
                        )
                    }),
            )
            .child(
                div()
                    .absolute()
                    .bottom(px(40.))
                    .right(px(16.))
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .children(toasts),
            )
    }
}
