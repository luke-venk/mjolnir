use crossbeam::channel::Sender;

use crate::camera::record::writer::Frame as RecordedFrame;
use crate::pipeline::{CameraId, Context, Frame as PipelineFrame};

/// Converts one recorded frame payload into the pipeline's frame type.
pub fn recorded_frame_to_frame(frame: RecordedFrame, camera_id: CameraId) -> PipelineFrame {
    let timestamp = if frame.metadata.system_timestamp_ns != 0 {
        frame.metadata.system_timestamp_ns
    } else if frame.metadata.buffer_timestamp_ns != 0 {
        frame.metadata.buffer_timestamp_ns
    } else {
        frame.metadata.frame_id
    };
    let resolution = (frame.metadata.width, frame.metadata.height);

    PipelineFrame::new(
        frame.bytes.into_boxed_slice(),
        resolution,
        Context::new(camera_id, timestamp),
    )
}

/// Sort key used to interleave the two cameras' recorded frames in the
/// timestamp order they were captured. Prefers `buffer_timestamp_ns`, then
/// `system_timestamp_ns`, then `frame_id`. The trailing fields are
/// tie-breakers so the ordering is deterministic.
pub fn recorded_frame_sort_key(frame: &RecordedFrame) -> (u64, u64, String, usize) {
    let primary_timestamp = if frame.metadata.buffer_timestamp_ns != 0 {
        frame.metadata.buffer_timestamp_ns
    } else if frame.metadata.system_timestamp_ns != 0 {
        frame.metadata.system_timestamp_ns
    } else {
        frame.metadata.frame_id
    };
    let secondary_timestamp = if frame.metadata.system_timestamp_ns != 0 {
        frame.metadata.system_timestamp_ns
    } else {
        frame.metadata.buffer_timestamp_ns
    };

    (
        primary_timestamp,
        secondary_timestamp,
        frame.metadata.camera_id.clone(),
        frame.metadata.frame_index,
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
    fn recorded_frame_to_frame_prefers_system_timestamp() {
        let frame = make_frame(
            Metadata {
                camera_id: "camera-a".to_string(),
                frame_index: 3,
                width: 3,
                height: 1,
                payload_bytes: 3,
                system_timestamp_ns: 123,
                buffer_timestamp_ns: 456,
                frame_id: 789,
            },
            vec![1, 2, 3],
        );

        let converted = recorded_frame_to_frame(frame, CameraId::FieldLeft);

        assert_eq!(converted.raw_bytes_full_resolution().as_ref(), &[1, 2, 3]);
        assert_eq!(converted.raw_full_resolution(), (3, 1));
        assert_eq!(converted.context().camera_id(), CameraId::FieldLeft);
        assert_eq!(converted.context().camera_buffer_timestamp(), 123);
    }

    #[test]
    fn recorded_frame_to_frame_falls_back_to_buffer_timestamp() {
        let frame = make_frame(
            Metadata {
                camera_id: "camera-b".to_string(),
                frame_index: 1,
                width: 3,
                height: 1,
                payload_bytes: 3,
                system_timestamp_ns: 0,
                buffer_timestamp_ns: 456,
                frame_id: 789,
            },
            vec![9, 8, 7],
        );

        let converted = recorded_frame_to_frame(frame, CameraId::FieldRight);

        assert_eq!(converted.context().camera_buffer_timestamp(), 456);
    }

    #[test]
    fn recorded_frame_to_frame_falls_back_to_frame_id() {
        let frame = make_frame(
            Metadata {
                camera_id: "camera-c".to_string(),
                frame_index: 2,
                width: 3,
                height: 1,
                payload_bytes: 3,
                system_timestamp_ns: 0,
                buffer_timestamp_ns: 0,
                frame_id: 789,
            },
            vec![4, 5, 6],
        );

        let converted = recorded_frame_to_frame(frame, CameraId::FieldLeft);

        assert_eq!(converted.context().camera_buffer_timestamp(), 789);
    }

    #[test]
    fn recorded_frame_sort_key_prefers_buffer_then_system_then_frame_id() {
        let frame = make_frame(
            Metadata {
                camera_id: "camera-z".to_string(),
                frame_index: 7,
                width: 0,
                height: 0,
                payload_bytes: 0,
                system_timestamp_ns: 33,
                buffer_timestamp_ns: 22,
                frame_id: 11,
            },
            vec![],
        );

        assert_eq!(
            recorded_frame_sort_key(&frame),
            (22, 33, "camera-z".to_string(), 7)
        );
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
                system_timestamp_ns: 123,
                buffer_timestamp_ns: 0,
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
        assert_eq!(left_rx.try_recv().unwrap().context().camera_buffer_timestamp(), 123);
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
                system_timestamp_ns: 1,
                buffer_timestamp_ns: 0,
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
