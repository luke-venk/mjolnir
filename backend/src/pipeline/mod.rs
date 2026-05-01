pub mod frame;
pub mod pipeline;
pub mod pipeline_stage;
#[cfg(test)]
pub mod test_utils;

pub use frame::{CameraId, Context, Frame};
pub use pipeline::{start_recorded_footage_pipelines, Pipeline};
pub use pipeline_stage::PipelineStage;

/// Bounded crossbeam channels between cams and pipeline, pipeline stages, have a capacity of 10
/// They block on tx when that limit is reached
/// This prevents us from blowing up memory if pipeline is too slow
pub const CAPACITY_PER_CROSSBEAM_CHANNEL: usize = 10;
