//! Execution of a validated plan.
//!
//! The full step machine (journal, copy verification, undo) lands in milestone
//! M5. This module already enforces the important part: execution is only
//! possible through a plan object, items are processed sequentially, and every
//! failure is reported per item instead of aborting the batch.

use std::path::Path;

use ferriscan_core::scan::errors::ErrorCode;

use crate::plan::{ActionKind, ActionPlan, PlanItem, PlanStatus};

/// Per-item execution outcome.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ItemOutcome {
    /// The item was processed.
    Done,
    /// The item was skipped because it was not ready.
    Skipped,
    /// The item failed.
    Failed,
}

/// Result of executing a plan.
#[derive(Clone, Debug, Default)]
pub struct ActionReport {
    /// Items completed.
    pub succeeded: u32,
    /// Items skipped.
    pub skipped: u32,
    /// Items failed.
    pub failed: u32,
    /// Bytes moved, copied or deleted.
    pub bytes: u64,
    /// Journal identifier, when the action is reversible.
    pub journal_id: Option<String>,
    /// Human-readable failure summaries keyed by source path.
    pub failures: Vec<(String, String)>,
}

/// Executes every ready item of `plan`.
///
/// Only [`ActionKind::Rename`] and same-volume [`ActionKind::Move`] are
/// implemented so far; the others return `E_UNSUPPORTED` per item rather than
/// pretending to work.
pub fn execute(plan: &ActionPlan) -> ActionReport {
    let mut report = ActionReport::default();
    for item in &plan.items {
        match execute_item(plan.kind, item) {
            Ok(ItemOutcome::Done) => {
                report.succeeded += 1;
                report.bytes = report.bytes.saturating_add(item.size);
            }
            Ok(ItemOutcome::Skipped) => report.skipped += 1,
            Ok(ItemOutcome::Failed) | Err(_) => {
                report.failed += 1;
                report.failures.push((
                    item.source.display().to_string(),
                    item.reason
                        .clone()
                        .unwrap_or_else(|| "operation failed".to_string()),
                ));
            }
        }
    }
    report
}

fn execute_item(kind: ActionKind, item: &PlanItem) -> Result<ItemOutcome, (ErrorCode, String)> {
    if item.status != PlanStatus::Ready {
        return Ok(ItemOutcome::Skipped);
    }
    let Some(destination) = &item.destination else {
        return Err((ErrorCode::Unsupported, "no destination".into()));
    };

    match kind {
        ActionKind::Rename | ActionKind::Move => {
            if !same_volume(&item.source, destination) {
                return Err((
                    ErrorCode::Unsupported,
                    "cross-volume move is not implemented yet".into(),
                ));
            }
            std::fs::rename(&item.source, destination)
                .map_err(|e| (ErrorCode::Io, e.to_string()))?;
            Ok(ItemOutcome::Done)
        }
        ActionKind::Copy | ActionKind::Trash | ActionKind::DeletePermanent => Err((
            ErrorCode::Unsupported,
            format!("{kind:?} is not implemented yet"),
        )),
    }
}

fn same_volume(a: &Path, b: &Path) -> bool {
    #[cfg(windows)]
    {
        let ra = a.components().next().map(|c| c.as_os_str().to_owned());
        let rb = b.components().next().map(|c| c.as_os_str().to_owned());
        match (ra, rb) {
            (Some(x), Some(y)) => x.eq_ignore_ascii_case(&y),
            _ => false,
        }
    }
    #[cfg(not(windows))]
    {
        use std::os::unix::fs::MetadataExt;
        match (std::fs::metadata(a), std::fs::metadata(b)) {
            (Ok(x), Ok(y)) => x.dev() == y.dev(),
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::plan;

    #[test]
    fn rename_moves_a_file() {
        let dir = std::env::temp_dir().join("ferriscan-exec-rename");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        let file = dir.join("a.txt");
        std::fs::write(&file, b"hello").expect("write");
        let dest_dir = dir.join("out");
        std::fs::create_dir_all(&dest_dir).expect("mkdir");

        let p = plan(
            ActionKind::Move,
            std::slice::from_ref(&file),
            Some(dest_dir.as_path()),
        )
        .expect("plan");
        let report = execute(&p);
        assert_eq!(report.succeeded, 1);
        assert!(dest_dir.join("a.txt").exists());
        assert!(!file.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unimplemented_actions_fail_instead_of_pretending() {
        let dir = std::env::temp_dir().join("ferriscan-exec-copy");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        let file = dir.join("a.txt");
        std::fs::write(&file, b"hello").expect("write");

        let p = plan(
            ActionKind::Copy,
            std::slice::from_ref(&file),
            Some(dir.as_path()),
        )
        .expect("plan");
        let report = execute(&p);
        assert_eq!(report.failed, 1);
        assert_eq!(report.succeeded, 0);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
