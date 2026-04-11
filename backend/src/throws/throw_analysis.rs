use super::{Infraction, ThrowType};
use serde::Serialize;
use uuid::Uuid;

// Refer to the `throwEventSchema` in frontend/lib/schemas.ts.
// Allow camelCase in frontend and snake_case in backend.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThrowAnalysisResponse {
    pub throw_id: Uuid,
    pub timestamp: String,
    pub throw_type: ThrowType,
    pub distance_m: f32,
    pub infractions: Vec<Infraction>,
    pub images: Vec<String>,

    // Note: Currently frontend doesn't return a landing point if there is
    // an infraction, but shouldn't it still return a landing point if
    // there is a circle infraction and not a sector violation?
    #[serde(skip_serializing_if = "Option::is_none")]
    pub landing_point_x_y: Option<(f32, f32)>,
}
