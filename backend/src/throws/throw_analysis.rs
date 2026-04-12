use super::{Infraction, ThrowType};
use serde::Serialize;
use uuid::Uuid;

// Refer to the `throwEventSchema` in frontend/lib/schemas.ts.
// Allow camelCase in frontend and snake_case in backend.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThrowAnalysisResponse {
    pub throw_id: Uuid,

    // The buffer camera timestamp, in microseconds, at the time the last
    // frame of impact was captured.
    pub frame_timestamp_from_camera_microseconds: String,

    // Whether the throw is shot put, discus, hammer, or javelin.
    pub throw_type: ThrowType,

    // The distance from where the implement landed to the inside edge of 
    // the stop board, in meters.
    pub distance_m: f32,

    // Can possibly have circle infractions, sector violations, both,
    // or no infractions.
    pub infractions: Vec<Infraction>,

    // The URLs associated with the images of the object's landing point.
    pub images: Vec<String>,

    // The landing point in X/Y coordinates. This is None if there is
    // a sector violation, in which case there is no valid landing point.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub landing_point_x_y: Option<(f32, f32)>,
}
