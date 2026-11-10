//! Application shell: toolbar, volume list, status bar.

#![allow(clippy::large_types_passed_by_value)] // Theme is Copy and moved into 'static GPUI closures

use std::{
    cell::RefCell,
    path::PathBuf,
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};

use ferriscan_actions::{ActionKind, ActionPlan, PlanStatus, execute::execute, plan::plan};
use ferriscan_core::{
    model::NodeId,
    model::{Snapshot, SnapshotStatus},
    scan::{ScanOptions, controller::CancelToken, progress::Progress, scan},
};
use ferriscan_i18n::Catalog;
use ferriscan_persist::{AppDirs, Settings, ThemeMode, Units, app_dirs};
use ferriscan_platform::volumes::{Volume, VolumeKind};
use gpui::{
    Bounds, ClipboardItem, Context, InteractiveElement, IntoElement, ParentElement, Render, Role,
    StatefulInteractiveElement, Styled, Toggled, Window, div, px, relative, uniform_list,
};

use gpui::prelude::FluentBuilder as _;

use crate::{a11y::Pressable as _, format, results::Results, theme::Theme, treemap::TreemapState};

/// Message catalogue for the language the user picked, or the system language.
fn catalog_for(settings: &Settings, session_lang: Option<&str>) -> Catalog {
    let system = ferriscan_platform::locale::system_locale();
    let chosen = session_lang.or(settings.language.as_deref());
    let tag = ferriscan_i18n::resolve_with(chosen, system.as_deref());
    Catalog::for_locale(&tag)
}

/// One selectable value of a setting: label, whether it is current, how to apply it.
type SettingChoice = (String, bool, fn(&mut Settings));

/// A validated destructive action waiting for the user's confirmation.
struct Pending {
    plan: ActionPlan,
    name: String,
    bytes: u64,
}

/// How the results panel presents the current folder.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ResultsMode {
    Table,
    Treemap,
}

/// Which main panel is visible.
#[derive(Clone, Copy, PartialEq, Eq)]
enum View {
    Volumes,
    Results,
    Settings,
}

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
    view: View,
    selected: Option<NodeId>,
    pending: Option<Pending>,
    mode: ResultsMode,
    treemap: Rc<RefCell<TreemapState>>,
    tm_hover: Option<usize>,
    dirs: AppDirs,
    launch: crate::launch::Launch,
    elevated: bool,
    /// Keyboard focus target of the size table (one tab stop, arrow keys move the selection).
    list_focus: gpui::FocusHandle,
    /// Keeps keyboard focus inside the window so Tab works before the first click.
    root_focus: gpui::FocusHandle,
    focus_ready: bool,
    list_scroll: gpui::UniformListScrollHandle,
    progress: Arc<Progress>,
    cancel: Arc<CancelToken>,
}

