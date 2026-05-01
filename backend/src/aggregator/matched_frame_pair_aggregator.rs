//! Groups frames by left and right camera, then matches temporally adjacent
//! observations into `MatchedFramePair` values for downstream aggregation and
//! trajectory preparation.

use crate::pipeline::{CameraId, Frame, MatchedFramePair};
use crossbeam::channel::{Receiver, Sender};
use std::collections::VecDeque;
use std::thread::{self, JoinHandle};

pub struct MatchedFramePairAggregator {
    _handle_output: JoinHandle<()>,
}

impl MatchedFramePairAggregator {
    pub fn new(
        output_rx: Receiver<Frame>,
        matched_pair_tx: Sender<MatchedFramePair>,
        expected_frame_interval_ns: u64,
    ) -> Self {
        let handle_output = thread::spawn(move || {
            let mut left_queue = VecDeque::new();
            let mut right_queue = VecDeque::new();

            for frame in output_rx.iter() {
                let _ = frame.clear_undistorted_image();
                let _ = frame.clear_downsampled_image();

                match frame.context().camera_id() {
                    CameraId::FieldLeft => left_queue.push_back(frame),
                    CameraId::FieldRight => right_queue.push_back(frame),
                }

                while let (Some(left), Some(right)) = (left_queue.front(), right_queue.front()) {
                    let left_ts = left.context().camera_buffer_timestamp();
                    let right_ts = right.context().camera_buffer_timestamp();
                    let delta = left_ts.abs_diff(right_ts);

                    if delta <= expected_frame_interval_ns {
                        let left_match = left_queue
                            .pop_front()
                            .expect("left queue should contain an item while matching frames");
                        let right_match = right_queue
                            .pop_front()
                            .expect("right queue should contain an item while matching frames");
                        let matched_pair = MatchedFramePair::new(left_match, right_match);
                        let _ = matched_pair_tx.send(matched_pair);
                    } else if left_ts < right_ts {
                        left_queue
                            .pop_front()
                            .expect("left queue should contain an item while pruning unmatched frames");
                    } else {
                        right_queue
                            .pop_front()
                            .expect("right queue should contain an item while pruning unmatched frames");
                    }
                }
            }
        });

        Self {
            _handle_output: handle_output,
        }
    }
}
