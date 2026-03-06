// Allows thread-safe access to app state, so each thread can access
// shared information, like which event is currently being thrown.
// This is necessary since Tokio designed Axum to run across many
// threads.
// docs: https://docs.rs/axum/latest/axum/extract/struct.State.html
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::sports::EventType;

#[derive(Clone)]
pub struct AppState {
    pub event_type: Arc<RwLock<EventType>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            event_type: Arc::new(RwLock::new(EventType::Shotput)),
        }
    }
}