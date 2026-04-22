use std::time::{Duration, Instant};

use aravis::Buffer;

use crate::camera::aravis_utils::copy_buffer_bytes;
use crate::camera::record::writer::Frame as RecordedFrame;
use crate::schemas::{Context, Frame as PipelineFrame};

// Converts one successful Aravis buffer into the pipeline's frame type.
pub fn buffer_to_frame(buffer: &Buffer) -> PipelineFrame {
    let data = copy_buffer_bytes(buffer);
    let timestamp = if buffer.system_timestamp() != 0 {
        buffer.system_timestamp()
    } else if buffer.timestamp() != 0 {
        buffer.timestamp()
    } else {
        buffer.frame_id()
    };

    PipelineFrame::new(data, Context::new(timestamp))
}

// Converts one recorded frame payload into the pipeline's frame type.
pub fn recorded_frame_to_frame(frame: RecordedFrame) -> PipelineFrame {
    let timestamp = if frame.metadata.system_timestamp_ns != 0 {
        frame.metadata.system_timestamp_ns
    } else if frame.metadata.buffer_timestamp_ns != 0 {
        frame.metadata.buffer_timestamp_ns
    } else {
        frame.metadata.frame_id
    };

    PipelineFrame::new(frame.bytes, Context::new(timestamp))
}

// Returns true once the configured frame limit has been reached.
pub fn reached_frame_limit(frames_sent: usize, max_frames: Option<usize>) -> bool {
    max_frames.is_some_and(|limit| frames_sent >= limit)
}

// Returns true once the configured recording duration has elapsed.
pub fn reached_duration_limit(start_time: Instant, max_duration_s: Option<f64>) -> bool {
    max_duration_s.is_some_and(|seconds| start_time.elapsed() >= Duration::from_secs_f64(seconds))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    use crate::camera::record::writer::{Frame as RecordedFrame, Metadata};

    use super::{reached_duration_limit, reached_frame_limit, recorded_frame_to_frame};

    #[test]
    fn recorded_frame_to_frame_prefers_system_timestamp() {
        let frame = RecordedFrame {
            output_camera_dir: PathBuf::new(),
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

        let converted = recorded_frame_to_frame(frame);

        assert_eq!(converted.data(), &[1, 2, 3]);
        assert_eq!(converted.context().timestamp(), 123);
    }

    #[test]
    fn recorded_frame_to_frame_falls_back_to_buffer_timestamp() {
        let frame = RecordedFrame {
            output_camera_dir: PathBuf::new(),
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

        let converted = recorded_frame_to_frame(frame);

        assert_eq!(converted.context().timestamp(), 456);
    }

    #[test]
    fn recorded_frame_to_frame_falls_back_to_frame_id() {
        let frame = RecordedFrame {
            output_camera_dir: PathBuf::new(),
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

        let converted = recorded_frame_to_frame(frame);

        assert_eq!(converted.context().timestamp(), 789);
    }

    #[test]
    fn reached_frame_limit_handles_some_and_none() {
        assert!(!reached_frame_limit(0, None));
        assert!(!reached_frame_limit(1, Some(2)));
        assert!(reached_frame_limit(2, Some(2)));
    }

    #[test]
    fn reached_duration_limit_handles_some_and_none() {
        assert!(!reached_duration_limit(Instant::now(), None));
        assert!(!reached_duration_limit(Instant::now(), Some(1.0)));

        let started_earlier = Instant::now() - Duration::from_millis(20);
        assert!(reached_duration_limit(started_earlier, Some(0.001)));
    }
}
