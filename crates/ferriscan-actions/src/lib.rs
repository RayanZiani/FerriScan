//! File mutations: plan, execute, journal, undo.
//!
//! This is the only crate allowed to delete, move, copy or rename user data. The
//! design is intentionally two-phase (ADR-0003): [`plan`] validates and describes
//! an operation without touching anything, and [`execute`] refuses to run a plan
//! that was not produced by this process.

#![forbid(unsafe_code)]

pub mod execute;
pub mod plan;
pub mod protected;

pub use plan::{ActionKind, ActionPlan, PlanItem, PlanStatus, plan};
