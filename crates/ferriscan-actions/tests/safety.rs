//! Safety suite for destructive operations.
//!
//! Every case runs in its own temporary sandbox. These tests are the guard rail
//! described in SPEC §17.1: if any of them fails, the build must not ship.

use std::fs;
use std::path::{Path, PathBuf};

use ferriscan_actions::execute::execute;
use ferriscan_actions::plan::{ActionKind, PlanStatus, plan};

#[cfg(unix)]
use ferriscan_actions::protected::{ProtectedCheck, check};

/// Creates a unique sandbox and returns its path. Callers clean up themselves.
fn sandbox(name: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join("ferriscan-action-tests")
        .join(format!("{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create sandbox");
    dir
}

fn write(path: &Path, bytes: usize) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent");
    }
    fs::write(path, vec![0u8; bytes]).expect("write file");
}

#[test]
fn move_into_a_new_directory_succeeds() {
    let dir = sandbox("move-ok");
    let src = dir.join("a.txt");
    let dest = dir.join("out");
    write(&src, 10);
    fs::create_dir_all(&dest).expect("dest");

    let plan = plan(ActionKind::Move, std::slice::from_ref(&src), Some(&dest)).expect("plan");
    assert!(plan.is_executable());
    let report = execute(&plan);

    assert_eq!(report.succeeded, 1);
    assert!(dest.join("a.txt").exists());
    assert!(!src.exists());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn existing_destination_is_a_conflict_and_nothing_is_overwritten() {
    let dir = sandbox("conflict");
    let src = dir.join("src.txt");
    let dest = dir.join("out");
    write(&src, 10);
    write(&dest.join("src.txt"), 99);

    let plan = plan(ActionKind::Move, std::slice::from_ref(&src), Some(&dest)).expect("plan");
    assert_eq!(plan.items[0].status, PlanStatus::Conflict);
    assert!(plan.has_conflicts());
    assert!(!plan.is_executable());

    let report = execute(&plan);
    assert_eq!(report.succeeded, 0);
    assert_eq!(report.skipped, 1);
    // Both files are untouched.
    assert!(src.exists());
    assert_eq!(fs::metadata(dest.join("src.txt")).expect("dest").len(), 99);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn missing_source_is_reported_without_touching_anything() {
    let dir = sandbox("missing");
    let src = dir.join("ghost.bin");
    let dest = dir.join("out");
    fs::create_dir_all(&dest).expect("dest");

    let plan = plan(ActionKind::Move, std::slice::from_ref(&src), Some(&dest)).expect("plan");
    assert_eq!(plan.items[0].status, PlanStatus::Missing);
    assert!(plan.items[0].reason.is_some());
    let report = execute(&plan);
    assert_eq!(report.succeeded, 0);
    assert_eq!(report.skipped, 1);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn destination_that_is_a_file_is_rejected_before_execution() {
    let dir = sandbox("bad-dest");
    let src = dir.join("a.txt");
    let dest = dir.join("not-a-dir");
    write(&src, 10);
    write(&dest, 10);

    let err = plan(ActionKind::Move, std::slice::from_ref(&src), Some(&dest))
        .expect_err("must reject a file destination");
    assert!(err.to_string().contains("not an existing directory"));
    assert!(src.exists());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn rename_requires_exactly_one_source() {
    let dir = sandbox("rename-arity");
    let a = dir.join("a.txt");
    let b = dir.join("b.txt");
    write(&a, 1);
    write(&b, 1);

    let err = plan(ActionKind::Rename, &[a, b], None).expect_err("must reject two sources");
    assert!(err.to_string().contains("exactly one"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn unimplemented_operations_fail_loudly_instead_of_pretending() {
    let dir = sandbox("unimplemented");
    let src = dir.join("a.txt");
    write(&src, 10);

    // Trash is implemented on Windows only; elsewhere it must still fail loudly.
    let mut kinds = vec![ActionKind::Copy, ActionKind::DeletePermanent];
    if cfg!(not(windows)) {
        kinds.push(ActionKind::Trash);
    }
    for kind in kinds {
        let dest = dir.join("out");
        fs::create_dir_all(&dest).expect("dest");
        let outcome = if kind == ActionKind::Copy {
            plan(kind, std::slice::from_ref(&src), Some(&dest))
        } else {
            plan(kind, std::slice::from_ref(&src), None)
        };
        let plan = outcome.expect("plan");
        let report = execute(&plan);
        assert_eq!(report.succeeded, 0, "{kind:?} must not claim success");
        assert_eq!(report.failed, 1, "{kind:?} must report a failure");
        assert!(src.exists(), "{kind:?} must not delete the source");
    }
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn total_bytes_counts_only_sized_items() {
    let dir = sandbox("bytes");
    let a = dir.join("a.bin");
    let b = dir.join("b.bin");
    let dest = dir.join("out");
    write(&a, 1_500);
    write(&b, 500);
    fs::create_dir_all(&dest).expect("dest");

    let plan = plan(ActionKind::Move, &[a, b], Some(&dest)).expect("plan");
    assert_eq!(plan.total_bytes, 2_000);
    let _ = fs::remove_dir_all(&dir);
}

#[cfg(unix)]
#[test]
fn system_directories_are_blocked() {
    assert!(matches!(
        check(Path::new("/etc/passwd")),
        ProtectedCheck::Blocked(_)
    ));
    assert!(matches!(
        check(Path::new("/usr/lib/libc.so")),
        ProtectedCheck::Blocked(_)
    ));
    assert_eq!(check(Path::new("/tmp/whatever")), ProtectedCheck::Allowed);
}

#[cfg(unix)]
#[test]
fn protected_paths_cannot_be_planned_for_deletion() {
    let plan = plan(
        ActionKind::DeletePermanent,
        &[PathBuf::from("/etc/passwd")],
        None,
    )
    .expect("plan is still produced");
    assert_eq!(plan.items[0].status, PlanStatus::Blocked);
    assert!(plan.needs_override);
    assert!(!plan.is_executable());
}

#[cfg(unix)]
#[test]
fn similarly_named_paths_are_not_falsely_blocked() {
    assert_eq!(
        check(Path::new("/etcetera/notes.txt")),
        ProtectedCheck::Allowed
    );
    assert_eq!(check(Path::new("/usr-local/data")), ProtectedCheck::Allowed);
}

#[cfg(unix)]
#[test]
fn the_volume_root_itself_is_protected() {
    assert!(matches!(check(Path::new("/")), ProtectedCheck::Blocked(_)));
    assert_eq!(check(Path::new("/var/tmp/x")), ProtectedCheck::Allowed);
}

#[test]
fn cross_volume_detection_is_reported_honestly() {
    let dir = sandbox("cross-volume");
    let src = dir.join("a.txt");
    write(&src, 10);
    let plan = plan(ActionKind::Move, std::slice::from_ref(&src), Some(&dir)).expect("plan");
    // Same sandbox means same volume; the flag must be false, not guessed true.
    assert!(!plan.cross_volume);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn plan_ids_are_unique_per_call() {
    let dir = sandbox("plan-ids");
    let src = dir.join("a.txt");
    write(&src, 1);
    let first = plan(ActionKind::Move, std::slice::from_ref(&src), Some(&dir)).expect("plan");
    let second = plan(ActionKind::Move, std::slice::from_ref(&src), Some(&dir)).expect("plan");
    assert_ne!(first.id, second.id);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_directory_can_be_moved_with_its_contents() {
    let dir = sandbox("move-dir");
    let src = dir.join("tree");
    write(&src.join("inner/deep.bin"), 32);
    let dest = dir.join("out");
    fs::create_dir_all(&dest).expect("dest");

    let plan = plan(ActionKind::Move, std::slice::from_ref(&src), Some(&dest)).expect("plan");
    let report = execute(&plan);
    assert_eq!(report.succeeded, 1);
    assert!(dest.join("tree/inner/deep.bin").exists());
    assert!(!src.exists());
    let _ = fs::remove_dir_all(&dir);
}
