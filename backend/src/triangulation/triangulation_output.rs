use nalgebra::Vector3;

/// Output contract that downstream consumers (decision logic, server) take
/// from the triangulation step. Produced by the thread that owns
/// `math_triangulation::optimize_trajectory` (see issue #80) by packaging its
/// return tuple together with an interpolated impact timestamp.
///
/// Covariance and drag are intentionally not included here — neither is
/// consumed by the decision logic today. They can be added later if a
/// consumer needs them.
#[derive(Debug, Clone)]
pub struct TriangulationOutput {
    /// 3D positions sampled at `dt` intervals from `optimize_trajectory`.
    /// The trajectory is truncated at impact during optimization, so the last
    /// element is the landing point in world coordinates.
    pub trajectory: Vec<Vector3<f64>>,
    /// Whether the Levenberg-Marquardt non-linear solver in
    /// `optimize_trajectory` converged. False means the trajectory is not
    /// usable; consumers should surface a "no valid throw" response upstream
    /// rather than running distance/sector classification on garbage output.
    pub triangulation_succeeded: bool,
    /// Interpolated impact instant in **Unix-epoch nanoseconds**.
    ///
    /// Cameras only sample at the configured frame rate, so we rarely capture
    /// a frame at the exact moment of impact. The LM-call layer (#80) is
    /// responsible for solving the trajectory polynomial for the y/z = 0
    /// crossing and producing the sub-frame-precision timestamp. The decision
    /// logic uses this to populate
    /// `frame_timestamp_from_camera_microseconds` on the response.
    ///
    /// Must already be referenced to the Unix epoch — converting from
    /// camera/PTP time scales is upstream's responsibility (tracked in #76).
    pub impact_timestamp_ns: u64,
}
