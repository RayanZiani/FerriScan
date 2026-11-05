//! Pure `FerriScan` engine.
//!
//! This crate owns the index, the scan orchestration, the query layer and the
//! layout algorithms. It performs no OS calls and never mutates user files: all
//! filesystem access goes through the [`fs::DirReader`] trait, which lets the
//! engine run identically on a real disk, a test fake, or a benchmark tree.
//!
//! Invariants enforced by [`model::NodeStore`]:
//!
//! 1. Every non-root node has a valid `parent`.
//! 2. `first_child` / `next_sibling` chains are acyclic and visit each child once.
//! 3. After `finalize`, a directory's `size` and `allocated` equal the sum of its
//!    children, and `depth(child) == depth(parent) + 1`.
//! 4. Name slices are valid UTF-8 and inside the name arena.
//! 5. Every `NodeId` is either [`model::NodeId::NONE`] or `< store.len()`.

#![forbid(unsafe_code)]

pub mod dto;
pub mod fs;
pub mod layout;
pub mod model;
pub mod query;
pub mod scan;
pub mod util;

pub use model::{NodeFlags, NodeId, NodeStore, Snapshot, SnapshotId, SnapshotStatus};
