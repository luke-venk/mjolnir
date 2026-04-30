use crossbeam::channel::Sender;

use crate::camera::record::writer::{Frame as RecordedFrame, Metadata};
use crate::pipeline::{CameraId, Context, Frame as PipelineFrame};

/// Converts one recorded frame payload into the pipeline's frame type.
///
/// Uses `buffer_timestamp_ns` (the camera's PTP-synchronized timestamp)
/// exclusively. Asserts it is non-zero, since `system_timestamp_ns` is on a
/// different (host wall-clock) time scale and `frame_id` is a counter, not a
/// timestamp.
pub fn recorded_frame_to_frame(frame: RecordedFrame, camera_id: CameraId) -> PipelineFrame {
    let timestamp = frame.metadata.buffer_timestamp_ns;
    assert!(
        timestamp != 0,
        "recorded frame {} for camera {} has zero buffer_timestamp_ns",
        frame.metadata.frame_index, frame.metadata.camera_id
    );
    let resolution = (frame.metadata.width, frame.metadata.height);

    PipelineFrame::new(
        frame.bytes.into_boxed_slice(),
        resolution,
        Context::new(camera_id, timestamp),
    )
}

/// Sort key used to interleave the two cameras' recorded frames in the order
/// they were captured. Sorts by `buffer_timestamp_ns` (the camera's PTP-
/// synchronized timestamp). Asserts it is non-zero. Trailing fields
/// (`camera_id`, `frame_index`) are deterministic tie-breakers when two
/// frames share a buffer timestamp.
///
/// Takes `&Metadata` (not `&RecordedFrame`) so callers can sort on lightweight
/// metadata sidecars without loading the frame payload bytes off disk first.
pub fn recorded_frame_sort_key(metadata: &Metadata) -> (u64, String, usize) {
    let timestamp = metadata.buffer_timestamp_ns;
    assert!(
        timestamp != 0,
        "recorded frame {} for camera {} has zero buffer_timestamp_ns",
        metadata.frame_index, metadata.camera_id
    );

    (
        timestamp,
        metadata.camera_id.clone(),
        metadata.frame_index,
    )
}

/// Sends one recorded frame into the pipeline matching `camera_id` (which is
/// determined upstream by which subdirectory the frame was loaded from).
/// Returns `false` if the destination channel has been dropped.
pub fn forward_recorded_frame(
    camera_id: CameraId,
    recorded_frame: RecordedFrame,
    left_tx: &Sender<PipelineFrame>,
    right_tx: &Sender<PipelineFrame>,
) -> bool {
    let frame_index = recorded_frame.metadata.frame_index;
    let pipeline_frame = recorded_frame_to_frame(recorded_frame, camera_id);

    let send_result = match camera_id {
        CameraId::FieldLeft => left_tx.send(pipeline_frame),
        CameraId::FieldRight => right_tx.send(pipeline_frame),
    };

    if send_result.is_err() {
        return false;
    }

    println!(
        "camera_ingest: forwarded recorded frame {} into {:?} pipeline",
        frame_index, camera_id
    );
    true
}

#[cfg(test)]
mod tests {
    use crate::camera::record::writer::{Frame as RecordedFrame, Metadata};
    use crate::pipeline::CameraId;

    use crossbeam::channel::bounded;
    use std::path::PathBuf;

    use super::{forward_recorded_frame, recorded_frame_sort_key, recorded_frame_to_frame};

    fn make_frame(metadata: Metadata, bytes: Vec<u8>) -> RecordedFrame {
        RecordedFrame {
            output_camera_dir: PathBuf::new(),
            frame_index: metadata.frame_index,
            bytes,
            metadata,
        }
    }

