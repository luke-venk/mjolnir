// Allows thread-safe access to app state, so each thread can access
// shared information, like which event is currently being thrown.
// This is necessary since Tokio designed Axum to run across many
// threads.
// docs: https://docs.rs/axum/latest/axum/extract/struct.State.html
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::{server::ThrowSource, throws::ThrowType};

#[derive(Clone)]
pub struct AppState {
    pub throw_type: Arc<RwLock<ThrowType>>,
    pub throw_source: ThrowSource,
}

impl AppState {
    pub fn new(throw_source: ThrowSource) -> Self {
        Self {
            throw_type: Arc::new(RwLock::new(ThrowType::Shotput)),
            throw_source
        }
    }
}
