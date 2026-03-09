use crate::hardware::{Frame, Context};

pub fn mog2(frame: Frame) -> Frame {
    // TODO: implement actual logic
    
    let data = frame.data();
    let new_metadata = frame.context().metadata() + 1;
    println!("Performing mog2 on frame with metadata: {:?}", new_metadata);  // TODO: remove
    Frame::new(data.to_vec(), Context::new(new_metadata))
}