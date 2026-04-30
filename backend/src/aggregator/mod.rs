//! Post-pipeline aggregation utilities that pair left/right frames, collect
//! matched observations over an active throw window, and build
//! trajectory-optimization inputs for downstream math routines.

pub mod aggregation_coordinator;
pub mod matched_frame_pair_aggregator;
pub mod trajectory_input_collector;

pub use aggregation_coordinator::{AggregationCommand, AggregationCoordinator};
pub use matched_frame_pair_aggregator::MatchedFramePairAggregator;
pub use trajectory_input_collector::{OptimizeTrajectoryInput, TrajectoryInputCollector};
