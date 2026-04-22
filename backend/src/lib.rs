#[cfg(feature = "real")]
pub mod camera;
#[cfg(feature = "real")]
pub mod camera_ingest;
// Technically it doesn't make sense to include circle infractions code in fake lib, but will break if not included. Spin off ticket?
pub mod circle_infractions_ingest;
#[cfg(feature = "real")]
pub mod computer_vision;
#[cfg(feature = "real")]
pub mod pipeline;
#[cfg(feature = "real")]
pub mod schemas;
pub mod server;
pub mod throws;
