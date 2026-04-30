//! Convert the output of `math_triangulation::optimize_trajectory` into a
//! `ThrowAnalysisResponse` that can be returned to the frontend.
//!
//! The triangulation step returns a 3D trajectory in world coordinates. This
//! module is responsible for picking the landing point, computing the
//! distance from the inside edge of the throwing circle, deciding whether
//! the throw is a sector violation, and assembling the final response.
//!
//! Coordinate frame:
//! - Origin is at the center of the throwing circle.
//! - +x points down the center of the legal sector.
//! - +z points up.
//!
//! A throw with `y == 0` is straight down the middle of the sector.

use super::{
    InfractionType, ThrowAnalysisResponse, ThrowType,
    simulate_throw::get_field_dimensions,
};
use crate::math_triangulation::TriangulationOutput;
use chrono::{DateTime, Utc};
use nalgebra::Vector3;
use uuid::Uuid;

/// Pick the landing point from the optimized trajectory.
///
/// The trajectory is a sequence of 3D positions sampled at `dt` intervals.
/// The landing point is the last sample (the trajectory is truncated at
/// impact during optimization). Returns `None` if the trajectory is empty.
///
/// In practice the trajectory will be empty only if the upstream LM-call
/// layer emits a `TriangulationOutput` with `triangulation_succeeded == true`
/// but no samples — which shouldn't happen, but we keep the `Option` to
/// avoid a panic if the upstream contract drifts. `build_throw_response`
/// already short-circuits on `triangulation_succeeded == false`, so this is
/// the secondary defense.
fn landing_from_trajectory(trajectory: &[Vector3<f64>]) -> Option<(f32, f32)> {
    let last = trajectory.last()?;
    Some((last[0] as f32, last[1] as f32))
}

/// Distance from the inside edge of the throwing circle to the landing
/// point, in meters. The throwing circle is centered at the origin in our
/// coordinate frame, so the distance is `||(x, y)|| - circle_radius`.
///
/// World athletics rules measure from the inside edge of the stop board,
/// but for shot put / discus / hammer the circle is symmetric and the
/// measurement is from the inside edge of the circle. For javelin, the
/// "circle" field dimension represents the runway length, not a circle
/// radius, so the subtraction is omitted.
fn compute_distance(landing_xy: (f32, f32), throw_type: ThrowType) -> f32 {
    let (x, y) = landing_xy;
    let raw = (x * x + y * y).sqrt();
    match throw_type {
        ThrowType::Javelin => raw,
        _ => {
            let (circle_diameter, _, _) = get_field_dimensions(throw_type);
            raw - circle_diameter / 2.0
        }
    }
}

/// Decide whether the landing point is outside the legal sector and, if so,
/// which side it landed on. Returns `None` if the throw is in-bounds.
///
/// The sector is symmetric about the +x axis with half-angle
/// `sector_angle / 2`. A throw with positive y past the half-angle is a
/// `LeftSector` violation; negative y past the half-angle is `RightSector`.
fn classify_sector(landing_xy: (f32, f32), throw_type: ThrowType) -> Option<InfractionType> {
    let (x, y) = landing_xy;
    let (_, _, sector_angle_deg) = get_field_dimensions(throw_type);
    let half_sector_rad = (sector_angle_deg / 2.0).to_radians();

    let theta = y.atan2(x);
    if theta.abs() <= half_sector_rad {
        return None;
    }

    Some(if y >= 0.0 {
        InfractionType::LeftSector
    } else {
        InfractionType::RightSector
    })
}

/// Convert a Unix-epoch nanosecond timestamp to the RFC3339 string the
/// frontend expects.
///
/// **Precondition:** `impact_unix_timestamp_ns` must be referenced to the
/// Unix epoch. The cameras' raw `buffer_timestamp_ns` values are PTP
/// timestamps in their own clock domain; converting from camera/PTP time
/// to Unix time is upstream's responsibility (tracked in #76). Passing in
/// raw PTP nanoseconds will produce nonsense datetimes.
///
/// The schema field this populates is named
/// `frame_timestamp_from_camera_microseconds` for historical reasons but
/// the value is an RFC3339 string (see `simulate_throw_event`).
fn timestamp_ns_to_string(impact_unix_timestamp_ns: u64) -> String {
    let secs = (impact_unix_timestamp_ns / 1_000_000_000) as i64;
    let nsecs = (impact_unix_timestamp_ns % 1_000_000_000) as u32;
    DateTime::<Utc>::from_timestamp(secs, nsecs)
        .unwrap_or_else(Utc::now)
        .to_rfc3339()
}

