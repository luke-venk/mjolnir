use chrono::Utc;
use rand::random;
use serde::Serialize;
use std::f32::consts::PI;
use uuid::Uuid;

use super::{Infraction, InfractionType, ThrowType};

// Refer to the `throwEventSchema` in frontend/lib/schemas.ts.
// Allow camelCase in frontend and snake_case in backend.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetSimulatedThrowResponse {
    pub throw_id: Uuid,
    pub timestamp: String,
    pub throw_type: ThrowType,
    pub distance: f32,
    pub infractions: Vec<Infraction>,
    pub images: Vec<String>,

    // Note: Currently frontend doesn't return a landing point if there is
    // an infraction, but shouldn't it still return a landing point if
    // there is a circle infraction and not a sector violation?
    #[serde(skip_serializing_if = "Option::is_none")]
    pub landing_point: Option<(f32, f32)>,
}

pub fn get_circle_field_dims(throw_type: ThrowType) -> (f32, f32) {
    match throw_type {
        ThrowType::Shotput => (2.135, 30.0),
        ThrowType::Discus => (2.5, 80.0),
        ThrowType::Hammer => (2.135, 90.0),
        ThrowType::Javelin => (16.0, 100.0),
    }
}

pub fn get_random_infractions() -> Vec<Infraction> {
    let mut infractions: Vec<Infraction> = vec![];

    if rand::random::<f32>() < 0.3 {
        infractions.push(Infraction {
            infraction_type: InfractionType::LeftSector,
            confidence: rand::random::<f32>(),
        });
    } else if rand::random::<f32>() < 0.3 {
        infractions.push(Infraction {
            infraction_type: InfractionType::RightSector,
            confidence: rand::random::<f32>(),
        });
    }

    if rand::random::<f32>() < 0.3 {
        infractions.push(Infraction {
            infraction_type: InfractionType::Circle,
            confidence: rand::random::<f32>(),
        });
    }

    infractions
}

pub fn simulate_throw_event(throw_type: ThrowType) -> GetSimulatedThrowResponse {
    let (circle_diameter, field_length): (f32, f32) = get_circle_field_dims(throw_type);

    // Note: Does the math look okay? Sometimes when I'm running sims, the object
    // will appear to land in the throwing area or out of bounds and not be an
    // infraction.
    let rand_distance_base: f32 = rand::random::<f32>() * field_length;
    let rand_distance: f32 = circle_diameter / 2.0 + rand_distance_base;

    let max_theta: f32 = match throw_type {
        ThrowType::Javelin => 28.96,
        _ => 34.92,
    };
    let norm_theta: f32 = max_theta / 2.0;
    let rand_pos_theta: f32 = rand::random::<f32>() * norm_theta;

    let random_x: f32 = rand_distance * ((rand_pos_theta * PI) / 180.0).cos();
    let random_y_multiplier: f32 = if rand::random::<f32>() < 0.5 {
        1.0
    } else {
        -1.0
    };
    let random_y = rand_distance * ((rand_pos_theta * PI) / 180.0).sin() * random_y_multiplier;

    // Cumulative probability of 26.5% chance of infraction.
    let infractions: Vec<Infraction> = if rand::random::<f32>() < 0.4 {
        get_random_infractions()
    } else {
        Vec::new()
    };

    let landing_point: Option<(f32, f32)> = if infractions.is_empty() {
        Some((random_x, random_y))
    } else {
        None
    };

    return GetSimulatedThrowResponse {
        throw_id: Uuid::new_v4(),
        timestamp: Utc::now().to_rfc3339(),
        throw_type,
        distance: rand_distance,
        infractions,
        images: vec![
            "https://placeholdpicsum.dev/photo/id/729/400".to_string(),
            "https://placeholdpicsum.dev/photo/id/929/600/400".to_string(),
            "https://placeholdpicsum.dev/photo/id/925/600/400".to_string(),
        ],
        landing_point,
    };
}
