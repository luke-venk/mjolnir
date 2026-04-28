use crossbeam::channel::Sender;

use aravis::Buffer;

use crate::camera::aravis_utils::copy_buffer_bytes;
use crate::camera::record::writer::Frame as RecordedFrame;
use crate::pipeline::{CameraId, Context, Frame as PipelineFrame};

// Converts one successful Aravis buffer into the pipeline's frame type.
pub fn buffer_to_frame(
    buffer: &Buffer,
    camera_id: CameraId,
    resolution: (u32, u32),
) -> PipelineFrame {
    let data = copy_buffer_bytes(buffer);
    let timestamp = if buffer.system_timestamp() != 0 {
        buffer.system_timestamp()
    } else if buffer.timestamp() != 0 {
        buffer.timestamp()
    } else {
        buffer.frame_id()
    };

    PipelineFrame::new(
        data.into_boxed_slice(),
        resolution,
        Context::new(camera_id, timestamp),
    )
}

// Converts one recorded frame payload into the pipeline's frame type.
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

pub fn forward_recorded_frame(
    recorded_frame: RecordedFrame,
    left_camera_id: &str,
    right_camera_id: &str,
    left_tx: &Sender<PipelineFrame>,
    right_tx: &Sender<PipelineFrame>,
) -> bool {
    let source_camera_id = recorded_frame.metadata.camera_id.clone();
    let frame_index = recorded_frame.metadata.frame_index;
    let (send_result, destination) = if source_camera_id == left_camera_id {
        (
            left_tx.send(recorded_frame_to_frame(recorded_frame, CameraId::FieldLeft)),
            "left",
        )
    } else if source_camera_id == right_camera_id {
        (
            right_tx.send(recorded_frame_to_frame(
                recorded_frame,
                CameraId::FieldRight,
            )),
            "right",
        )
    } else {
        eprintln!("Received frame for unexpected camera {}.", source_camera_id);
        return true;
    };

    if send_result.is_err() {
        return false;
    }

    println!(
        "camera_ingest: forwarded recorded frame {} from {} into {} pipeline",
        frame_index, source_camera_id, destination
    );
    true
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::camera::record::writer::{Frame as RecordedFrame, Metadata};
    use crate::pipeline::CameraId;

    use crossbeam::channel::bounded;

    use super::{forward_recorded_frame, recorded_frame_sort_key, recorded_frame_to_frame};

    #[test]
    fn recorded_frame_to_frame_prefers_system_timestamp() {
        let frame = RecordedFrame {
            output_camera_dir: Some(PathBuf::new()),
            frame_index: 3,
            bytes: vec![1, 2, 3],
            metadata: Metadata {
                camera_id: "camera-a".to_string(),
                frame_index: 3,
                width: 3,
                height: 1,
                payload_bytes: 3,
                system_timestamp_ns: 123,
                buffer_timestamp_ns: 456,
                frame_id: 789,
            },
        };

        let converted = recorded_frame_to_frame(frame, CameraId::FieldLeft);

        assert_eq!(converted.data(), &[1, 2, 3]);
        assert_eq!(converted.raw_full_resolution(), (3, 1));
        assert_eq!(converted.context().camera_id(), CameraId::FieldLeft);
        assert_eq!(converted.context().timestamp(), 123);
    }

    #[test]
    fn recorded_frame_to_frame_falls_back_to_buffer_timestamp() {
        let frame = RecordedFrame {
            output_camera_dir: Some(PathBuf::new()),
            frame_index: 1,
            bytes: vec![9, 8, 7],
            metadata: Metadata {
                camera_id: "camera-b".to_string(),
                frame_index: 1,
                width: 3,
                height: 1,
                payload_bytes: 3,
                system_timestamp_ns: 0,
                buffer_timestamp_ns: 456,
                frame_id: 789,
            },
        };

        let converted = recorded_frame_to_frame(frame, CameraId::FieldRight);

        assert_eq!(converted.context().timestamp(), 456);
    }

    #[test]
    fn recorded_frame_to_frame_falls_back_to_frame_id() {
        let frame = RecordedFrame {
            output_camera_dir: Some(PathBuf::new()),
            frame_index: 2,
            bytes: vec![4, 5, 6],
            metadata: Metadata {
                camera_id: "camera-c".to_string(),
                frame_index: 2,
                width: 3,
                height: 1,
                payload_bytes: 3,
                system_timestamp_ns: 0,
                buffer_timestamp_ns: 0,
                frame_id: 789,
            },
        };

        let converted = recorded_frame_to_frame(frame, CameraId::FieldLeft);

        assert_eq!(converted.context().timestamp(), 789);
    }

    #[test]
    fn recorded_frame_sort_key_prefers_buffer_then_system_then_frame_id() {
        let frame = RecordedFrame {
            output_camera_dir: Some(PathBuf::new()),
            frame_index: 7,
            bytes: vec![],
            metadata: Metadata {
                camera_id: "camera-z".to_string(),
                frame_index: 7,
                width: 0,
                height: 0,
                payload_bytes: 0,
                system_timestamp_ns: 33,
                buffer_timestamp_ns: 22,
                frame_id: 11,
            },
        };

        assert_eq!(
            recorded_frame_sort_key(&frame),
            (22, 33, "camera-z".to_string(), 7)
        );
    }

    #[test]
    fn forward_recorded_frame_routes_by_camera_id() {
        let frame = RecordedFrame {
            output_camera_dir: Some(PathBuf::new()),
            frame_index: 1,
            bytes: vec![1, 2, 3],
            metadata: Metadata {
                camera_id: "left-camera".to_string(),
                frame_index: 1,
                width: 3,
                height: 1,
                payload_bytes: 3,
                system_timestamp_ns: 123,
                buffer_timestamp_ns: 0,
                frame_id: 0,
            },
        };
        let (left_tx, left_rx) = bounded(1);
        let (right_tx, right_rx) = bounded(1);

        let forwarded =
            forward_recorded_frame(frame, "left-camera", "right-camera", &left_tx, &right_tx);

        assert!(forwarded);
        assert_eq!(left_rx.try_recv().unwrap().context().timestamp(), 123);
        assert!(right_rx.try_recv().is_err());
    }
}