    #[test]
    fn recorded_frame_to_frame_uses_buffer_timestamp() {
        let frame = make_frame(
            Metadata {
                camera_id: "camera-a".to_string(),
                frame_index: 3,
                width: 3,
                height: 1,
                payload_bytes: 3,
                system_timestamp_ns: 999,
                buffer_timestamp_ns: 456,
                frame_id: 789,
            },
            vec![1, 2, 3],
        );

        let converted = recorded_frame_to_frame(frame, CameraId::FieldLeft);

        assert_eq!(converted.raw_bytes_full_resolution().as_ref(), &[1, 2, 3]);
        assert_eq!(converted.raw_full_resolution(), (3, 1));
        assert_eq!(converted.context().camera_id(), CameraId::FieldLeft);
        assert_eq!(converted.context().camera_buffer_timestamp(), 456);
    }

    #[test]
    #[should_panic(expected = "zero buffer_timestamp_ns")]
    fn recorded_frame_to_frame_panics_on_zero_buffer_timestamp() {
        let frame = make_frame(
            Metadata {
                camera_id: "camera-c".to_string(),
                frame_index: 2,
                width: 3,
                height: 1,
                payload_bytes: 3,
                system_timestamp_ns: 999,
                buffer_timestamp_ns: 0,
                frame_id: 789,
            },
            vec![4, 5, 6],
        );

        let _ = recorded_frame_to_frame(frame, CameraId::FieldLeft);
    }

    #[test]
    fn recorded_frame_sort_key_uses_buffer_timestamp_with_deterministic_tiebreakers() {
        let frame = make_frame(
            Metadata {
                camera_id: "camera-z".to_string(),
                frame_index: 7,
                width: 0,
                height: 0,
                payload_bytes: 0,
                system_timestamp_ns: 999,
                buffer_timestamp_ns: 22,
                frame_id: 11,
            },
            vec![],
        );

        assert_eq!(
            recorded_frame_sort_key(&frame.metadata),
            (22, "camera-z".to_string(), 7)
        );
    }

    #[test]
    #[should_panic(expected = "zero buffer_timestamp_ns")]
    fn recorded_frame_sort_key_panics_on_zero_buffer_timestamp() {
        let frame = make_frame(
            Metadata {
                camera_id: "camera-z".to_string(),
                frame_index: 7,
                width: 0,
                height: 0,
                payload_bytes: 0,
                system_timestamp_ns: 999,
                buffer_timestamp_ns: 0,
                frame_id: 11,
            },
            vec![],
        );

        let _ = recorded_frame_sort_key(&frame.metadata);
    }

    #[test]
    fn forward_recorded_frame_routes_to_pipeline_matching_camera_id() {
        let left_frame = make_frame(
            Metadata {
                camera_id: "anything".to_string(),
                frame_index: 1,
                width: 3,
                height: 1,
                payload_bytes: 3,
                system_timestamp_ns: 0,
                buffer_timestamp_ns: 123,
                frame_id: 0,
            },
            vec![1, 2, 3],
        );
        let (left_tx, left_rx) = bounded(1);
        let (right_tx, right_rx) = bounded(1);

        assert!(forward_recorded_frame(
            CameraId::FieldLeft,
            left_frame,
            &left_tx,
            &right_tx,
        ));
        assert_eq!(
            left_rx.try_recv().unwrap().context().camera_buffer_timestamp(),
            123
        );
        assert!(right_rx.try_recv().is_err());
    }

    #[test]
    fn forward_recorded_frame_returns_false_when_destination_channel_is_dropped() {
        let frame = make_frame(
            Metadata {
                camera_id: "anything".to_string(),
                frame_index: 0,
                width: 1,
                height: 1,
                payload_bytes: 1,
                system_timestamp_ns: 0,
                buffer_timestamp_ns: 1,
                frame_id: 0,
            },
            vec![1],
        );
        let (left_tx, _left_rx) = bounded(1);
        let (right_tx, right_rx) = bounded(1);
        drop(right_rx);

        assert!(!forward_recorded_frame(
            CameraId::FieldRight,
            frame,
            &left_tx,
            &right_tx,
        ));
    }
}
