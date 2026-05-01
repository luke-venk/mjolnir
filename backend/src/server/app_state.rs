// Allows thread-safe access to app state, so each thread can access
// shared information, like which event is currently being thrown.
// This is necessary since Tokio designed Axum to run across many
// threads.
// docs: https://docs.rs/axum/latest/axum/extract/struct.State.html
use crate::{server::ThrowSource, throws::ThrowType};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct AppState {
    pub throw_type: Arc<RwLock<ThrowType>>,
    pub throw_source: ThrowSource,
    /// Root directory used by the `/api/frames/{*path}` route to serve
    /// recorded TIFF frames as PNG. `None` when running in simulated mode
    /// (no real frames on disk to serve).
    pub frames_dir: Option<PathBuf>,
    infraction_history: Arc<RwLock<VecDeque<u64>>>,
    circle_infraction_system_is_stale: Arc<RwLock<bool>>,
}

impl AppState {
    pub fn new(throw_source: ThrowSource, frames_dir: Option<PathBuf>) -> Self {
        Self {
            throw_type: Arc::new(RwLock::new(ThrowType::Shotput)),
            throw_source,
            frames_dir,
            infraction_history: Arc::new(RwLock::new(VecDeque::new())),
            circle_infraction_system_is_stale: Arc::new(RwLock::new(false)),
        }
    }

    pub async fn get_infraction_history(&self) -> Vec<u64> {
        self.infraction_history.read().await.clone().into()
    }

    pub async fn record_infraction(&self, timestamp_ms: u64) {
        let mut history = self.infraction_history.write().await;
        let sixty_seconds_ago = timestamp_ms.saturating_sub(60_000);
        while history.front().map_or(false, |&t| t < sixty_seconds_ago) {
            history.pop_front();
        }
        history.push_back(timestamp_ms);
    }

    pub async fn is_circle_infraction_system_stale(&self) -> bool {
        *self.circle_infraction_system_is_stale.read().await
    }

    pub async fn set_circle_infraction_system_is_stale(&self, is_stale: bool) {
        let mut stale_flag = self.circle_infraction_system_is_stale.write().await;
        *stale_flag = is_stale;
    }
}
