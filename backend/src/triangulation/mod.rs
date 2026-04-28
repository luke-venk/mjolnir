// Output-side contract for the triangulation stage.
//
// The math itself lives in `crate::math_triangulation` (Yash's
// `optimize_trajectory`). The thread that owns calling `optimize_trajectory`
// and emits values of this shape to downstream consumers — the decision logic
// in `throws::decision`, the aggregator endpoint, etc. — is the subject of
// issue #80.
//
// This module currently exposes only the data contract so that #69 can land
// the decision logic against a stable shape. The thread implementation will be
// added by #80 once the upstream `OptimizeTrajectoryInput` shape stabilizes
// (see PR #74) and a calibration-loading mechanism is decided.

pub mod triangulation_output;

pub use triangulation_output::TriangulationOutput;
