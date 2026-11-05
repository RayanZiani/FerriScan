//! Treemap and sunburst layouts, computed in Rust and rendered on canvas.
//!
//! Both algorithms are deterministic: the same request on the same snapshot
//! produces the same rectangles and arcs, which lets golden tests catch
//! regressions and keeps the UI stable across re-renders.

pub mod squarify;
pub mod sunburst;
pub mod treemap;

pub use sunburst::{SunburstArc, SunburstRequest, sunburst};
pub use treemap::{TreemapRequest, TreemapTile, treemap};
