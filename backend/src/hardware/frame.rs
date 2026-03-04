/**
 * Each camera frame will consist of metadata and the actual image.
 */


#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Frame {
    image_data: Vec<u8>,
    timestamp: u64,
}

#[allow(dead_code)]
impl Frame {
    pub fn new(image_data: Vec<u8>, timestamp: u64) -> Self {
        Self {
            image_data,
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
        let image_data = vec![1, 2, 3, 4];
        let frame = Frame::new(image_data, 34151);
        
        assert_eq!(frame.timestamp, 34151);
    }
}