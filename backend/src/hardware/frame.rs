/**
 * Each camera frame will consist of metadata and the actual image.
 */


#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Frame {
    // TODO: add image data
    timestamp: u64,
}

#[allow(dead_code)]
impl Frame {
    pub fn new(timestamp: u64) -> Self {
        Self {
            timestamp,
        }
    }

    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_constructor_and_getter() {
        let frame = Frame::new(34151);
        
        assert_eq!(frame.timestamp, 34151);
    }
}