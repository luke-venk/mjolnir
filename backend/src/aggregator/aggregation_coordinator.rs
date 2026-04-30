//! Coordinates start/end-triggered collection of matched left/right frame pairs
//! and emits finished matched windows for rerun processing.

use crate::pipeline::{CameraId, Context, Frame, MatchedFramePair};
use crossbeam::channel::{Receiver, Sender, select};
use std::collections::VecDeque;
use std::thread::{self, JoinHandle};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AggregationCommand {
    Start { timestamp_ns: u64 },
    End { timestamp_ns: u64 },
    Reset,
}

#[derive(Debug, Clone, PartialEq)]
struct ActiveWindow {
    start_timestamp_ns: u64,
    end_timestamp_ns: Option<u64>,
}

pub struct AggregationCoordinator {
    _handle: JoinHandle<()>,
}

impl AggregationCoordinator {
    pub fn new(
        matched_pair_rx: Receiver<MatchedFramePair>,
        command_rx: Receiver<AggregationCommand>,
        matched_window_tx: Sender<Vec<MatchedFramePair>>,
        lookback_capacity: usize,
    ) -> Self {
        let handle = thread::spawn(move || {
            let mut active_window: Option<ActiveWindow> = None;
            let mut lookback_buffer: VecDeque<MatchedFramePair> =
                VecDeque::with_capacity(lookback_capacity);

            loop {
                select! {
                    recv(command_rx) -> message => {
                        match message {
                            Ok(AggregationCommand::Start { timestamp_ns }) => {
                                active_window = Some(ActiveWindow {
                                    start_timestamp_ns: timestamp_ns,
                                    end_timestamp_ns: None,
                                });
                            }
                            Ok(AggregationCommand::End { timestamp_ns }) => {
                                if let Some(window) = active_window.as_mut() {
                                    window.end_timestamp_ns = Some(timestamp_ns);
                                }
                            }
                            Ok(AggregationCommand::Reset) => {
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

                                if let Some(window) = active_window.as_ref()
                                    && let Some(end_timestamp_ns) = window.end_timestamp_ns
                                    && pair_timestamp_ns > end_timestamp_ns
                                {
                                    let matched_window: Vec<MatchedFramePair> = lookback_buffer
                                        .iter()
                                        .filter(|matched_pair| {
                                            matched_pair.pair_timestamp_ns() >= window.start_timestamp_ns
                                        })
                                        .cloned()
                                        .collect();

                                    if !matched_window.is_empty() {
                                        let _ = matched_window_tx.send(matched_window);
                                    }
                                    active_window = None;
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
    fn test_coordinator_emits_window_after_end_boundary_is_crossed() {
        let (matched_pair_tx, matched_pair_rx) = crossbeam::channel::unbounded();
        let (command_tx, command_rx) = crossbeam::channel::unbounded();
        let (matched_window_tx, matched_window_rx) = crossbeam::channel::unbounded();

        let _coordinator =
            AggregationCoordinator::new(matched_pair_rx, command_rx, matched_window_tx, 250);

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
        command_tx
            .send(AggregationCommand::End { timestamp_ns: 150 })
            .unwrap();
        matched_pair_tx
            .send(make_pair(166, Some((12.0, 22.0)), Some((32.0, 42.0))))
            .unwrap();

        let matched_window = matched_window_rx.recv().expect("expected matched window");
        assert_eq!(matched_window.len(), 3);
        assert_eq!(matched_window[0].pair_timestamp_ns(), 100);
        assert_eq!(matched_window[1].pair_timestamp_ns(), 133);
        assert_eq!(matched_window[2].pair_timestamp_ns(), 166);
    }
}
