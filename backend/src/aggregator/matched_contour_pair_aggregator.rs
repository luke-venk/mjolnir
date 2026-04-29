//! Groups contour outputs by left and right camera, then matches temporally
//! adjacent observations into `MatchedContourPair` values for downstream
//! aggregation and trajectory preparation.

use crate::pipeline::{CameraId, ContourOutput, Frame, MatchedContourPair};
use crossbeam::channel::{Receiver, Sender};
use std::collections::VecDeque;
use std::thread::{self, JoinHandle};

pub struct MatchedContourPairAggregator {
    _handle_output: JoinHandle<()>,
}

impl MatchedContourPairAggregator {
    pub fn new(
        output_rx: Receiver<Frame>,
        matched_pair_tx: Sender<MatchedContourPair>,
        expected_frame_interval_ns: u64,
    ) -> Self {
        let handle_output = thread::spawn(move || {
            let mut left_queue = VecDeque::new();
            let mut right_queue = VecDeque::new();

            for frame in output_rx.iter() {
                let contour_output = ContourOutput::from(frame);
                match contour_output.camera_id() {
                    CameraId::FieldLeft => left_queue.push_back(contour_output),
                    CameraId::FieldRight => right_queue.push_back(contour_output),
                }

                while let (Some(left), Some(right)) = (left_queue.front(), right_queue.front()) {
                    let left_ts = left.camera_buffer_timestamp();
                    let right_ts = right.camera_buffer_timestamp();
                    let delta = left_ts.abs_diff(right_ts);

                    if delta <= expected_frame_interval_ns {
                        let left_match = left_queue.pop_front().unwrap();
                        let right_match = right_queue.pop_front().unwrap();
                        let matched_pair = MatchedContourPair::new(left_match, right_match);
                        let _ = matched_pair_tx.send(matched_pair);
                    } else if left_ts < right_ts {
                        let _ = left_queue.pop_front();
                    } else {
                        let _ = right_queue.pop_front();
                    }
                }
            }
        });

        Self {
            _handle_output: handle_output,
        }
    }
}
