// Reads a recorded session directory, applies the same left/right resolver
// the live and replay paths use, and writes the FIRST frame from each camera
// as an 8-bit grayscale PGM (P5) image. The PGM format is supported natively
// by macOS Preview and most image viewers, so you can eyeball whether
// physical-left and logical-left match without touching any new dependencies.
//
// Usage:
//   bazel run //backend:dump_first_frames -- --footage-dir /tmp/camera_out/<ts> \
//       --output-dir /tmp/camera_check
//   open /tmp/camera_check/left_first.pgm /tmp/camera_check/right_first.pgm

use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use backend_lib::camera::camera_assignment::{
    AssignmentInputs, CameraAssignment, resolve_camera_assignment,
};
use backend_lib::camera::record::writer::{
    Frame as RecordedFrame, read_recorded_frame, read_session_manifest,
    SESSION_MANIFEST_FILE_NAME,
};
use clap::Parser;

#[derive(Parser, Debug)]
#[command(about = "Dump the first frame from each camera in a recorded session as PGM images.")]
struct Args {
    /// Directory produced by //backend:record (contains recording_session.json).
    #[arg(long = "footage-dir")]
    footage_dir: PathBuf,

    /// Where to write left_first.pgm and right_first.pgm.
    #[arg(long = "output-dir")]
    output_dir: PathBuf,

    /// Optional override of the FieldLeft camera ID. See //backend:prod_real.
    #[arg(long = "left-camera-id")]
    left_camera_id: Option<String>,

    /// Optional override of the FieldRight camera ID. See //backend:prod_real.
    #[arg(long = "right-camera-id")]
    right_camera_id: Option<String>,
}

fn main() {
    let args = Args::parse();
    fs::create_dir_all(&args.output_dir).unwrap_or_else(|err| {
        panic!(
            "Failed to create output dir {}: {err}",
            args.output_dir.display()
        )
    });

    let frames = load_all_frames(&args.footage_dir);
    if frames.is_empty() {
        panic!(
            "No recorded frame metadata files were found in {}.",
            args.footage_dir.display()
        );
    }

    let camera_ids = frames
        .iter()
        .map(|frame| frame.metadata.camera_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    let manifest = read_session_manifest(&args.footage_dir);
    let CameraAssignment {
        left_camera_id,
        right_camera_id,
    } = resolve_camera_assignment(AssignmentInputs {
        cli_left: args.left_camera_id,
        cli_right: args.right_camera_id,
        manifest,
        available_camera_ids: &camera_ids,
    });

    println!(
        "dump_first_frames: resolved left={} right={}",
        left_camera_id, right_camera_id
    );

    let left_frame = first_frame_for(&frames, &left_camera_id).unwrap_or_else(|| {
        panic!("No frames found for left camera {left_camera_id}");
    });
    let right_frame = first_frame_for(&frames, &right_camera_id).unwrap_or_else(|| {
        panic!("No frames found for right camera {right_camera_id}");
    });

    let left_path = args.output_dir.join("left_first.pgm");
    let right_path = args.output_dir.join("right_first.pgm");
    write_pgm(&left_path, left_frame);
    write_pgm(&right_path, right_frame);

    println!("dump_first_frames: wrote {}", left_path.display());
    println!("dump_first_frames: wrote {}", right_path.display());
    println!(
        "Open both PGM files (e.g. `open {} {}`) and confirm the\n\
         physical left camera's view appears in left_first.pgm.",
        left_path.display(),
        right_path.display()
    );
}

fn load_all_frames(footage_dir: &Path) -> Vec<RecordedFrame> {
    let mut json_paths: Vec<PathBuf> = Vec::new();
    collect_frame_json_paths(footage_dir, &mut json_paths);
    json_paths
        .into_iter()
        .map(|json_path| read_recorded_frame(&json_path))
        .collect()
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

fn first_frame_for<'a>(
    frames: &'a [RecordedFrame],
    camera_id: &str,
) -> Option<&'a RecordedFrame> {
    let mut candidates: Vec<&RecordedFrame> = frames
        .iter()
        .filter(|frame| frame.metadata.camera_id == camera_id)
        .collect();
    candidates.sort_by_key(|frame| frame.metadata.frame_index);
    candidates.into_iter().next()
}

fn write_pgm(path: &Path, frame: &RecordedFrame) {
    let width = frame.metadata.width;
    let height = frame.metadata.height;
    if width <= 0 || height <= 0 {
        panic!(
            "Refusing to write {} with non-positive dimensions {}x{}.",
            path.display(),
            width,
            height
        );
    }
    let expected_pixels = (width as usize) * (height as usize);
    if frame.bytes.len() != expected_pixels {
        panic!(
            "Recorded frame from camera {} has payload {} bytes but {}x{} expects {} bytes (8-bit mono only).",
            frame.metadata.camera_id,
            frame.bytes.len(),
            width,
            height,
            expected_pixels
        );
    }

    let mut file = File::create(path)
        .unwrap_or_else(|err| panic!("Failed to create {}: {err}", path.display()));
    let header = format!("P5\n{width} {height}\n255\n");
    file.write_all(header.as_bytes())
        .unwrap_or_else(|err| panic!("Failed to write PGM header to {}: {err}", path.display()));
    file.write_all(&frame.bytes)
        .unwrap_or_else(|err| panic!("Failed to write PGM payload to {}: {err}", path.display()));
}
