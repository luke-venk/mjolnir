use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::throws::infractions::Infraction;
use crate::throws::throw_type::ThrowType;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThrowAnalysisResponse {
    pub throw_id: Uuid,
    pub frame_timestamp_from_camera_microseconds: String,
    pub throw_start_timestamp_from_camera_microseconds: Option<String>,
    pub throw_end_timestamp_from_camera_microseconds: Option<String>,
    pub throw_type: ThrowType,
    pub distance_m: f32,
    pub infractions: Vec<Infraction>,
    pub images: Vec<String>,
    pub landing_point_x_y: Option<(f32, f32)>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PostThrowTypeRequest {
    pub throw_type: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetThrowTypeResponse {
    pub throw_type: ThrowType,
}
