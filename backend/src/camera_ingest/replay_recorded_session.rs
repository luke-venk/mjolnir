use crate::camera::record::writer::{
    Frame as RecordedFrame, SESSION_MANIFEST_FILE_NAME, SessionManifest, read_recorded_frame,
    read_session_manifest,
};
use crate::camera_ingest::camera_ingest_helpers::{
    forward_recorded_frame, recorded_frame_sort_key,
};
use crate::pipeline::Frame as PipelineFrame;
use crossbeam::channel::Sender;
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug)]
struct LoadedRecordedSession {
    left_camera_id: String,
    right_camera_id: String,
    frames: Vec<RecordedFrame>,
}

/// Replays one recorded session directory into the same left/right pipeline
/// entry points that the live camera path uses.
pub fn replay_recorded_session(
    footage_dir: PathBuf,
    left_tx: Sender<PipelineFrame>,
    right_tx: Sender<PipelineFrame>,
) {
    let session = load_recorded_session(&footage_dir);
    println!(
        "camera_ingest: replaying {} recorded frame(s) from {} with left={} and right={}",
        session.frames.len(),
        footage_dir.display(),
        session.left_camera_id,
        session.right_camera_id
    );

    for recorded_frame in session.frames {
        if !forward_recorded_frame(
            recorded_frame,
            &session.left_camera_id,
            &session.right_camera_id,
            &left_tx,
            &right_tx,
        ) {
            break;
        }
    }
}

fn load_recorded_session(footage_dir: &Path) -> LoadedRecordedSession {
    let manifest = read_session_manifest(footage_dir);
    let mut frame_json_paths = Vec::new();
    collect_frame_json_paths(footage_dir, &mut frame_json_paths);
    if frame_json_paths.is_empty() {
        panic!(
            "No recorded frame metadata files were found in {}.",
            footage_dir.display()
        );
    }

    let mut frames = frame_json_paths
        .into_iter()
        .map(|json_path| read_recorded_frame(&json_path))
        .collect::<Vec<_>>();
    frames.sort_by_key(recorded_frame_sort_key);

    let (left_camera_id, right_camera_id) = resolve_camera_mapping(&frames, manifest);

    LoadedRecordedSession {
        left_camera_id,
        right_camera_id,
        frames,
    }
}

fn collect_frame_json_paths(dir: &Path, frame_json_paths: &mut Vec<PathBuf>) {
    let mut entries = fs::read_dir(dir)
        .unwrap_or_else(|err| panic!("Failed to read directory {}: {err}", dir.display()))
        .collect::<Result<Vec<_>, _>>()
        .unwrap_or_else(|err| panic!("Failed to enumerate directory {}: {err}", dir.display()));

    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            collect_frame_json_paths(&path, frame_json_paths);
            continue;
        }

        let is_json = path.extension().is_some_and(|ext| ext == "json");
        let is_manifest = path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name == SESSION_MANIFEST_FILE_NAME);
        if is_json && !is_manifest {
            frame_json_paths.push(path);
        }
    }
}

