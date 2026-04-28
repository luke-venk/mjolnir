use crate::schemas::MatchedContourPair;
use nalgebra::Vector2;

#[derive(Debug, Clone, PartialEq)]
pub struct OptimizeTrajectoryInput {
    pixels: Vec<Vec<Vector2<f64>>>,
    dt_s: f64,
}

impl OptimizeTrajectoryInput {
    pub fn new(pixels: Vec<Vec<Vector2<f64>>>, dt_s: f64) -> Self {
        Self { pixels, dt_s }
    }

    pub fn pixels(&self) -> &[Vec<Vector2<f64>>] {
        &self.pixels
    }

    pub fn dt_s(&self) -> f64 {
        self.dt_s
    }
}

#[derive(Debug, Default)]
pub struct TrajectoryInputCollector {
    matched_pairs: Vec<MatchedContourPair>,
}

impl TrajectoryInputCollector {
    pub fn new() -> Self {
        Self {
            matched_pairs: Vec::new(),
        }
    }

    pub fn push(&mut self, matched_pair: MatchedContourPair) {
        self.matched_pairs.push(matched_pair);
    }

    pub fn clear(&mut self) {
        self.matched_pairs.clear();
    }

    pub fn len(&self) -> usize {
        self.matched_pairs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.matched_pairs.is_empty()
    }

    pub fn matched_pairs(&self) -> &[MatchedContourPair] {
        &self.matched_pairs
    }

    pub fn build_optimize_trajectory_input(&self) -> Option<OptimizeTrajectoryInput> {
        self.build_optimize_trajectory_input_from_all_pairs()
    }

    pub fn build_optimize_trajectory_input_from_all_pairs(
        &self,
    ) -> Option<OptimizeTrajectoryInput> {
        let usable_pairs: Vec<&MatchedContourPair> = self
            .matched_pairs
            .iter()
            .filter(|pair| pair.left().center_px().is_some() && pair.right().center_px().is_some())
            .collect();

        if usable_pairs.len() < 2 {
            return None;
        }

        let left_pixels: Vec<Vector2<f64>> = usable_pairs
            .iter()
            .map(|pair| {
                let center = pair.left().center_px().expect("left center should exist");
                Vector2::new(center.cx_px(), center.cy_px())
            })
            .collect();

        let right_pixels: Vec<Vector2<f64>> = usable_pairs
            .iter()
            .map(|pair| {
                let center = pair.right().center_px().expect("right center should exist");
                Vector2::new(center.cx_px(), center.cy_px())
            })
            .collect();

        let dt_s = median_dt_seconds(&usable_pairs)?;

        Some(OptimizeTrajectoryInput::new(vec![left_pixels, right_pixels], dt_s))
    }
}

fn median_dt_seconds(usable_pairs: &[&MatchedContourPair]) -> Option<f64> {
    if usable_pairs.len() < 2 {
        return None;
    }

    let mut deltas_ns: Vec<u64> = usable_pairs
        .windows(2)
        .map(|window| {
            window[1]
                .pair_timestamp_ns()
                .abs_diff(window[0].pair_timestamp_ns())
        })
        .filter(|delta_ns| *delta_ns > 0)
        .collect();

    if deltas_ns.is_empty() {
        return None;
    }

    deltas_ns.sort_unstable();
    let mid = deltas_ns.len() / 2;
    let median_ns = if deltas_ns.len() % 2 == 1 {
        deltas_ns[mid] as f64
    } else {
        (deltas_ns[mid - 1] as f64 + deltas_ns[mid] as f64) / 2.0
    };

    Some(median_ns / 1_000_000_000.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schemas::{CameraId, ContourOutput, Context, MatchedContourPair, PixelCenter};

    fn make_pair(
        timestamp_ns: u64,
        left_center: Option<(f64, f64)>,
        right_center: Option<(f64, f64)>,
    ) -> MatchedContourPair {
        let left = ContourOutput::new(
            Context::new(CameraId::FieldLeft, 0, timestamp_ns, timestamp_ns),
            left_center.map(|(cx, cy)| PixelCenter::new(cx, cy)),
        );
        let right = ContourOutput::new(
            Context::new(CameraId::FieldRight, 0, timestamp_ns, timestamp_ns),
            right_center.map(|(cx, cy)| PixelCenter::new(cx, cy)),
        );

        MatchedContourPair::new(left, right)
    }

    #[test]
    fn test_build_optimize_trajectory_input_filters_missing_detections_and_uses_median_dt() {
        let mut collector = TrajectoryInputCollector::new();
        collector.push(make_pair(100, Some((10.0, 20.0)), Some((30.0, 40.0))));
        collector.push(make_pair(133, Some((11.0, 21.0)), Some((31.0, 41.0))));
        collector.push(make_pair(166, Some((12.0, 22.0)), None));
        collector.push(make_pair(200, Some((13.0, 23.0)), Some((33.0, 43.0))));

        let input = collector
            .build_optimize_trajectory_input()
            .expect("expected optimize trajectory input");

        assert_eq!(input.pixels().len(), 2);
        assert_eq!(input.pixels()[0].len(), 3);
        assert_eq!(input.pixels()[1].len(), 3);
        assert_eq!(input.pixels()[0][0], Vector2::new(10.0, 20.0));
        assert_eq!(input.pixels()[0][1], Vector2::new(11.0, 21.0));
        assert_eq!(input.pixels()[0][2], Vector2::new(13.0, 23.0));
        assert_eq!(input.pixels()[1][0], Vector2::new(30.0, 40.0));
        assert_eq!(input.pixels()[1][1], Vector2::new(31.0, 41.0));
        assert_eq!(input.pixels()[1][2], Vector2::new(33.0, 43.0));
        assert!((input.dt_s() - 50.0e-9).abs() < 1.0e-12);
    }

    #[test]
    fn test_build_optimize_trajectory_input_returns_none_if_too_few_valid_pairs() {
        let mut collector = TrajectoryInputCollector::new();
        collector.push(make_pair(100, Some((10.0, 20.0)), None));
        collector.push(make_pair(133, Some((11.0, 21.0)), Some((31.0, 41.0))));

        assert!(collector.build_optimize_trajectory_input().is_none());
    }
}