/// Build the final response from the triangulation output and surrounding
/// context. `circle_infraction` comes from the touch sensor pipeline and is
/// provided by the caller; this module does not detect circle infractions.
///
/// Returns `Err(message)` if the triangulation step did not converge or
/// produced no trajectory. The caller decides what to do with the error
/// (e.g., return 503, fall back to the previous response, etc.).
///
/// `triangulation` is produced by the LM-call layer (issue #80) by wrapping
/// the return tuple of `math_triangulation::optimize_trajectory`. See
/// [`TriangulationOutput`] for the contract — including that
/// `impact_timestamp_ns` must already be in Unix-epoch nanoseconds.
pub fn build_throw_response(
    triangulation: &TriangulationOutput,
    throw_type: ThrowType,
    circle_infraction: Option<InfractionType>,
    image_urls: Vec<String>,
) -> Result<ThrowAnalysisResponse, String> {
    if !triangulation.triangulation_succeeded {
        return Err("Levenberg-Marquardt optimization did not converge".to_string());
    }
    let landing_xy = landing_from_trajectory(&triangulation.trajectory)
        .ok_or_else(|| "Triangulation produced an empty trajectory".to_string())?;

    let mut infractions: Vec<InfractionType> = Vec::new();
    let sector_violation = classify_sector(landing_xy, throw_type);
    let has_sector_violation = sector_violation.is_some();
    if let Some(infraction) = sector_violation {
        infractions.push(infraction);
    }
    if let Some(infraction) = circle_infraction {
        infractions.push(infraction);
    }

    let landing_point_x_y = if has_sector_violation {
        None
    } else {
        Some(landing_xy)
    };

    Ok(ThrowAnalysisResponse {
        throw_id: Uuid::new_v4(),
        frame_timestamp_from_camera_microseconds: timestamp_ns_to_string(
            triangulation.impact_timestamp_ns,
        ),
        throw_type,
        distance_m: compute_distance(landing_xy, throw_type),
        infractions,
        images: image_urls,
        landing_point_x_y,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn traj(points: &[(f64, f64, f64)]) -> Vec<Vector3<f64>> {
        points.iter().map(|(x, y, z)| Vector3::new(*x, *y, *z)).collect()
    }

    fn output(trajectory: Vec<Vector3<f64>>, succeeded: bool) -> TriangulationOutput {
        TriangulationOutput {
            trajectory,
            triangulation_succeeded: succeeded,
            impact_timestamp_ns: 1_775_771_934_343_718_000,
        }
    }

    #[test]
    fn landing_returns_last_trajectory_point() {
        let t = traj(&[(0.0, 0.0, 2.0), (5.0, 0.0, 1.0), (10.0, 0.5, 0.1)]);
        assert_eq!(landing_from_trajectory(&t), Some((10.0, 0.5)));
    }

    #[test]
    fn landing_empty_trajectory_returns_none() {
        let t: Vec<Vector3<f64>> = vec![];
        assert_eq!(landing_from_trajectory(&t), None);
    }

    #[test]
    fn distance_subtracts_circle_radius_for_throwing_events() {
        // Shotput circle diameter = 2.135 m, so radius = 1.0675 m.
        let d = compute_distance((10.0, 0.0), ThrowType::Shotput);
        assert!((d - (10.0 - 1.0675)).abs() < 1e-4, "got {d}");
    }

    #[test]
    fn distance_uses_euclidean_norm() {
        // Discus circle diameter = 2.50 m. Landing at (3, 4) -> norm 5.
        let d = compute_distance((3.0, 4.0), ThrowType::Discus);
        assert!((d - (5.0 - 1.25)).abs() < 1e-4, "got {d}");
    }

    #[test]
    fn distance_negative_y_does_not_break_computation() {
        // Landing past the right sector should still produce a positive distance.
        let d = compute_distance((10.0, -3.0), ThrowType::Hammer);
        let expected = ((100.0_f32 + 9.0).sqrt()) - 1.0675;
        assert!((d - expected).abs() < 1e-3, "got {d}, expected {expected}");
    }

    #[test]
    fn distance_for_javelin_does_not_subtract_runway_length() {
        // Javelin's "circle" field is actually the runway length (16 m), not a
        // circle radius, so the subtraction would produce a wildly wrong value.
        let d = compute_distance((50.0, 0.0), ThrowType::Javelin);
        assert!((d - 50.0).abs() < 1e-4, "got {d}");
    }

    #[test]
    fn sector_in_bounds_returns_none() {
        // Shotput sector angle = 34.92 deg, half-angle = 17.46 deg.
        // tan(17.46 deg) = 0.3145, so a throw at (10, 1) is in-bounds.
        assert!(classify_sector((10.0, 1.0), ThrowType::Shotput).is_none());
    }

    #[test]
    fn sector_straight_down_middle_is_in_bounds() {
        assert!(classify_sector((10.0, 0.0), ThrowType::Discus).is_none());
    }

    #[test]
    fn sector_left_violation_for_positive_y() {
        // (5, 5) is at 45 deg, well outside any sector.
        let infraction = classify_sector((5.0, 5.0), ThrowType::Hammer);
        assert_eq!(infraction, Some(InfractionType::LeftSector));
    }

    #[test]
    fn sector_right_violation_for_negative_y() {
        let infraction = classify_sector((5.0, -5.0), ThrowType::Hammer);
        assert_eq!(infraction, Some(InfractionType::RightSector));
    }

    #[test]
    fn sector_javelin_uses_narrower_angle() {
        // Javelin sector angle = 28.96 deg, half = 14.48 deg.
        // tan(14.48 deg) ~ 0.2582. Landing at (10, 3) -> ratio 0.3 -> outside.
        // The same point is in-bounds for shotput (half-angle 17.46 deg, tan 0.3145).
        assert!(classify_sector((10.0, 3.0), ThrowType::Shotput).is_none());
        assert!(classify_sector((10.0, 3.0), ThrowType::Javelin).is_some());
    }

    #[test]
    fn build_response_in_bounds_no_circle_infraction() {
        let trajectory = traj(&[(0.0, 0.0, 2.0), (10.0, 0.5, 0.05)]);
        let response = build_throw_response(
            &output(trajectory, true),
            ThrowType::Shotput,
            None,
            vec!["a.png".to_string()],
        )
        .expect("should produce a response");
        assert!(response.infractions.is_empty());
        assert_eq!(response.landing_point_x_y, Some((10.0, 0.5)));
        assert!(
            (response.distance_m - (((10.0_f32).powi(2) + 0.25_f32).sqrt() - 1.0675)).abs() < 1e-3
        );
        assert_eq!(response.throw_type, ThrowType::Shotput);
        assert_eq!(response.images, vec!["a.png".to_string()]);
    }

    #[test]
    fn build_response_sector_violation_hides_landing_point() {
        let trajectory = traj(&[(0.0, 0.0, 2.0), (5.0, 5.0, 0.05)]);
        let response = build_throw_response(
            &output(trajectory, true),
            ThrowType::Hammer,
            None,
            vec![],
        )
        .expect("should produce a response");
        assert_eq!(response.landing_point_x_y, None);
        assert_eq!(response.infractions, vec![InfractionType::LeftSector]);
    }

    #[test]
    fn build_response_circle_infraction_passes_through() {
        let trajectory = traj(&[(0.0, 0.0, 2.0), (10.0, 0.0, 0.05)]);
        let response = build_throw_response(
            &output(trajectory, true),
            ThrowType::Shotput,
            Some(InfractionType::Circle),
            vec![],
        )
        .expect("should produce a response");
        assert_eq!(response.infractions, vec![InfractionType::Circle]);
        assert!(response.landing_point_x_y.is_some());
    }

    #[test]
    fn build_response_sector_and_circle_both_reported() {
        let trajectory = traj(&[(0.0, 0.0, 2.0), (5.0, 5.0, 0.05)]);
        let response = build_throw_response(
            &output(trajectory, true),
            ThrowType::Hammer,
            Some(InfractionType::Circle),
            vec![],
        )
        .expect("should produce a response");
        assert_eq!(
            response.infractions,
            vec![InfractionType::LeftSector, InfractionType::Circle]
        );
        assert_eq!(response.landing_point_x_y, None);
    }

    #[test]
    fn build_response_returns_err_when_triangulation_failed() {
        let trajectory = traj(&[(0.0, 0.0, 2.0), (10.0, 0.0, 0.05)]);
        let response = build_throw_response(
            &output(trajectory, false),
            ThrowType::Shotput,
            None,
            vec![],
        );
        assert_eq!(
            response.unwrap_err(),
            "Levenberg-Marquardt optimization did not converge"
        );
    }

    #[test]
    fn build_response_returns_err_for_empty_trajectory() {
        let response = build_throw_response(
            &output(vec![], true),
            ThrowType::Shotput,
            None,
            vec![],
        );
        assert_eq!(
            response.unwrap_err(),
            "Triangulation produced an empty trajectory"
        );
    }

    #[test]
    fn timestamp_ns_round_trips_to_rfc3339() {
        let s = timestamp_ns_to_string(1_775_771_934_343_718_000);
        // Just sanity-check the format; exact value depends on chrono's RFC3339 emission.
        assert!(s.contains('T'));
        assert!(s.contains('-'));
        assert!(s.contains(':'));
    }
}