fn resolve_camera_mapping(
    frames: &[RecordedFrame],
    manifest: Option<SessionManifest>,
) -> (String, String) {
    let camera_ids = frames
        .iter()
        .map(|frame| frame.metadata.camera_id.clone())
        .collect::<BTreeSet<_>>();

    if camera_ids.len() != 2 {
        panic!(
            "Expected exactly 2 cameras in recorded footage, found {}.",
            camera_ids.len()
        );
    }

    if let Some(manifest) = manifest {
        if !camera_ids.contains(&manifest.left_camera_id)
            || !camera_ids.contains(&manifest.right_camera_id)
        {
            panic!(
                "Session manifest camera ids ({}, {}) do not match recorded footage cameras {:?}.",
                manifest.left_camera_id, manifest.right_camera_id, camera_ids
            );
        }
        if manifest.left_camera_id == manifest.right_camera_id {
            panic!("Session manifest must map left/right to two different cameras.");
        }
        return (manifest.left_camera_id, manifest.right_camera_id);
    }

    println!("camera_ingest: no {} found", SESSION_MANIFEST_FILE_NAME);
    let mut ids = camera_ids.into_iter().collect::<Vec<_>>();
    ids.sort();
    (ids[0].clone(), ids[1].clone())
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use crossbeam::channel::bounded;

    use super::{load_recorded_session, replay_recorded_session};
    use crate::camera::record::writer::{Metadata, SESSION_MANIFEST_FILE_NAME, SessionManifest};
    use crate::pipeline::{CameraId, Frame as PipelineFrame};

    #[test]
    fn replay_recorded_session_routes_frames_using_session_manifest() {
        let temp_dir = env::temp_dir().join(format!(
            "mjolnir_replay_test_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time should be after epoch")
                .as_nanos()
        ));
        let left_dir = temp_dir.join("camera-b");
        let right_dir = temp_dir.join("camera-a");
        fs::create_dir_all(&left_dir).expect("create left test dir");
        fs::create_dir_all(&right_dir).expect("create right test dir");
        fs::write(
            temp_dir.join(SESSION_MANIFEST_FILE_NAME),
            serde_json::to_vec_pretty(&SessionManifest {
                left_camera_id: "camera-b".to_string(),
                right_camera_id: "camera-a".to_string(),
            })
            .expect("serialize session manifest"),
        )
        .expect("write session manifest");

        write_test_recorded_frame(
            &left_dir,
            &Metadata {
                camera_id: "camera-b".to_string(),
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
                camera_id: "camera-a".to_string(),
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
        replay_recorded_session(temp_dir.clone(), left_tx, right_tx);

        let left_frames: Vec<_> = left_rx.try_iter().collect();
        let right_frames: Vec<_> = right_rx.try_iter().collect();

        assert_eq!(left_frames.len(), 1);
        assert_eq!(left_frames[0].data(), &[1, 2, 3, 4]);
        assert_eq!(left_frames[0].context().camera_id(), CameraId::FieldLeft);
        assert_eq!(left_frames[0].context().timestamp(), 300);
        assert_eq!(right_frames.len(), 1);
        assert_eq!(right_frames[0].data(), &[5, 6, 7, 8]);
        assert_eq!(right_frames[0].context().camera_id(), CameraId::FieldRight);
        assert_eq!(right_frames[0].context().timestamp(), 320);

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn load_recorded_session_sorts_by_current_timestamp_rules_without_renumbering_indices() {
        let temp_dir = env::temp_dir().join(format!(
            "mjolnir_replay_order_test_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time should be after epoch")
                .as_nanos()
        ));
        let left_dir = temp_dir.join("camera-b");
        let right_dir = temp_dir.join("camera-a");
        fs::create_dir_all(&left_dir).expect("create left test dir");
        fs::create_dir_all(&right_dir).expect("create right test dir");
        fs::write(
            temp_dir.join(SESSION_MANIFEST_FILE_NAME),
            serde_json::to_vec_pretty(&SessionManifest {
                left_camera_id: "camera-b".to_string(),
                right_camera_id: "camera-a".to_string(),
            })
            .expect("serialize session manifest"),
        )
        .expect("write session manifest");

        // Intentionally skip frame_index 0 and use mixed timestamp sources to
        // prove we preserve original indices while sorting by the current
        // timestamp-first replay rules.
        write_test_recorded_frame(
            &left_dir,
            &Metadata {
                camera_id: "camera-b".to_string(),
                frame_index: 1,
                width: 4,
                height: 1,
                payload_bytes: 4,
                system_timestamp_ns: 300,
                buffer_timestamp_ns: 200,
                frame_id: 901,
            },
            &[1, 1, 1, 1],
        );
        write_test_recorded_frame(
            &right_dir,
            &Metadata {
                camera_id: "camera-a".to_string(),
                frame_index: 4,
                width: 4,
                height: 1,
                payload_bytes: 4,
                system_timestamp_ns: 500,
                buffer_timestamp_ns: 150,
                frame_id: 902,
            },
            &[2, 2, 2, 2],
        );
        write_test_recorded_frame(
            &left_dir,
            &Metadata {
                camera_id: "camera-b".to_string(),
                frame_index: 9,
                width: 4,
                height: 1,
                payload_bytes: 4,
                system_timestamp_ns: 250,
                buffer_timestamp_ns: 0,
                frame_id: 903,
            },
            &[3, 3, 3, 3],
        );
        write_test_recorded_frame(
            &right_dir,
            &Metadata {
                camera_id: "camera-a".to_string(),
                frame_index: 8,
                width: 4,
                height: 1,
                payload_bytes: 4,
                system_timestamp_ns: 0,
                buffer_timestamp_ns: 0,
                frame_id: 260,
            },
            &[4, 4, 4, 4],
        );

        let session = load_recorded_session(&temp_dir);
        let ordered_indices = session
            .frames
            .iter()
            .map(|frame| (frame.metadata.camera_id.clone(), frame.metadata.frame_index))
            .collect::<Vec<_>>();

        assert_eq!(
            ordered_indices,
            vec![
                ("camera-a".to_string(), 4),
                ("camera-b".to_string(), 1),
                ("camera-b".to_string(), 9),
                ("camera-a".to_string(), 8),
            ]
        );
        assert_eq!(session.frames[0].metadata.buffer_timestamp_ns, 150);
        assert_eq!(session.frames[1].metadata.buffer_timestamp_ns, 200);
        assert_eq!(session.frames[2].metadata.system_timestamp_ns, 250);
        assert_eq!(session.frames[3].metadata.frame_id, 260);

        let _ = fs::remove_dir_all(temp_dir);
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
}
