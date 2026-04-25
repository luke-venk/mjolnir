// Each camera frame will consist of bytes representing the actual
// image as well as timestamp (Context).
use crate::camera::Resolution;
use opencv::core::Mat;
use opencv::prelude::MatTraitConstManual;

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

#[derive(Debug, Clone, Default)]
pub struct Frame {
    data: Mat,
    context: Context,
}

#[allow(dead_code)]
impl Frame {
    pub fn new(data: Mat, context: Context) -> Self {
        Self { data, context }
    }

    pub fn data(&self) -> &Mat {
        &self.data
    }

    pub fn data_as_arr(&self) -> &[u8] {
        self.data
            .data_bytes()
            .expect("Error: Failed to convert outupt Mat to array.")
    }

    pub fn context(&self) -> &Context {
        &self.context
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::test_utils::generate_frame;

    #[test]
    fn test_context_constructor_and_getter() {
        let context = Context::new(6767, crate::camera::Resolution::FullHD);

        assert_eq!(context.timestamp(), 6767);
        assert_eq!(context.resolution(), crate::camera::Resolution::FullHD);
    }

    #[test]
    fn test_frame_constructor_and_getter() {
        let frame = generate_frame(69, 34151, crate::camera::Resolution::HD);

        assert_eq!(frame.context().timestamp(), 34151);
        assert_eq!(
            frame.context().resolution(),
            crate::camera::Resolution::HD
        );
    }
}
