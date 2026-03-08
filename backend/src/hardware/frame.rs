/**
 * Each camera frame will consist of bytes representing the actual
 * image as well as metadata (Context).
 */

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Context {
    // TODO: replace placeholder
    metadata: u64,
}

impl Context {
    pub fn new(metadata: u64) -> Self {
        Self {
            metadata
        }
    }

    pub fn metadata(&self) -> u64 {
        self.metadata
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Frame {
    data: Vec<u8>,
    context: Context,
}

#[allow(dead_code)]
impl Frame {
    pub fn new(data: Vec<u8>, context: Context) -> Self {
        Self {
            data,
            context,
        }
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
    fn test_frame_constructor_and_getter() {
        let data = vec![1, 2, 3, 4];
        let frame = Frame::new(data, Context::new(34151));
        
        assert_eq!(frame.context().metadata(), 34151);
    }
}