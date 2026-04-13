use serde::Serialize;

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InfractionType {
    LeftSector,
    RightSector,
    Circle,
}

#[derive(Debug, Clone, Serialize)]
pub struct Infraction {
    #[serde(rename = "type")]
    pub infraction_type: InfractionType,
    pub confidence: f32,
}
