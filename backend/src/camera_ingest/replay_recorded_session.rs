// Replays one recorded session directory into the left and right pipeline
// inputs.
//
// Convention: the session directory is expected to contain a `left_cam/`
// subdirectory and a `right_cam/` subdirectory. Each subdirectory holds the
// per-frame `.tiff` (or `.raw`) + `.json` pair that
// `crate::camera::record::writer::write_to_disk` produces. Frames in the
// `left_cam/` directory are forwarded into the FieldLeft pipeline; frames in
// `right_cam/` go to FieldRight.
//
// Frame routing here is purely folder-based — there is no manifest. Camera
// assignment (which physical camera ends up in `left_cam` vs `right_cam`) is
// the recorder's responsibility and is out of scope for replay.

use crate::camera::record::writer::{Frame as RecordedFrame, read_recorded_frame};
use crate::camera_ingest::camera_ingest_helpers::{
    forward_recorded_frame, recorded_frame_sort_key,
};
use crate::pipeline::{CameraId, Frame as PipelineFrame};
use crossbeam::channel::Sender;
use std::{
    fs,
    path::{Path, PathBuf},
};

const LEFT_CAM_DIR: &str = "left_cam";
const RIGHT_CAM_DIR: &str = "right_cam";

pub fn replay_recorded_session(
    footage_dir: PathBuf,
    left_tx: Sender<PipelineFrame>,
    right_tx: Sender<PipelineFrame>,
) {
    let left_dir = footage_dir.join(LEFT_CAM_DIR);
    let right_dir = footage_dir.join(RIGHT_CAM_DIR);
    if !left_dir.is_dir() {
        panic!(
            "Expected `{LEFT_CAM_DIR}` subdirectory in {}.",
            footage_dir.display()
        );
    }
    if !right_dir.is_dir() {
        panic!(
            "Expected `{RIGHT_CAM_DIR}` subdirectory in {}.",
            footage_dir.display()
        );
    }

    let mut frames: Vec<(CameraId, RecordedFrame)> = Vec::new();
    for json_path in collect_frame_json_paths(&left_dir) {
        frames.push((CameraId::FieldLeft, read_recorded_frame(&json_path)));
    }
    for json_path in collect_frame_json_paths(&right_dir) {
        frames.push((CameraId::FieldRight, read_recorded_frame(&json_path)));
    }

    if frames.is_empty() {
        panic!(
            "No recorded frame metadata files were found in {} (looked under {LEFT_CAM_DIR}/ and {RIGHT_CAM_DIR}/).",
            footage_dir.display()
        );
    }

    frames.sort_by_key(|(_, frame)| recorded_frame_sort_key(frame));

    println!(
        "camera_ingest: replaying {} recorded frame(s) from {}.",
        frames.len(),
        footage_dir.display()
    );

    for (camera_id, recorded_frame) in frames {
        if !forward_recorded_frame(camera_id, recorded_frame, &left_tx, &right_tx) {
            break;
        }
    }
}

fn collect_frame_json_paths(dir: &Path) -> Vec<PathBuf> {
    let mut json_paths = Vec::new();
    walk_for_frame_json(dir, &mut json_paths);
    json_paths
}

