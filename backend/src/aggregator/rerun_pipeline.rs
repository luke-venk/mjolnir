//! Replays matched frame windows through a reduced pipeline path that preserves
//! undistorted 4k imagery, reruns late CV stages, and produces refined
//! `OptimizeTrajectoryInput` values.

use crate::aggregator::{OptimizeTrajectoryInput, TrajectoryInputCollector};
use crate::computer_vision::{contour_with_options, mog2_with_options};
use crate::math_triangulation::math_triangulation::optimize_trajectory;
use crate::pipeline::{Frame, MatchedFramePair, PipelineStageOptions};
use crossbeam::channel::{Receiver, Sender};
use nalgebra::{Matrix3x4, Vector3};
use std::thread::{self, JoinHandle};

pub struct RerunPipeline {
    _handle: JoinHandle<()>,
}

impl RerunPipeline {
    pub fn new(
        matched_window_rx: Receiver<Vec<MatchedFramePair>>,
        optimize_input_tx: Sender<OptimizeTrajectoryInput>,
    ) -> Self {
        let stage_options = PipelineStageOptions { rerun_4k_mode: true };

        let handle = thread::spawn(move || {
            for matched_window in matched_window_rx.iter() {
                let rerun_pairs: Vec<MatchedFramePair> = matched_window
                    .into_iter()
                    .map(|matched_pair| {
                        let left = rerun_frame(matched_pair.left().clone(), stage_options);
                        let right = rerun_frame(matched_pair.right().clone(), stage_options);
                        MatchedFramePair::new(left, right)
                    })
                    .collect();

                let mut collector = TrajectoryInputCollector::new();
                for matched_pair in rerun_pairs {
                    collector.push(matched_pair);
                }

                if let Some(optimize_input) = collector.build_optimize_trajectory_input() {
                    let _ = optimize_input_tx.send(optimize_input.clone());

                    let pixels = optimize_input.pixels().to_vec();
                    let dt_s = optimize_input.dt_s();
                    let p_list = default_projection_matrices();

                    let _ = futures::executor::block_on(optimize_trajectory(
                        &p_list,
                        &pixels,
                        dt_s,
                        Some(Vector3::new(0.0, 0.0, -9.81)),
                        0.0,
                        1.0,
                        1.0,
                        1.0,
                    ));
                }
            }
        });

        Self { _handle: handle }
    }
}

fn rerun_frame(frame: Frame, stage_options: PipelineStageOptions) -> Frame {
    let frame = mog2_with_options(frame, stage_options);
    contour_with_options(frame, stage_options)
}

fn default_projection_matrices() -> Vec<Matrix3x4<f64>> {
    vec![
        Matrix3x4::new(1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0),
        Matrix3x4::new(1.0, 0.0, 0.0, -0.55, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0),
    ]
}
