// Allows thread-safe access to app state, so each thread can access
// shared information, like which event is currently being thrown.
// This is necessary since Tokio designed Axum to run across many
// threads.
// docs: https://docs.rs/axum/latest/axum/extract/struct.State.html
use crate::pipeline::CameraId;
use crate::{server::ThrowSource, throws::ThrowType};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::RwLock;

/// In-memory raw grayscale frame plus its `(width, height)`. Held by
/// `AppState` per camera so the pipeline can publish the latest impact
/// frame and the `/api/frames/{camera}` route can PNG-encode it on
/// demand.
pub type ImpactFrame = (Vec<u8>, (u32, u32));

#[derive(Clone)]
pub struct AppState {
    pub throw_type: Arc<RwLock<ThrowType>>,
    pub throw_source: ThrowSource,
    /// Latest impact frame from the left camera. `None` until the
    /// pipeline publishes one. Raw grayscale bytes plus dimensions; the
    /// route PNG-encodes per request.
    pub left_impact_frame: Arc<RwLock<Option<ImpactFrame>>>,
    /// Same as `left_impact_frame` but for the right camera.
    pub right_impact_frame: Arc<RwLock<Option<ImpactFrame>>>,
    infraction_history: Arc<RwLock<VecDeque<u64>>>,
    circle_infraction_system_is_stale: Arc<RwLock<bool>>,
}

impl AppState {
    pub fn new(throw_source: ThrowSource) -> Self {
        Self {
            throw_type: Arc::new(RwLock::new(ThrowType::Shotput)),
            throw_source,
            left_impact_frame: Arc::new(RwLock::new(None)),
            right_impact_frame: Arc::new(RwLock::new(None)),
            infraction_history: Arc::new(RwLock::new(VecDeque::new())),
            circle_infraction_system_is_stale: Arc::new(RwLock::new(false)),
        }
    }

    pub async fn get_infraction_history(&self) -> Vec<u64> {
        self.infraction_history.read().await.clone().into()
    }

    pub async fn record_infraction(&self, timestamp_ns: u64) {
        let mut history = self.infraction_history.write().await;
        let sixty_seconds_ago = timestamp_ns.saturating_sub(60_000_000_000);
        while history.front().map_or(false, |&t| t < sixty_seconds_ago) {
            history.pop_front();
        }
        history.push_back(timestamp_ns);
    }

    pub async fn is_circle_infraction_system_stale(&self) -> bool {
        *self.circle_infraction_system_is_stale.read().await
    }

    pub async fn set_circle_infraction_system_is_stale(&self, is_stale: bool) {
        let mut stale_flag = self.circle_infraction_system_is_stale.write().await;
        *stale_flag = is_stale;
    }

    /// Publishes the impact frame for the given camera. Overwrites any
    /// previous frame so only the latest impact is kept. Called by the
    /// pipeline output handler when a throw completes.
    pub async fn set_impact_frame(&self, camera: CameraId, frame: ImpactFrame) {
        let slot = match camera {
            CameraId::FieldLeft => &self.left_impact_frame,
            CameraId::FieldRight => &self.right_impact_frame,
        };
        *slot.write().await = Some(frame);
    }
}
