use backend_lib::schemas::CameraId;
use backend_lib::pipeline::Pipeline;
use backend_lib::server::{create_app, start_server};

// Start tokio async runtime.
#[tokio::main]
async fn main() {
    // Start the 2 pipelines (one for each camera).
    let rolling_buffer_size: usize = 10;
    let _ = Pipeline::new(CameraId::FieldLeft, rolling_buffer_size);
    let _ = Pipeline::new(CameraId::FieldRight, rolling_buffer_size);

    // TODO(#7): Implement Clean Shutdown.

    // Build the Axum router.
    let app = create_app();

    // Start the Axum server.
    start_server(app, "0.0.0.0:5001").await;
}
