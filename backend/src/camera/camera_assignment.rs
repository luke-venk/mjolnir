// Single source of truth for which physical camera is "left" (FieldLeft) and
// which is "right" (FieldRight). Used by live capture, the recorder, and the
// replay path so all three modes agree.
//
// Precedence chain (first match wins):
//   1. CLI flags (--left-camera-id / --right-camera-id). If only one is given,
//      the other is inferred as the remaining available camera.
//   2. Recording session manifest (replay only — frozen snapshot from record time).
//   3. Repo-relative config file at <workspace_root>/camera_assignment.json.
//   4. None of the above -> panic with copy-pasteable instructions.

use std::env;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::camera::record::writer::SessionManifest;

pub const REPO_CONFIG_FILE_NAME: &str = "camera_assignment.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CameraAssignment {
    pub left_camera_id: String,
    pub right_camera_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoCameraAssignmentFile {
    pub left_camera_id: String,
    pub right_camera_id: String,
}

/// Inputs the resolver considers. `available_camera_ids` should be the set the
/// caller knows about: discovered cameras for live/record, recorded frame
/// `camera_id`s for replay.
#[derive(Debug, Clone)]
pub struct AssignmentInputs<'a> {
    pub cli_left: Option<String>,
    pub cli_right: Option<String>,
    pub manifest: Option<SessionManifest>,
    pub available_camera_ids: &'a [String],
}

pub fn resolve_camera_assignment(inputs: AssignmentInputs<'_>) -> CameraAssignment {
    if inputs.available_camera_ids.len() != 2 {
        panic!(
            "Expected exactly 2 cameras for left/right assignment, found {}: {:?}",
            inputs.available_camera_ids.len(),
            inputs.available_camera_ids
        );
    }

    if let Some(assignment) = resolve_from_cli(&inputs) {
        validate_assignment(&assignment, inputs.available_camera_ids, "CLI flags");
        return assignment;
    }

    if let Some(manifest) = inputs.manifest.as_ref() {
        let assignment = CameraAssignment {
            left_camera_id: manifest.left_camera_id.clone(),
            right_camera_id: manifest.right_camera_id.clone(),
        };
        validate_assignment(
            &assignment,
            inputs.available_camera_ids,
            "session manifest (recording_session.json)",
        );
        return assignment;
    }

    if let Some(repo_config) = read_repo_config_file() {
        let assignment = CameraAssignment {
            left_camera_id: repo_config.left_camera_id,
            right_camera_id: repo_config.right_camera_id,
        };
        validate_assignment(
            &assignment,
            inputs.available_camera_ids,
            "repo config (camera_assignment.json at workspace root)",
        );
        return assignment;
    }

    panic_with_setup_instructions(inputs.available_camera_ids);
}

fn resolve_from_cli(inputs: &AssignmentInputs<'_>) -> Option<CameraAssignment> {
    match (inputs.cli_left.clone(), inputs.cli_right.clone()) {
        (Some(left), Some(right)) => Some(CameraAssignment {
            left_camera_id: left,
            right_camera_id: right,
        }),
        (Some(left), None) => {
            let right = infer_other_camera(&left, inputs.available_camera_ids);
            Some(CameraAssignment {
                left_camera_id: left,
                right_camera_id: right,
            })
        }
        (None, Some(right)) => {
            let left = infer_other_camera(&right, inputs.available_camera_ids);
            Some(CameraAssignment {
                left_camera_id: left,
                right_camera_id: right,
            })
        }
        (None, None) => None,
    }
}

fn infer_other_camera(known: &str, available: &[String]) -> String {
    let mut candidates = available.iter().filter(|id| id.as_str() != known);
    let inferred = candidates.next().unwrap_or_else(|| {
        panic!(
            "Cannot infer the other camera: --left-camera-id / --right-camera-id was set to {known:?}, but that ID is not in the available set {available:?}."
        )
    });
    if candidates.next().is_some() {
        panic!(
            "Cannot infer the other camera: more than one candidate left after excluding {known:?} from {available:?}."
        );
    }
    inferred.clone()
}

fn validate_assignment(assignment: &CameraAssignment, available: &[String], source: &str) {
    if assignment.left_camera_id == assignment.right_camera_id {
        panic!(
            "Invalid camera assignment from {source}: left and right are both {:?}.",
            assignment.left_camera_id
        );
    }
    if !available.contains(&assignment.left_camera_id) {
        panic!(
            "Invalid camera assignment from {source}: left camera {:?} is not in the available set {:?}.",
            assignment.left_camera_id, available
        );
    }
    if !available.contains(&assignment.right_camera_id) {
        panic!(
            "Invalid camera assignment from {source}: right camera {:?} is not in the available set {:?}.",
            assignment.right_camera_id, available
        );
    }
}

fn read_repo_config_file() -> Option<RepoCameraAssignmentFile> {
    let path = repo_config_path()?;
    let json = fs::read_to_string(&path).ok()?;
    Some(serde_json::from_str(&json).unwrap_or_else(|err| {
        panic!("Failed to parse {}: {err}", path.display());
    }))
}

fn repo_config_path() -> Option<PathBuf> {
    let root = workspace_root()?;
    let candidate = root.join(REPO_CONFIG_FILE_NAME);
    candidate.exists().then_some(candidate)
}

fn workspace_root() -> Option<PathBuf> {
    if let Ok(explicit) = env::var("MJOLNIR_REPO_ROOT") {
        let path = PathBuf::from(explicit);
        if path.is_dir() {
            return Some(path);
        }
    }

    let start = env::current_dir().ok()?;
    let mut current = start.as_path();
    loop {
        if current.join("MODULE.bazel").exists() {
            return Some(current.to_path_buf());
        }
        match current.parent() {
            Some(parent) => current = parent,
            None => return None,
        }
    }
}