fn walk_for_frame_json(dir: &Path, frame_json_paths: &mut Vec<PathBuf>) {
    let mut entries = fs::read_dir(dir)
        .unwrap_or_else(|err| panic!("Failed to read directory {}: {err}", dir.display()))
        .collect::<Result<Vec<_>, _>>()
        .unwrap_or_else(|err| panic!("Failed to enumerate directory {}: {err}", dir.display()));

    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            walk_for_frame_json(&path, frame_json_paths);
            continue;
        }
        if path.extension().is_some_and(|ext| ext == "json") {
            frame_json_paths.push(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use crossbeam::channel::bounded;

    use super::{LEFT_CAM_DIR, RIGHT_CAM_DIR, replay_recorded_session};
    use crate::camera::record::writer::Metadata;
    use crate::pipeline::{CameraId, Frame as PipelineFrame};

    fn make_temp_session(suffix: &str) -> PathBuf {
        env::temp_dir().join(format!(
            "mjolnir_replay_{suffix}_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time should be after epoch")
                .as_nanos()
        ))
    }

    fn write_test_recorded_frame(dir: &PathBuf, metadata: &Metadata, bytes: &[u8]) {
        let frame_name = format!("frame_{:04}", metadata.frame_index);
        fs::write(dir.join(format!("{frame_name}.raw")), bytes).expect("write frame raw");
        fs::write(
            dir.join(format!("{frame_name}.json")),
            serde_json::to_vec_pretty(metadata).expect("serialize metadata"),
        )
        .expect("write frame metadata");
    }

    #[test]
    fn routes_frames_from_left_cam_and_right_cam_subdirectories() {
        let session = make_temp_session("routes");
        let left_dir = session.join(LEFT_CAM_DIR);
        let right_dir = session.join(RIGHT_CAM_DIR);
        fs::create_dir_all(&left_dir).expect("create left dir");
        fs::create_dir_all(&right_dir).expect("create right dir");

        write_test_recorded_frame(
            &left_dir,
            &Metadata {
                camera_id: "left-cam-serial".to_string(),
                frame_index: 0,
                width: 4,
                height: 1,
                payload_bytes: 4,
                system_timestamp_ns: 300,
                buffer_timestamp_ns: 200,
                frame_id: 3,
            },
            &[1, 2, 3, 4],
        );
        write_test_recorded_frame(
            &right_dir,
            &Metadata {
                camera_id: "right-cam-serial".to_string(),
                frame_index: 0,
                width: 4,
                height: 1,
                payload_bytes: 4,
                system_timestamp_ns: 320,
                buffer_timestamp_ns: 210,
                frame_id: 4,
            },
            &[5, 6, 7, 8],
        );

        let (left_tx, left_rx) = bounded::<PipelineFrame>(4);
        let (right_tx, right_rx) = bounded::<PipelineFrame>(4);
        replay_recorded_session(session.clone(), left_tx, right_tx);

        let left_frames: Vec<_> = left_rx.try_iter().collect();
        let right_frames: Vec<_> = right_rx.try_iter().collect();

        assert_eq!(left_frames.len(), 1);
        assert_eq!(left_frames[0].raw_bytes_full_resolution().as_ref(), &[1, 2, 3, 4]);
        assert_eq!(left_frames[0].context().camera_id(), CameraId::FieldLeft);
        assert_eq!(left_frames[0].context().camera_buffer_timestamp(), 200);
        assert_eq!(right_frames.len(), 1);
        assert_eq!(right_frames[0].raw_bytes_full_resolution().as_ref(), &[5, 6, 7, 8]);
        assert_eq!(right_frames[0].context().camera_id(), CameraId::FieldRight);
        assert_eq!(right_frames[0].context().camera_buffer_timestamp(), 210);

        let _ = fs::remove_dir_all(session);
    }

    #[test]
    fn frames_emitted_in_buffer_timestamp_order() {
        let session = make_temp_session("ordered");
        let left_dir = session.join(LEFT_CAM_DIR);
        let right_dir = session.join(RIGHT_CAM_DIR);
        fs::create_dir_all(&left_dir).expect("create left dir");
        fs::create_dir_all(&right_dir).expect("create right dir");

        // Buffer timestamps: right=100, left=200, right=300.
        // The interleaved replay order should preserve that ordering.
        write_test_recorded_frame(
            &left_dir,
            &Metadata {
                camera_id: "left".to_string(),
                frame_index: 0,
                width: 1,
                height: 1,
                payload_bytes: 1,
                system_timestamp_ns: 0,
                buffer_timestamp_ns: 200,
                frame_id: 1,
            },
            &[10],
        );
        write_test_recorded_frame(
            &right_dir,
            &Metadata {
                camera_id: "right".to_string(),
                frame_index: 0,
                width: 1,
                height: 1,
                payload_bytes: 1,
                system_timestamp_ns: 0,
                buffer_timestamp_ns: 100,
                frame_id: 2,
            },
            &[20],
        );
        write_test_recorded_frame(
            &right_dir,
            &Metadata {
                camera_id: "right".to_string(),
                frame_index: 1,
                width: 1,
                height: 1,
                payload_bytes: 1,
                system_timestamp_ns: 0,
                buffer_timestamp_ns: 300,
                frame_id: 3,
            },
            &[21],
        );

        let (left_tx, left_rx) = bounded::<PipelineFrame>(4);
        let (right_tx, right_rx) = bounded::<PipelineFrame>(4);
        replay_recorded_session(session.clone(), left_tx, right_tx);

        // Right-100 first, then Left-200, then Right-300.
        let right_collected: Vec<u64> = right_rx
            .try_iter()
            .map(|f| f.context().camera_buffer_timestamp())
            .collect();
        let left_collected: Vec<u64> = left_rx
            .try_iter()
            .map(|f| f.context().camera_buffer_timestamp())
            .collect();
        assert_eq!(right_collected, vec![100, 300]);
        assert_eq!(left_collected, vec![200]);

        let _ = fs::remove_dir_all(session);
    }

    #[test]
    #[should_panic(expected = "Expected `left_cam` subdirectory")]
    fn missing_left_cam_subdir_panics() {
        let session = make_temp_session("missing_left");
        fs::create_dir_all(session.join(RIGHT_CAM_DIR)).expect("create right only");

        let (left_tx, _left_rx) = bounded::<PipelineFrame>(1);
        let (right_tx, _right_rx) = bounded::<PipelineFrame>(1);
        replay_recorded_session(session, left_tx, right_tx);
    }

    #[test]
    #[should_panic(expected = "Expected `right_cam` subdirectory")]
    fn missing_right_cam_subdir_panics() {
        let session = make_temp_session("missing_right");
        fs::create_dir_all(session.join(LEFT_CAM_DIR)).expect("create left only");

        let (left_tx, _left_rx) = bounded::<PipelineFrame>(1);
        let (right_tx, _right_rx) = bounded::<PipelineFrame>(1);
        replay_recorded_session(session, left_tx, right_tx);
    }
}
