// Allows thread-safe access to app state, so each thread can access
// shared information, like which event is currently being thrown.
// This is necessary since Tokio designed Axum to run across many
// threads.
// docs: https://docs.rs/axum/latest/axum/extract/struct.State.html
use std::sync::Arc;
use std::collections::VecDeque;
use tokio::sync::RwLock;
use crate::schemas::ThrowType;

#[derive(Clone)]
pub struct AppState {
    pub throw_type: Arc<RwLock<ThrowType>>,
    infraction_history: Arc<RwLock<VecDeque<u64>>>,
    circle_infraction_system_is_stale: Arc<RwLock<bool>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            throw_type: Arc::new(RwLock::new(ThrowType::Shotput)),
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
