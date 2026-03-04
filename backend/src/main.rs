mod hardware;
mod pipeline;
mod server;
mod sports;

use crate::hardware::CameraId;
use crate::pipeline::Pipeline;
use crate::server::{create_app, start_server};


// Start tokio async runtime.
#[tokio::main]
async fn main() {
    // Start the 2 pipelines (one for each camera) on their own threads.
    let rolling_buffer_size: usize = 10;
    let mut pipeline_left = Pipeline::new(CameraId::FieldLeft, rolling_buffer_size);
    let mut pipeline_right = Pipeline::new(CameraId::FieldRight, rolling_buffer_size);

    pipeline_left.start();
    pipeline_right.start();

    // Build the Axum router.
    let app = create_app();

    // Build the shutdown signal: TODO
    // Once the server is stopped, also stop the pipelines gracefully and block
    // this thread until they come down.
    // pipeline_left.stop();
    // pipeline_right.stop();

    // Start the Axum server.
    start_server(app, "0.0.0.0:3000").await;
}
