//! End-to-end scan pipeline over an in-memory filesystem.
//!
//! These tests exercise the same code path the app uses (controller → workers →
//! aggregation → query) without touching a disk, so they are deterministic on
//! every CI runner.

use ferriscan_core::fs::{EntryKind, FakeFs};
use ferriscan_core::model::{SizeBasis, SnapshotStatus};
use ferriscan_core::query::children::{SortDirection, SortKey, children_page};
use ferriscan_core::query::filters::{FilterSet, KindFilter};
use ferriscan_core::query::search::search;
use ferriscan_core::scan::controller::CancelToken;
use ferriscan_core::scan::errors::ErrorCode;
use ferriscan_core::scan::progress::{Progress, ScanState};
use ferriscan_core::scan::{ScanOptions, scan};
use std::path::Path;

fn run(fs: &FakeFs, options: &ScanOptions) -> ferriscan_core::model::Snapshot {
    let progress = Progress::new();
    let cancel = CancelToken::new();
    let snapshot = scan(fs, Path::new(""), options, &progress, &cancel).expect("scan");
    snapshot.store.validate().expect("store invariants");
    snapshot
}

fn sample_tree() -> FakeFs {
    let mut fs = FakeFs::new();
    fs.dir("media");
    fs.dir("media/2024");
    fs.dir("media/2025");
    fs.file("media/2024/a.mkv", 4_000);
    fs.file("media/2024/b.mkv", 1_000);
    fs.file("media/2025/c.mp4", 2_500);
    fs.file("notes.txt", 200);
    fs.file("readme.md", 50);
    fs
}

#[test]
fn aggregates_sizes_counts_and_depth() {
    let fs = sample_tree();
    let snapshot = run(&fs, &ScanOptions::default());

    assert_eq!(snapshot.store.len(), 9);
    assert_eq!(snapshot.meta.file_count, 5);
    assert_eq!(snapshot.meta.dir_count, 4);
    assert_eq!(snapshot.meta.logical_bytes, 7_750);
    assert_eq!(snapshot.meta.max_depth, 3);
    assert_eq!(snapshot.status, SnapshotStatus::Complete);
    assert_eq!(snapshot.total_size(SizeBasis::Logical), 7_750);

    let media = snapshot
        .store
        .children(snapshot.root)
        .find(|id| snapshot.store.name(*id) == "media")
        .expect("media dir");
    assert_eq!(snapshot.store.get(media).size, 7_500);
    assert_eq!(snapshot.store.get(media).child_count(), 2);
}

#[test]
fn children_page_is_sorted_and_bounded() {
    let fs = sample_tree();
    let snapshot = run(&fs, &ScanOptions::default());

    let page = children_page(
        &snapshot,
        snapshot.root,
        SortKey::Size,
        SortDirection::Descending,
        &FilterSet::default(),
        0,
        2,
    );
    assert_eq!(page.total_matching, 3);
    assert_eq!(page.ids.len(), 2);
    let first = snapshot.store.name(page.ids[0]);
    assert_eq!(first, "media", "the biggest entry must come first");
}

#[test]
fn filters_narrow_the_listing() {
    let fs = sample_tree();
    let snapshot = run(&fs, &ScanOptions::default());

    let files_only = FilterSet {
        kind: KindFilter::Files,
        ..FilterSet::default()
    };
    let page = children_page(
        &snapshot,
        snapshot.root,
        SortKey::Name,
        SortDirection::Ascending,
        &files_only,
        0,
        10,
    );
    assert_eq!(page.total_matching, 2);

    let markdown = FilterSet {
        extensions: vec!["MD".into()],
        ..FilterSet::default()
    };
    let page = children_page(
        &snapshot,
        snapshot.root,
        SortKey::Name,
        SortDirection::Ascending,
        &markdown,
        0,
        10,
    );
    assert_eq!(page.total_matching, 1);
    assert_eq!(snapshot.store.name(page.ids[0]), "readme.md");
}

#[test]
fn search_finds_nested_entries_with_paths() {
    let fs = sample_tree();
    let snapshot = run(&fs, &ScanOptions::default());

    let hits = search(&snapshot, "2025", 10);
    assert_eq!(hits.len(), 1, "only the folder name matches");
    assert!(hits[0].is_dir);

    let videos = search(&snapshot, ".mkv", 10);
    assert_eq!(videos.len(), 2);
    assert_eq!(videos[0].size, 4_000, "results are sorted by size");
    assert_eq!(videos[0].relative_path, Path::new("media/2024/a.mkv"));
}

#[test]
fn max_depth_limits_traversal() {
    let fs = sample_tree();
    let snapshot = run(
        &fs,
        &ScanOptions {
            max_depth: Some(1),
            ..ScanOptions::default()
        },
    );
    // Root children only: media, notes.txt, readme.md.
    assert_eq!(snapshot.store.len(), 4);
    assert_eq!(snapshot.meta.max_depth, 1);
}