fn panic_with_setup_instructions(available: &[String]) -> ! {
    let example = serde_json::to_string_pretty(&RepoCameraAssignmentFile {
        left_camera_id: available
            .first()
            .cloned()
            .unwrap_or_else(|| "<camera-id-1>".to_string()),
        right_camera_id: available
            .get(1)
            .cloned()
            .unwrap_or_else(|| "<camera-id-2>".to_string()),
    })
    .expect("failed to serialize example assignment");

    let workspace_hint = workspace_root()
        .map(|root| root.join(REPO_CONFIG_FILE_NAME).display().to_string())
        .unwrap_or_else(|| {
            format!("<workspace-root>/{REPO_CONFIG_FILE_NAME} (set MJOLNIR_REPO_ROOT if auto-detection fails)")
        });

    panic!(
        "\nNo camera assignment found. Available cameras: {available:?}.\n\n\
         To fix, choose ONE of:\n\
         \n\
         1. Pass CLI flags:\n   \
                --left-camera-id <ID> --right-camera-id <ID>\n   \
                (or pass just one; the other is inferred since there are exactly 2 cameras)\n\
         \n\
         2. Create the repo config file at:\n   \
                {workspace_hint}\n   \
                with contents:\n\n{example}\n"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(left: &str, right: &str) -> Vec<String> {
        vec![left.to_string(), right.to_string()]
    }

    #[test]
    fn cli_both_flags_take_precedence_over_manifest() {
        let available = ids("camera-a", "camera-b");
        let assignment = resolve_camera_assignment(AssignmentInputs {
            cli_left: Some("camera-b".to_string()),
            cli_right: Some("camera-a".to_string()),
            manifest: Some(SessionManifest {
                left_camera_id: "camera-a".to_string(),
                right_camera_id: "camera-b".to_string(),
            }),
            available_camera_ids: &available,
        });
        assert_eq!(assignment.left_camera_id, "camera-b");
        assert_eq!(assignment.right_camera_id, "camera-a");
    }

    #[test]
    fn cli_left_only_infers_right_from_available_pool() {
        let available = ids("camera-a", "camera-b");
        let assignment = resolve_camera_assignment(AssignmentInputs {
            cli_left: Some("camera-b".to_string()),
            cli_right: None,
            manifest: None,
            available_camera_ids: &available,
        });
        assert_eq!(assignment.left_camera_id, "camera-b");
        assert_eq!(assignment.right_camera_id, "camera-a");
    }

    #[test]
    fn cli_right_only_infers_left_from_available_pool() {
        let available = ids("camera-a", "camera-b");
        let assignment = resolve_camera_assignment(AssignmentInputs {
            cli_left: None,
            cli_right: Some("camera-a".to_string()),
            manifest: None,
            available_camera_ids: &available,
        });
        assert_eq!(assignment.left_camera_id, "camera-b");
        assert_eq!(assignment.right_camera_id, "camera-a");
    }

    #[test]
    fn manifest_used_when_no_cli_flags() {
        let available = ids("camera-a", "camera-b");
        let assignment = resolve_camera_assignment(AssignmentInputs {
            cli_left: None,
            cli_right: None,
            manifest: Some(SessionManifest {
                left_camera_id: "camera-b".to_string(),
                right_camera_id: "camera-a".to_string(),
            }),
            available_camera_ids: &available,
        });
        assert_eq!(assignment.left_camera_id, "camera-b");
        assert_eq!(assignment.right_camera_id, "camera-a");
    }

    #[test]
    #[should_panic(expected = "left and right are both")]
    fn cli_with_equal_left_and_right_panics() {
        let available = ids("camera-a", "camera-b");
        resolve_camera_assignment(AssignmentInputs {
            cli_left: Some("camera-a".to_string()),
            cli_right: Some("camera-a".to_string()),
            manifest: None,
            available_camera_ids: &available,
        });
    }

    #[test]
    #[should_panic(expected = "is not in the available set")]
    fn cli_with_unknown_camera_panics() {
        let available = ids("camera-a", "camera-b");
        resolve_camera_assignment(AssignmentInputs {
            cli_left: Some("camera-z".to_string()),
            cli_right: Some("camera-a".to_string()),
            manifest: None,
            available_camera_ids: &available,
        });
    }

    #[test]
    #[should_panic(expected = "Cannot infer the other camera")]
    fn cli_single_flag_with_unknown_camera_panics() {
        let available = ids("camera-a", "camera-b");
        resolve_camera_assignment(AssignmentInputs {
            cli_left: Some("camera-z".to_string()),
            cli_right: None,
            manifest: None,
            available_camera_ids: &available,
        });
    }

    #[test]
    #[should_panic(expected = "Expected exactly 2 cameras")]
    fn fewer_than_two_available_cameras_panics() {
        let available = vec!["camera-a".to_string()];
        resolve_camera_assignment(AssignmentInputs {
            cli_left: None,
            cli_right: None,
            manifest: None,
            available_camera_ids: &available,
        });
    }

    #[test]
    #[should_panic(expected = "session manifest")]
    fn manifest_referencing_unknown_camera_panics() {
        let available = ids("camera-a", "camera-b");
        resolve_camera_assignment(AssignmentInputs {
            cli_left: None,
            cli_right: None,
            manifest: Some(SessionManifest {
                left_camera_id: "camera-z".to_string(),
                right_camera_id: "camera-a".to_string(),
            }),
            available_camera_ids: &available,
        });
    }
}
