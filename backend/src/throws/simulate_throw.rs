use super::{InfractionType, ThrowAnalysisResponse, ThrowType};
use chrono::Utc;
use std::f32::consts::PI;
use uuid::Uuid;

/// Based on what sport is being evaluated, return the field dimensions.
///
/// The first number is the diameter of the throwing circle in meters, or
/// for javelin it's the length of the runway.
///
/// The second number is the length of the field in meters.
///
/// The third number is the sector angle, in degrees.
///
/// For reference, see the following link:
/// https://www.cits.wa.gov.au/sport-and-recreation/sports-dimensions-guide/athletics-throwing-events
pub fn get_field_dimensions(throw_type: ThrowType) -> (f32, f32, f32) {
    match throw_type {
        ThrowType::Shotput => (2.135, 30.0, 34.92),
        ThrowType::Discus => (2.50, 80.0, 34.92),
        ThrowType::Hammer => (2.135, 90.0, 34.92),
        ThrowType::Javelin => (16.0, 100.0, 28.96),
    }
}

pub fn get_random_infractions() -> Vec<InfractionType> {
    let infraction_probability = 0.3;

    let mut infractions: Vec<InfractionType> = vec![];

    if rand::random::<f32>() < infraction_probability {
        infractions.push(InfractionType::LeftSector);
    } else if rand::random::<f32>() < infraction_probability {
        infractions.push(InfractionType::RightSector);
    }

    infractions
}

pub fn simulate_throw_event(throw_type: ThrowType) -> ThrowAnalysisResponse {
    let (circle_diameter, field_length, sector_angle): (f32, f32, f32) =
        get_field_dimensions(throw_type);

    let rand_distance_base: f32 = rand::random::<f32>() * field_length;
    let rand_distance: f32 = circle_diameter / 2.0 + rand_distance_base;

    let norm_theta: f32 = sector_angle / 2.0;
    let rand_pos_theta: f32 = rand::random::<f32>() * norm_theta;

    let random_x: f32 = rand_distance * ((rand_pos_theta * PI) / 180.0).cos();
    let random_y_multiplier: f32 = if rand::random::<f32>() < 0.5 {
        1.0
    } else {
        -1.0
    };
    let random_y = rand_distance * ((rand_pos_theta * PI) / 180.0).sin() * random_y_multiplier;

    // Cumulative probability of 26.5% chance of infraction.
    let infractions: Vec<InfractionType> = if rand::random::<f32>() < 0.4 {
        get_random_infractions()
    } else {
        Vec::new()
    };

    // Show landing point as long as there is no sector violation. It is still
    // useful to show the landing point even if there is a circle infraction.
    let has_sector_violation = infractions.iter().any(|infraction| {
        matches!(
            infraction,
            InfractionType::LeftSector | InfractionType::RightSector
        )
    });
    let landing_point_x_y: Option<(f32, f32)> = if !has_sector_violation {
        Some((random_x, random_y))
    } else {
        None
    };

    return ThrowAnalysisResponse {
        throw_id: Uuid::new_v4(),
        frame_timestamp_from_camera_microseconds: Utc::now().to_rfc3339(),
        throw_type,
        distance_m: rand_distance,
        infractions,
        images: vec![
            "https://placeholdpicsum.dev/photo/id/929/600/400".to_string(),
            "https://placeholdpicsum.dev/photo/id/925/600/400".to_string(),
        ],
        landing_point_x_y,
    };
}
