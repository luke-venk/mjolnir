//! Replays matched frame windows through a reduced pipeline path that preserves
//! undistorted 4k imagery, reruns late CV stages, and produces refined
//! `OptimizeTrajectoryInput` values.

use crate::aggregator::{OptimizeTrajectoryInput, TrajectoryInputCollector};
use crate::computer_vision::{contour, mog2};
use crate::pipeline::MatchedFramePair;
use crossbeam::channel::{Receiver, Sender};
use std::thread::{self, JoinHandle};

pub struct RerunPipeline {
    _handle: JoinHandle<()>,
}

impl RerunPipeline {
    pub fn new(
        matched_window_rx: Receiver<Vec<MatchedFramePair>>,
        optimize_input_tx: Sender<OptimizeTrajectoryInput>,
    ) -> Self {
        let handle = thread::spawn(move || {
            for matched_window in matched_window_rx.iter() {
                let rerun_pairs: Vec<MatchedFramePair> = matched_window
                    .into_iter()
                    .map(|matched_pair| {
                        let left = contour(mog2(matched_pair.left().clone()));
                        let right = contour(mog2(matched_pair.right().clone()));
                        MatchedFramePair::new(left, right)
                    })
                    .collect();

                let mut collector = TrajectoryInputCollector::new();
                for matched_pair in rerun_pairs {
                    collector.push(matched_pair);
                }

                if let Some(optimize_input) = collector.build_optimize_trajectory_input() {
                    let _ = optimize_input_tx.send(optimize_input);
                }
            }
        });

        Self { _handle: handle }
    }
}
