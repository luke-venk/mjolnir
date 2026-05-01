//! Triangulation-stage output contract consumed by `throws::decision`.
//!
//! The math itself lives in `math_triangulation.rs` (Yash's
//! `optimize_trajectory`) and is currently not declared in the module tree —
//! wiring it in is the subject of issue #80, which also owns producing values
//! of [`TriangulationOutput`] from its return tuple.
//!
//! This module exposes only the data contract so that #69 can land the
//! decision logic against a stable shape.

pub mod math_triangulation;
pub mod triangulation_output;

pub use triangulation_output::TriangulationOutput;
