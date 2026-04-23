// Each camera frame will consist of bytes representing the actual
// image as well as timestamp (Context).
use crate::camera::Resolution;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Context {
    timestamp: u64,
    resolution: Resolution,
}

impl Context {
    pub fn new(timestamp: u64, resolution: Resolution) -> Self {
        Self { 
            timestamp,
            resolution,
        }
    }

    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }

    pub fn resolution(&self) -> Resolution {
        self.resolution
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Frame {
    data: Vec<u8>,
    context: Context,
}

#[allow(dead_code)]
impl Frame {
    pub fn new(data: Vec<u8>, context: Context) -> Self {
        Self { data, context }
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn context(&self) -> &Context {
        &self.context
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_constructor_and_getter() {
        let context = Context::new(6767, crate::camera::Resolution::FullHD);

        assert_eq!(context.timestamp(), 6767);
        assert_eq!(context.resolution(), crate::camera::Resolution::FullHD);
    }

    #[test]
    fn test_frame_constructor_and_getter() {
        let data = vec![1, 2, 3, 4];
        let frame = Frame::new(data, Context::new(34151, crate::camera::Resolution::FullHD));

        assert_eq!(frame.context().timestamp(), 34151);
        assert_eq!(frame.context().resolution(), crate::camera::Resolution::FullHD);
    }
}
