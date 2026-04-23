/**
 * All logic specific to the type of throwing event can live here. The 4
 * types of throwing events are shot put, discus throw, hammer throw, and
 * javelin throw.
 */
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, Serialize)]
pub enum ThrowType {
    Shotput,
    Discus,
    Hammer,
    Javelin,
}

// Request and response bodies for specifying the type of throwing event.
// Used by Axum.
#[derive(Deserialize)]
pub struct PostThrowTypeRequest {
    // Allow camelCase in frontend and snake_case in backend.
    #[serde(alias = "throwType")]
    pub throw_type: String,
}

#[derive(Serialize)]
pub struct GetThrowTypeResponse {
    pub throw_type: ThrowType,
}