impl Shell {
    pub fn new(mut launch: crate::launch::Launch, cx: &mut Context<Self>) -> Self {
        let dirs = app_dirs("FerriScan");
        let settings = Settings::load(&dirs.settings_file());
        let mut shell = Self {
            dirs,
            catalog: catalog_for(&settings, launch.lang.as_deref()),
            settings,
            launch: launch.clone(),
            volumes: Vec::new(),
            loading: false,
            scan: ScanUi::Idle,
            toasts: Vec::new(),
            next_toast: 0,
            results: None,
            view: View::Volumes,
            selected: None,
            pending: None,
            mode: ResultsMode::Table,
            treemap: Rc::default(),
            tm_hover: None,
            elevated: ferriscan_platform::elevation::is_elevated(),
            list_focus: cx.focus_handle().tab_stop(true),
            root_focus: cx.focus_handle(),
            focus_ready: false,
            list_scroll: gpui::UniformListScrollHandle::new(),
            progress: Arc::new(Progress::new()),
            cancel: Arc::new(CancelToken::new()),
        };
        shell.refresh(cx);
        if launch.settings {
            shell.view = View::Settings;
        }
        if let Some(path) = launch.scan.take() {
            shell.start_scan(path, cx);
        }
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
        let elevated = self.elevated;
        let task = cx.background_executor().spawn(async move {
            // The MFT reader needs an elevated process and a whole-drive root; any failure
            // falls back to the directory walk (ADR-0006).
            #[cfg(windows)]
            let mft = elevated
                .then(|| ferriscan_platform::mft::MftReader::try_open(&root))
                .flatten();
            #[cfg(not(windows))]
            let mft: Option<ferriscan_platform::PlatformDirReader> = {
                let _ = elevated;
                None
            };
            let fast = mft.is_some();
            let result = match &mft {
                Some(reader) => scan(reader, &root, &options, &progress, &cancel),
                None => scan(
                    &ferriscan_platform::PlatformDirReader::new(),
                    &root,
                    &options,
                    &progress,
                    &cancel,
                ),
            };
            (result, fast)
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
            let (result, fast) = task.await;
            drop(ticker);
            let _ = this.update(cx, |this, cx| {
                let (text, error) = match &result {
                    Ok(s) if s.status == SnapshotStatus::Cancelled => {
                        (this.catalog.t("scan.cancelled").to_string(), false)
                    }
                    Ok(_) => (
                        this.catalog
                            .t(if fast {
                                "scan.ready_fast"
                            } else {
                                "scan.ready"
                            })
                            .to_string(),
                        false,
                    ),
                    Err(e) => (format!("{}: {e}", this.catalog.t("scan.failed")), true),
                };
                this.scan = match result {
                    Ok(snapshot) => {
                        let snapshot = Arc::new(snapshot);
                        if snapshot.status != SnapshotStatus::Cancelled {
                            this.results = Some(Results::new(Arc::clone(&snapshot)));
                            this.view = View::Results;
                            if this.launch.treemap {
                                this.mode = ResultsMode::Treemap;
                            }
                            this.selected = None;
                            this.pending = None;
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

    /// Keeps the latest window rectangle in memory; it is written once, when the window closes.
    pub fn remember_bounds(&mut self, bounds: gpui::Bounds<gpui::Pixels>) {
        let whole = |v: gpui::Pixels| f32::from(v).round() as i32;
        self.settings.window_geometry = Some((
            whole(bounds.origin.x),
            whole(bounds.origin.y),
            u32::try_from(whole(bounds.size.width)).unwrap_or(0),
            u32::try_from(whole(bounds.size.height)).unwrap_or(0),
        ));
    }

    /// Writes settings (including the window rectangle); failure is not worth interrupting a close.
    pub fn save_settings(&self) {
        let _ = self.settings.save(&self.dirs.settings_file());
    }

    /// Starts an elevated copy of the app after the OS consent prompt, then quits this one.
    fn relaunch_elevated(&mut self, cx: &mut Context<Self>) {
        if ferriscan_platform::elevation::relaunch_elevated().is_ok() {
            cx.quit();
        } else {
            let text = self.catalog.t("volumes.relaunch_failed").to_string();
            self.push_toast(text, true, cx);
        }
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

    fn select_node(&mut self, id: NodeId, cx: &mut Context<Self>) {
        self.selected = Some(id);
        self.pending = None;
        cx.notify();
    }

    /// Absolute path of a node, resolved from the snapshot (nodes are the only handle the UI keeps).
    fn node_path(&self, id: NodeId) -> Option<PathBuf> {
        let r = self.results.as_ref()?;
        Some(
            r.snapshot
                .meta
                .root_path
                .join(r.snapshot.store.relative_path(id)),
        )
    }

    fn open_selected(&mut self, reveal: bool, cx: &mut Context<Self>) {
        let Some(path) = self.selected.and_then(|id| self.node_path(id)) else {
            return;
        };
        let target = if reveal {
            path.parent()
                .map_or(path.clone(), std::path::Path::to_path_buf)
        } else {
            path
        };
        if ferriscan_platform::shell::open_path(&target).is_err() {
            let text = self.catalog.t("common.error").to_string();
            self.push_toast(text, true, cx);
        }
    }

    fn copy_selected_path(&mut self, cx: &mut Context<Self>) {
        if let Some(path) = self.selected.and_then(|id| self.node_path(id)) {
            cx.write_to_clipboard(ClipboardItem::new_string(path.display().to_string()));
        }
    }

    /// Phase one of the two-phase flow (ADR-0003): validate and show what would happen.
    fn request_trash(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.selected else { return };
        let Some(path) = self.node_path(id) else {
            return;
        };
        let (name, bytes) = self.results.as_ref().map_or_else(Default::default, |r| {
            (
                r.snapshot.store.name(id).to_string(),
                r.snapshot.store.get(id).size,
            )
        });
        match plan(ActionKind::Trash, std::slice::from_ref(&path), None) {
            Ok(p) if p.items.iter().all(|i| i.status == PlanStatus::Ready) && !p.needs_override => {
                self.pending = Some(Pending {
                    plan: p,
                    name,
                    bytes,
                });
            }
            Ok(_) => {
                let text = self.catalog.t("actions.protected").to_string();
                self.push_toast(text, true, cx);
            }
            Err(_) => {
                let text = self.catalog.t("common.error").to_string();
                self.push_toast(text, true, cx);
            }
        }
        cx.notify();
    }

    /// Phase two: run the reviewed plan off the UI thread, then rescan so the snapshot is fresh.
    fn confirm_trash(&mut self, cx: &mut Context<Self>) {
        let Some(pending) = self.pending.take() else {
            return;
        };
        let root = self
            .results
            .as_ref()
            .map(|r| r.snapshot.meta.root_path.clone());
        cx.notify();
        cx.spawn(async move |this, cx| {
            let plan = pending.plan;
            let report = cx
                .background_executor()
                .spawn(async move { execute(&plan) })
                .await;
            let _ = this.update(cx, |this, cx| {
                if report.failed > 0 {
                    let text = this
                        .catalog
                        .t_args("actions.failed", &[("count", &report.failed.to_string())]);
                    this.push_toast(text, true, cx);
                }
                if report.succeeded > 0 {
                    let text = this
                        .catalog
                        .t_args("actions.done", &[("count", &report.succeeded.to_string())]);
                    this.push_toast(text, false, cx);
                    if let Some(root) = root {
                        this.start_scan(root, cx);
                    }
                }
            });
        })
        .detach();
    }

    fn render_action_bar(&self, th: Theme, cx: &mut Context<Self>) -> Option<gpui::Div> {
        let id = self.selected?;
        let r = self.results.as_ref()?;
        let t = |key: &str| self.catalog.t(key).to_string();
        let button = |id: &'static str, label: String, danger: bool| {
            div()
                .id(id)
                .px(px(10.))
                .py(px(2.))
                .rounded(px(4.))
                .cursor_pointer()
                .pressable(Role::Button, label.clone(), th)
                .bg(if danger { th.danger } else { th.surface_3 })
                .text_color(if danger { th.text_inverse } else { th.text })
                .child(label)
        };
        let bar = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .child(div().flex_1().child(r.snapshot.store.name(id).to_string()));
        if let Some(p) = &self.pending {
            let units = self.settings.units;
            return Some(
                bar.child(div().text_color(th.danger).child(t("actions.plan_title")))
                    .child(self.catalog.t_args(
                        "actions.plan_items",
                        &[("count", &p.plan.ready_count().to_string())],
                    ))
                    .child(self.catalog.t_args(
                        "actions.plan_bytes",
                        &[("size", &format::bytes(p.bytes, units))],
                    ))
                    .child(div().child(p.name.clone()))
                    .child(
                        button("trash-run", t("actions.confirm"), true)
                            .on_click(cx.listener(|this, _, _, cx| this.confirm_trash(cx))),
                    )
                    .child(button("trash-cancel", t("actions.cancel"), false).on_click(
                        cx.listener(|this, _, _, cx| {
                            this.pending = None;
                            cx.notify();
                        }),
                    )),
            );
        }
        Some(
            bar.child(
                button("act-open", t("actions.open"), false)
                    .on_click(cx.listener(|this, _, _, cx| this.open_selected(false, cx))),
            )
            .child(
                button("act-reveal", t("actions.reveal"), false)
                    .on_click(cx.listener(|this, _, _, cx| this.open_selected(true, cx))),
            )
            .child(
                button("act-copy", t("actions.copy_path"), false)
                    .on_click(cx.listener(|this, _, _, cx| this.copy_selected_path(cx))),
            )
            .child(
                button("act-trash", t("actions.trash"), true)
                    .on_click(cx.listener(|this, _, _, cx| this.request_trash(cx))),
            ),
        )
    }

    /// Keyboard handling for the size table: arrows move the selection, Enter opens a folder,
    /// Backspace goes up and Delete asks to move the selection to the recycle bin.
    fn list_key(&mut self, key: &str, cx: &mut Context<Self>) {
        let Some(r) = &self.results else { return };
        match key {
            "down" | "up" => {
                let Some(next) = r.step(self.selected, key == "down") else {
                    return;
                };
                let id = r.rows[next];
                self.list_scroll
                    .scroll_to_item(next, gpui::ScrollStrategy::Nearest);
                self.select_node(id, cx);
            }
            "enter" => {
                if let Some(id) = self.selected
                    && r.snapshot.store.get(id).flags().is_dir()
                {
                    self.open_node(id, cx);
                }
            }
            "backspace" => self.go_up(cx),
            "delete" => self.request_trash(cx),
            _ => {}
        }
    }

    fn open_node(&mut self, id: NodeId, cx: &mut Context<Self>) {
        if let Some(r) = &mut self.results {
            r.open(id);
            self.selected = None;
            self.tm_hover = None;
            cx.notify();
        }
    }

    fn render_treemap(&self, th: Theme, cx: &mut Context<Self>) -> gpui::Div {
        let Some(results) = &self.results else {
            return div();
        };
        let snapshot = Arc::clone(&results.snapshot);
        let node = results.current;
        let state = Rc::clone(&self.treemap);
        let hover = self.tm_hover;
        let units = self.settings.units;

        let label = hover
            .and_then(|i| state.borrow().tiles.get(i).copied())
            .filter(|tile| !tile.node.is_none())
            .map(|tile| {
                format!(
                    "{}  {}",
                    snapshot.store.name(tile.node),
                    format::bytes(tile.size, units)
                )
            })
            .unwrap_or_default();

        let hit = |state: &Rc<RefCell<TreemapState>>, p: gpui::Point<gpui::Pixels>| {
            let st = state.borrow();
            let (x, y) = (
                f32::from(p.x - st.bounds.origin.x),
                f32::from(p.y - st.bounds.origin.y),
            );
            st.hit(x, y)
        };
        let (move_state, down_state) = (Rc::clone(&state), Rc::clone(&state));

        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(div().h(px(20.)).text_color(th.text_muted).child(label))
            .child(
                div()
                    .id("treemap")
                    .role(Role::Image)
                    .aria_label(self.catalog.t("view.treemap").to_string())
                    .aria_description(self.catalog.t("view.treemap_hint").to_string())
                    .flex_1()
                    .on_mouse_move(cx.listener(move |this, ev: &gpui::MouseMoveEvent, _, cx| {
                        let h = hit(&move_state, ev.position);
                        if h != this.tm_hover {
                            this.tm_hover = h;
                            cx.notify();
                        }
                    }))
                    .on_mouse_down(
                        gpui::MouseButton::Left,
                        cx.listener(move |this, ev: &gpui::MouseDownEvent, _, cx| {
                            let st = down_state.borrow();
                            let (x, y) = (
                                f32::from(ev.position.x - st.bounds.origin.x),
                                f32::from(ev.position.y - st.bounds.origin.y),
                            );
                            let target = st.hit(x, y).and_then(|i| st.tiles.get(i).copied());
                            drop(st);
                            if let Some(tile) = target.filter(|t| !t.node.is_none()) {
                                let is_dir = this.results.as_ref().is_some_and(|r| {
                                    r.snapshot.store.get(tile.node).flags().is_dir()
                                });
                                if is_dir && ev.click_count >= 2 {
                                    this.open_node(tile.node, cx);
                                } else {
                                    this.select_node(tile.node, cx);
                                }
                            }
                        }),
                    )
                    .child(
                        gpui::canvas(
                            |_, _, _| (),
                            move |bounds, (), window, _| {
                                let mut st = state.borrow_mut();
                                st.bounds = bounds;
                                st.ensure(
                                    &snapshot,
                                    node,
                                    f32::from(bounds.size.width),
                                    f32::from(bounds.size.height),
                                );
                                for (i, tile) in st.tiles.iter().enumerate() {
                                    let r = tile.rect;
                                    if r.w < 1.0 || r.h < 1.0 {
                                        continue;
                                    }
                                    let color = if Some(i) == hover {
                                        th.text
                                    } else if tile.node.is_none() {
                                        th.surface_3
                                    } else {
                                        let rec = snapshot.store.get(tile.node);
                                        if rec.flags().is_dir() {
                                            th.surface_3
                                        } else {
                                            th.chart[usize::from(rec.ext_id) % th.chart.len()]
                                        }
                                    };
                                    window.paint_quad(gpui::fill(
                                        Bounds::new(
                                            bounds.origin + gpui::point(px(r.x), px(r.y)),
                                            gpui::size(
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
            )
    }

    fn go_up(&mut self, cx: &mut Context<Self>) {
        if let Some(r) = &mut self.results {
            r.up();
            self.selected = None;
            self.tm_hover = None;
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
        let mode_button = |id: &'static str, label: String, target: ResultsMode| {
            div()
                .id(id)
                .px(px(8.))
                .py(px(2.))
                .rounded(px(4.))
                .cursor_pointer()
                .pressable(Role::Button, label.clone(), th)
                .aria_toggled(if self.mode == target {
                    Toggled::True
                } else {
                    Toggled::False
                })
                .bg(if self.mode == target {
                    th.accent
                } else {
                    th.surface_3
                })
                .text_color(if self.mode == target {
                    th.text_inverse
                } else {
                    th.text
                })
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.mode = target;
                    this.tm_hover = None;
                    cx.notify();
                }))
                .child(label)
        };
        let header = div()
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
                    .pressable(Role::Button, t("table.up"), th)
                    .when(!at_root, |d| {
                        d.cursor_pointer()
                            .on_click(cx.listener(|this, _, _, cx| this.go_up(cx)))
                    })
                    .when(at_root, |d| d.opacity(0.4))
                    .child(t("table.up")),
            )
            .child(
                div()
                    .id("path")
                    .role(Role::Label)
                    .aria_label(path.clone())
                    .child(path),
            )
            .child(div().flex_1())
            .child(mode_button(
                "mode-table",
                t("view.table"),
                ResultsMode::Table,
            ))
            .child(mode_button(
                "mode-treemap",
                t("view.treemap"),
                ResultsMode::Treemap,
            ));
        let frame = div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .p(px(16.))
            .gap(px(8.))
            .child(header)
            .children(self.render_action_bar(th, cx));
        if self.mode == ResultsMode::Treemap {
            return frame.child(self.render_treemap(th, cx));
        }
        frame
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
                div()
                    .id("rows-focus")
                    .role(Role::Table)
                    .aria_label(t("nav.results"))
                    .track_focus(&self.list_focus)
                    .tab_index(0)
                    .focus_visible(move |style| style.border_1().border_color(th.accent))
                    .on_key_down(cx.listener(|this, ev: &gpui::KeyDownEvent, _, cx| {
                        this.list_key(ev.keystroke.key.as_str(), cx);
                    }))
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .child(
                        uniform_list(
                            "rows",
                            count,
                            cx.processor(move |this, range: std::ops::Range<usize>, _, cx| {
                                let Some(r) = &this.results else {
                                    return Vec::new();
                                };
                                let store = &r.snapshot.store;
                                let selected = this.selected;
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
                                            .w_full()
                                            .h(row_h)
                                            .px(px(8.))
                                            .rounded(px(4.))
                                            .cursor_pointer()
                                            .role(Role::Row)
                                            .aria_label(format!(
                                                "{}, {}",
                                                store.name(id),
                                                format::bytes(rec.size, units)
                                            ))
                                            .aria_selected(selected == Some(id))
                                            .when(
                                                selected == Some(id),
                                                StatefulInteractiveElement::aria_active_descendant,
                                            )
                                            .hover(|d| d.bg(th.surface_2))
                                            .when(selected == Some(id), |d| d.bg(th.surface_3))
                                            .on_click(cx.listener(
                                                move |this, ev: &gpui::ClickEvent, window, cx| {
                                                    this.list_focus.focus(window, cx);
                                                    if is_dir && ev.click_count() >= 2 {
                                                        this.open_node(id, cx);
                                                    } else {
                                                        this.select_node(id, cx);
                                                    }
                                                },
                                            ))
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .min_w_0()
                                                    .overflow_hidden()
                                                    .whitespace_nowrap()
                                                    .text_ellipsis()
                                                    .child(format!(
                                                        "{}{}",
                                                        if is_dir { "\u{25B8} " } else { "" },
                                                        store.name(id)
                                                    )),
                                            )
                                            .child(
                                                div()
                                                    .w(px(100.))
                                                    .child(format::bytes(rec.size, units)),
                                            )
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
                        .track_scroll(&self.list_scroll)
                        .flex_1(),
                    ),
            )
    }

    /// Applies `change`, persists the result and reports a failed save as a toast.
    fn update_settings(&mut self, change: impl FnOnce(&mut Settings), cx: &mut Context<Self>) {
        change(&mut self.settings);
        self.catalog = catalog_for(&self.settings, self.launch.lang.as_deref());
        if self.settings.save(&self.dirs.settings_file()).is_err() {
            let text = self.catalog.t("common.error").to_string();
            self.push_toast(text, true, cx);
        }
        cx.notify();
    }

    fn render_settings(&self, th: Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let t = |key: &str| self.catalog.t(key).to_string();
        let s = &self.settings;
        // One labelled group of mutually exclusive choices; `active` marks the current one.
        let group = |id: &'static str, label: String, options: Vec<SettingChoice>| {
            div()
                .flex()
                .items_center()
                .gap(px(12.))
                .child(div().w(px(220.)).child(label))
                .children(
                    options
                        .into_iter()
                        .enumerate()
                        .map(|(i, (text, active, apply))| {
                            div()
                                .id((id, i))
                                .px(px(10.))
                                .py(px(4.))
                                .rounded(px(4.))
                                .cursor_pointer()
                                .pressable(Role::RadioButton, text.clone(), th)
                                .aria_toggled(if active {
                                    Toggled::True
                                } else {
                                    Toggled::False
                                })
                                .bg(if active { th.accent } else { th.surface_3 })
                                .text_color(if active { th.text_inverse } else { th.text })
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.update_settings(apply, cx);
                                }))
                                .child(text)
                        }),
                )
        };
        div()
            .id("settings")
            .flex_1()
            .overflow_y_scroll()
            .p(px(16.))
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(div().child(t("settings.title")))
            .child(group(
                "theme",
                t("settings.theme"),
                vec![
                    (
                        t("settings.theme_system"),
                        s.theme == ThemeMode::System,
                        |s| s.theme = ThemeMode::System,
                    ),
                    (
                        t("settings.theme_light"),
                        s.theme == ThemeMode::Light,
                        |s| s.theme = ThemeMode::Light,
                    ),
                    (t("settings.theme_dark"), s.theme == ThemeMode::Dark, |s| {
                        s.theme = ThemeMode::Dark;
                    }),
                    (
                        t("settings.theme_high_contrast"),
                        s.theme == ThemeMode::HighContrast,
                        |s| s.theme = ThemeMode::HighContrast,
                    ),
                ],
            ))
            .child(group(
                "density",
                t("settings.density"),
                vec![
                    (t("settings.density_comfortable"), !s.compact_density, |s| {
                        s.compact_density = false;
                    }),
                    (t("settings.density_compact"), s.compact_density, |s| {
                        s.compact_density = true;
                    }),
                ],
            ))
            .child(group(
                "language",
                t("settings.language"),
                vec![
                    (t("settings.language_system"), s.language.is_none(), |s| {
                        s.language = None;
                    }),
                    ("English".into(), s.language.as_deref() == Some("en"), |s| {
                        s.language = Some("en".into());
                    }),
                    (
                        "Français".into(),
                        s.language.as_deref() == Some("fr"),
                        |s| {
                            s.language = Some("fr".into());
                        },
                    ),
                    (
                        "简体中文".into(),
                        s.language.as_deref() == Some("zh-CN"),
                        |s| s.language = Some("zh-CN".into()),
                    ),
                ],
            ))
            .child(group(
                "threads",
                t("settings.threads"),
                vec![
                    (t("settings.threads_auto"), s.threads == 0, |s| {
                        s.threads = 0;
                    }),
                    ("2".into(), s.threads == 2, |s| {
                        s.threads = 2;
                    }),
                    ("4".into(), s.threads == 4, |s| {
                        s.threads = 4;
                    }),
                    ("8".into(), s.threads == 8, |s| {
                        s.threads = 8;
                    }),
                    ("16".into(), s.threads == 16, |s| {
                        s.threads = 16;
                    }),
                ],
            ))
            .child(group(
                "units",
                t("settings.units"),
                vec![
                    (t("settings.units_binary"), s.units == Units::Binary, |s| {
                        s.units = Units::Binary;
                    }),
                    (
                        t("settings.units_decimal"),
                        s.units == Units::Decimal,
                        |s| s.units = Units::Decimal,
                    ),
                ],
            ))
            .child(group(
                "hidden",
                t("settings.include_hidden"),
                vec![
                    (t("common.yes"), s.include_hidden, |s| {
                        s.include_hidden = true;
                    }),
                    (t("common.no"), !s.include_hidden, |s| {
                        s.include_hidden = false;
                    }),
                ],
            ))
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
        if !self.focus_ready {
            self.focus_ready = true;
            self.root_focus.focus(window, cx);
        }
        let th = Theme::resolve(
            self.launch.theme.unwrap_or(self.settings.theme),
            window.appearance(),
        );
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
                                            .pressable(
                                                Role::Button,
                                                format!(
                                                    "{} {}",
                                                    t("volumes.scan"),
                                                    v.mount_point.display()
                                                ),
                                                th,
                                            )
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
        let view = self.view;
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
                    .role(Role::Alert)
                    .aria_label(toast.text.clone())
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
            .track_focus(&self.root_focus)
            .on_key_down(cx.listener(|this, ev: &gpui::KeyDownEvent, window, cx| {
                // Escape dismisses the pending confirmation without acting.
                if ev.keystroke.key == "escape" && this.pending.take().is_some() {
                    cx.notify();
                }
                // GPUI leaves Tab navigation to the app.
                if ev.keystroke.key == "tab" {
                    if ev.keystroke.modifiers.shift {
                        window.focus_prev(cx);
                    } else {
                        window.focus_next(cx);
                    }
                }
            }))
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
                    .child({
                        let tab = |id: &'static str, label: String, target: View| {
                            div()
                                .id(id)
                                .px(px(10.))
                                .py(px(2.))
                                .rounded(px(4.))
                                .cursor_pointer()
                                .pressable(Role::Tab, label.clone(), th)
                                .aria_selected(view == target)
                                .when(view == target, |d| d.bg(th.surface_3))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.view = target;
                                    cx.notify();
                                }))
                                .child(label)
                        };
                        div()
                            .flex()
                            .gap(px(4.))
                            .child(tab("tab-volumes", t("nav.volumes"), View::Volumes))
                            .when(self.results.is_some(), |d| {
                                d.child(tab("tab-results", t("nav.results"), View::Results))
                            })
                            .child(tab("tab-settings", t("nav.settings"), View::Settings))
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
                            .pressable(Role::Button, t("volumes.refresh"), th)
                            .on_click(cx.listener(|this, _, _, cx| this.refresh(cx)))
                            .child(t("volumes.refresh")),
                    ),
            )
            .child(if view == View::Settings {
                self.render_settings(th, cx).into_any_element()
            } else if view == View::Results && self.results.is_some() {
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
                    .when(cfg!(windows), |d| {
                        d.child(if self.elevated {
                            div().text_color(th.text_muted).child(t("volumes.fast_on"))
                        } else {
                            div()
                                .flex()
                                .gap(px(12.))
                                .items_center()
                                .child(
                                    div()
                                        .text_color(th.text_muted)
                                        .child(t("volumes.fast_hint")),
                                )
                                .child(
                                    div()
                                        .id("relaunch")
                                        .px(px(10.))
                                        .py(px(2.))
                                        .rounded(px(4.))
                                        .bg(th.accent)
                                        .text_color(th.text_inverse)
                                        .cursor_pointer()
                                        .pressable(Role::Button, t("volumes.relaunch"), th)
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.relaunch_elevated(cx);
                                        }))
                                        .child(t("volumes.relaunch")),
                                )
                        })
                    })
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
                    .id("statusbar")
                    .role(Role::Status)
                    .aria_label(status.clone())
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
                                .pressable(Role::Button, t("scan.cancel"), th)
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
