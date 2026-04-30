//! Coordinates start-triggered collection of matched left/right frame pairs and
//! emits `OptimizeTrajectoryInput` once a fixed sample count has been gathered.

use crate::aggregator::{OptimizeTrajectoryInput, TrajectoryInputCollector};
use crate::pipeline::{CameraId, Context, Frame, MatchedFramePair};
use crossbeam::channel::{Receiver, Sender, select};
use std::collections::VecDeque;
use std::thread::{self, JoinHandle};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AggregationCommand {
    Start { timestamp_ns: u64 },
    Reset,
}

#[derive(Debug, Clone, PartialEq)]
struct ActiveWindow {
    start_timestamp_ns: u64,
}

pub struct AggregationCoordinator {
    _handle: JoinHandle<()>,
}

impl AggregationCoordinator {
    pub fn new(
        matched_pair_rx: Receiver<MatchedFramePair>,
        command_rx: Receiver<AggregationCommand>,
        optimize_input_tx: Sender<OptimizeTrajectoryInput>,
        lookback_capacity: usize,
        sample_limit: usize,
    ) -> Self {
        let handle = thread::spawn(move || {
            let mut collector = TrajectoryInputCollector::new();
            let mut active_window: Option<ActiveWindow> = None;
            let mut lookback_buffer: VecDeque<MatchedFramePair> =
                VecDeque::with_capacity(lookback_capacity);

            loop {
                select! {
                    recv(command_rx) -> message => {
                        match message {
                            Ok(AggregationCommand::Start { timestamp_ns }) => {
                                collector.clear();
                                for matched_pair in lookback_buffer.iter() {
                                    if matched_pair.pair_timestamp_ns() >= timestamp_ns {
                                        collector.push(matched_pair.clone());
                                    }
                                }
                                active_window = Some(ActiveWindow {
                                    start_timestamp_ns: timestamp_ns,
                                });
                            }
                            Ok(AggregationCommand::Reset) => {
                                collector.clear();
                                active_window = None;
                            }
                            Err(_) => break,
                        }
                    }
                    recv(matched_pair_rx) -> message => {
                        match message {
                            Ok(matched_pair) => {
                                let pair_timestamp_ns = matched_pair.pair_timestamp_ns();

                                lookback_buffer.push_back(matched_pair.clone());
                                while lookback_buffer.len() > lookback_capacity {
                                    let _ = lookback_buffer.pop_front();
                                }

                                if let Some(window) = active_window.as_ref() {
                                    if pair_timestamp_ns < window.start_timestamp_ns {
                                        continue;
                                    }

                                    collector.push(matched_pair);

                                    if collector.len() >= sample_limit {
                                        if let Some(optimize_input) = collector.build_optimize_trajectory_input() {
                                            let _ = optimize_input_tx.send(optimize_input);
                                        }
                                        collector.clear();
                                        active_window = None;
                                    }
                                }
                            }
                            Err(_) => break,
                        }
                    }
                }
            }
        });

        Self { _handle: handle }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_frame(
        camera_id: CameraId,
        timestamp_ns: u64,
        center: Option<(f64, f64)>,
    ) -> Frame {
        let mut frame = Frame::new(
            vec![1, 2, 3, 4].into_boxed_slice(),
            (2, 2),
            Context::new(camera_id, timestamp_ns),
        );
        frame.context_mut().set_detected(Some(center.is_some()));
        frame.context_mut().set_centroid(center);
        frame
    }

    fn make_pair(
        timestamp_ns: u64,
        left_center: Option<(f64, f64)>,
        right_center: Option<(f64, f64)>,
    ) -> MatchedFramePair {
        let left = make_frame(CameraId::FieldLeft, timestamp_ns, left_center);
        let right = make_frame(CameraId::FieldRight, timestamp_ns, right_center);

        MatchedFramePair::new(left, right)
    }

    #[test]
    fn test_coordinator_collects_from_start_and_emits_after_sample_limit() {
        let (matched_pair_tx, matched_pair_rx) = crossbeam::channel::unbounded();
        let (command_tx, command_rx) = crossbeam::channel::unbounded();
        let (optimize_input_tx, optimize_input_rx) = crossbeam::channel::unbounded();

        let _coordinator =
            AggregationCoordinator::new(matched_pair_rx, command_rx, optimize_input_tx, 250, 3);

        command_tx
            .send(AggregationCommand::Start { timestamp_ns: 100 })
            .unwrap();
        matched_pair_tx
            .send(make_pair(90, Some((1.0, 2.0)), Some((3.0, 4.0))))
            .unwrap();
        matched_pair_tx
            .send(make_pair(100, Some((10.0, 20.0)), Some((30.0, 40.0))))
            .unwrap();
        matched_pair_tx
            .send(make_pair(133, Some((11.0, 21.0)), None))
            .unwrap();
        matched_pair_tx
            .send(make_pair(166, Some((12.0, 22.0)), Some((32.0, 42.0))))
            .unwrap();

        let optimize_input = optimize_input_rx.recv().expect("expected optimize input");
        assert_eq!(optimize_input.pixels().len(), 2);
        assert_eq!(optimize_input.pixels()[0].len(), 2);
        assert_eq!(optimize_input.pixels()[1].len(), 2);
        assert_eq!(optimize_input.pixels()[0][0], nalgebra::Vector2::new(10.0, 20.0));
        assert_eq!(optimize_input.pixels()[0][1], nalgebra::Vector2::new(12.0, 22.0));
        assert_eq!(optimize_input.pixels()[1][0], nalgebra::Vector2::new(30.0, 40.0));
        assert_eq!(optimize_input.pixels()[1][1], nalgebra::Vector2::new(32.0, 42.0));
    }
}
