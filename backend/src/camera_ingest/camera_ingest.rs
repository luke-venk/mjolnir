//defines how cam settigs should be stored and read from env variables
// reads cameras settings from env variables, sends placeholer frames
use std::thread;
use std::time::Duration;
use crate::schemas::{Frame, Context};
use aravis::prelude::*;
use aravis::{AcquisitionMode, Aravis, Buffer, BufferStatus, Camera, ExposureMode};
use crossbeam::channel::Sender; 
use crate::camera_ingest::camera_ingest_helpers::{
    buffer_to_frame, configure_camera, create_stream_and_queue_buffers, initialize_aravis,
    open_camera,};
use crate::schemas::camera_ingest_config::CameraIngestConfig;

//Intializes Aravis once
//static ARAVIS_INIT: Once = Once::new();

// Ingests frames from the cameras using the GigEVision API, and enqueues
// the frames into the camera ingest sender for the camera's pipeline to
// begin processing.
pub fn ingest_frames(tx: Sender<Frame>, config: CameraIngestConfig){
    // TODO(#3): Implement Camera Ingest with Aravis.

    //rough workflow
    //1. Open the camera
    //2. Apply settings from config 
    //3. Allocate and queue buffers
    //4. Convert each buffer to Frame
    //5. Frame to hevc through channel
    //6. 

    
    //test loop
    loop {
        let data = vec![1, 2, 3, 4];
        let context = Context::new(1234);
        println!("Dummy frame");
        if tx.send(Frame::new(data,context)).is_err(){
            break;
        }
        thread::sleep(Duration::from_millis(3000));
    }

    initialize_aravis();
    //Define camera. 
    let camera = open_camera(&config); 
    configure_camera(&camera, &config);
    //Define stream.
    let stream = create_stream_and_queue_buffers(&camera, config.num_buffers); 

    //
    camera
        .start_acquisition()
        .expect("Failed to start it");

    //Pulling buffers from camera stream
    loop {
        let buffer = match stream.timeout_pop_buffer(config.timeout_ms){
            Some(buffer) => buffer, 
            None => continue,
            // Put something for timeout option
        };


        if buffer.status() == BufferStatus::Success{
            let frame = buffer_to_frame(&buffer); 

            if tx.send(frame).is_err(){
                //stopping camera ingest, requeue
                stream.push_buffer(buffer); 
                break; 
            }
        } else {
            eprintln!("Buffer {:?}",buffer.status());
        }
        stream.push_buffer(buffer); 
    }
}