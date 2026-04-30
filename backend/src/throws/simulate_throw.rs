use crate::throws::infractions::{Infraction, InfractionType};
use crate::throws::throw_analysis::ThrowAnalysisResponse;
use crate::throws::throw_type::ThrowType;

use chrono::Utc;
use std::f32::consts::PI;
use uuid::Uuid;

pub fn get_field_dimensions(throw_type: ThrowType) -> (f32, f32, f32) {
    match throw_type {
        ThrowType::Shotput => (2.135, 30.0, 34.92),
        ThrowType::Discus => (2.50, 80.0, 34.92),
        ThrowType::Hammer => (2.135, 90.0, 34.92),
        ThrowType::Javelin => (16.0, 100.0, 28.96),
    }
}

fn get_random_infractions() -> Vec<Infraction> {
    let p = 0.3;
    let mut infractions: Vec<Infraction> = vec![];

    if rand::random::<f32>() < p {
        infractions.push(Infraction {
            infraction_type: InfractionType::LeftSector,
            confidence: rand::random::<f32>(),
        });
    } else if rand::random::<f32>() < p {
        infractions.push(Infraction {
            infraction_type: InfractionType::RightSector,
            confidence: rand::random::<f32>(),
        });
    }

    // Simulated Circle may appear here, but server.rs should remove it and override using real circle system.
    if rand::random::<f32>() < p {
        infractions.push(Infraction {
            infraction_type: InfractionType::Circle,
            confidence: rand::random::<f32>(),
        });
    }

    infractions
}

pub fn simulate_throw_event(throw_type: ThrowType) -> ThrowAnalysisResponse {
    let (circle_diameter, field_length, sector_angle) = get_field_dimensions(throw_type);

    let rand_distance_base: f32 = rand::random::<f32>() * field_length;
    let rand_distance: f32 = circle_diameter / 2.0 + rand_distance_base;

    let norm_theta: f32 = sector_angle / 2.0;
    let rand_pos_theta: f32 = rand::random::<f32>() * norm_theta;

    let random_x: f32 = rand_distance * ((rand_pos_theta * PI) / 180.0).cos();
    let random_y_multiplier: f32 = if rand::random::<f32>() < 0.5 { 1.0 } else { -1.0 };
    let random_y: f32 = rand_distance * ((rand_pos_theta * PI) / 180.0).sin() * random_y_multiplier;

    let infractions: Vec<Infraction> = if rand::random::<f32>() < 0.4 {
        get_random_infractions()
    } else {
        Vec::new()
    };

    let has_sector_violation = infractions.iter().any(|inf| {
        matches!(inf.infraction_type, InfractionType::LeftSector | InfractionType::RightSector)
    });

    let landing_point_x_y: Option<(f32, f32)> = if !has_sector_violation {
        Some((random_x, random_y))
    } else {
        None
    };

    // Microseconds since Unix epoch
    let end_us: i64 = Utc::now().timestamp_micros();
    // Simulate ~1.5 seconds of "throw window"
    let start_us: i64 = end_us - 1_500_000;

    ThrowAnalysisResponse {
        throw_id: Uuid::new_v4(),
        frame_timestamp_from_camera_microseconds: end_us.to_string(),
        throw_start_timestamp_from_camera_microseconds: Some(start_us.to_string()),
        throw_end_timestamp_from_camera_microseconds: Some(end_us.to_string()),
        throw_type,
        distance_m: rand_distance,
        infractions,
        images: vec![
            "https://placeholdpicsum.dev/photo/id/729/400".to_string(),
            "https://placeholdpicsum.dev/photo/id/929/600/400".to_string(),
            "https://placeholdpicsum.dev/photo/id/925/600/400".to_string(),
        ],
        landing_point_x_y,
    }
}