#[test]
fn min_file_size_skips_small_entries() {
    let fs = sample_tree();
    let snapshot = run(
        &fs,
        &ScanOptions {
            min_file_size: 1_000,
            ..ScanOptions::default()
        },
    );
    let names: Vec<String> = snapshot
        .store
        .iter()
        .map(|(id, _)| snapshot.store.name(id).to_string())
        .collect();
    assert!(!names.contains(&"readme.md".to_string()));
    assert!(!names.contains(&"notes.txt".to_string()));
    assert_eq!(snapshot.meta.file_count, 3);
}

#[test]
fn excludes_prune_a_subtree() {
    let fs = sample_tree();
    let snapshot = run(
        &fs,
        &ScanOptions {
            excludes: vec!["2025".into()],
            ..ScanOptions::default()
        },
    );
    let hits = search(&snapshot, "2025", 10);
    assert!(hits.is_empty());
    assert_eq!(snapshot.meta.file_count, 4);
}

#[test]
fn permission_errors_give_a_partial_snapshot() {
    let mut fs = sample_tree();
    fs.failing("media/2025", ErrorCode::PermissionDenied);
    let snapshot = run(&fs, &ScanOptions::default());

    assert_eq!(snapshot.status, SnapshotStatus::Partial);
    assert_eq!(snapshot.meta.error_count, 1);
    // The rest of the tree is still indexed and usable.
    assert!(snapshot.meta.file_count >= 4);
}

#[test]
fn cancelled_scan_keeps_partial_results() {
    let fs = sample_tree();
    let progress = Progress::new();
    let cancel = CancelToken::new();
    cancel.cancel();

    let snapshot = scan(
        &fs,
        Path::new(""),
        &ScanOptions::default(),
        &progress,
        &cancel,
    )
    .expect("cancelled scan still yields a snapshot");
    assert_eq!(snapshot.status, SnapshotStatus::Cancelled);
    assert_eq!(progress.state(), ScanState::Cancelled);
    // The root itself is always indexed so navigation has an anchor.
    assert_eq!(snapshot.store.len(), 1);
}

#[test]
fn empty_root_yields_a_single_node() {
    let fs = FakeFs::new();
    let snapshot = run(&fs, &ScanOptions::default());
    assert_eq!(snapshot.store.len(), 1);
    assert_eq!(snapshot.store.get(snapshot.root).size, 0);
    assert!(snapshot.store.get(snapshot.root).flags().is_dir());
}

#[test]
fn names_with_unicode_and_spaces_survive() {
    let mut fs = FakeFs::new();
    fs.file("Résumé final (v2).pdf", 10);
    fs.file("日本語のファイル.txt", 20);
    fs.dir("espace dans le nom");
    fs.file("espace dans le nom/ok.bin", 5);

    let snapshot = run(&fs, &ScanOptions::default());
    let names: Vec<&str> = snapshot
        .store
        .iter()
        .map(|(id, _)| snapshot.store.name(id))
        .collect();
    assert!(names.contains(&"Résumé final (v2).pdf"));
    assert!(names.contains(&"日本語のファイル.txt"));
    assert_eq!(snapshot.meta.logical_bytes, 35);
}

#[test]
fn caches_are_not_created_and_tree_stays_immutable_during_queries() {
    let fs = sample_tree();
    let snapshot = run(&fs, &ScanOptions::default());
    let before = snapshot.store.len();

    // Many interleaved queries must not mutate anything.
    for _ in 0..50 {
        let _ = children_page(
            &snapshot,
            snapshot.root,
            SortKey::Size,
            SortDirection::Descending,
            &FilterSet::default(),
            0,
            50,
        );
        let _ = search(&snapshot, "media", 5);
    }
    assert_eq!(snapshot.store.len(), before);
    snapshot.store.validate().expect("still valid");
}

#[test]
fn hidden_entries_can_be_excluded() {
    let mut fs = FakeFs::new();
    fs.symlink(".hidden-link", "target");
    fs.file("visible.txt", 10);

    let snapshot = run(
        &fs,
        &ScanOptions {
            include_hidden: false,
            ..ScanOptions::default()
        },
    );
    let names: Vec<&str> = snapshot
        .store
        .iter()
        .map(|(id, _)| snapshot.store.name(id))
        .collect();
    assert!(!names.contains(&".hidden-link"));
    assert!(names.contains(&"visible.txt"));
}

#[test]
fn symlinks_are_recorded_but_not_followed() {
    let mut fs = FakeFs::new();
    fs.dir("real");
    fs.file("real/data.bin", 100);
    fs.symlink("link-to-real", "real");

    let snapshot = run(&fs, &ScanOptions::default());
    let link = snapshot
        .store
        .iter()
        .find(|(id, _)| snapshot.store.name(*id) == "link-to-real")
        .map(|(id, _)| id)
        .expect("link node");
    let flags = snapshot.store.get(link).flags();
    assert!(flags.is_symlink());
    assert!(!flags.is_dir());
    assert!(!flags.is_file());

    // The target is indexed exactly once, through its real path. If the scanner
    // had followed the link, `data.bin` would appear twice.
    assert_eq!(snapshot.meta.file_count, 1);
    // FakeFs does not resolve links, so a descent would have produced an error.
    assert_eq!(snapshot.meta.error_count, 0);
    let _ = EntryKind::Symlink;
}
