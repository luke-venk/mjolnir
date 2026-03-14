/**
 * Each camera frame will consist of bytes representing the actual
 * image as well as timestamp (Context).
 */

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Context {
    // TODO: replace placeholder
    timestamp: u64,
}

impl Context {
    pub fn new(timestamp: u64) -> Self {
        Self {
            timestamp
        }
    }

    pub fn timestamp(&self) -> u64 {
        self.timestamp
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
    fn test_context_constructor_and_getter() {
        let context = Context::new(6767);

        assert_eq!(context.timestamp(), 6767);
    }

    #[test]
    fn test_frame_constructor_and_getter() {
        let data = vec![1, 2, 3, 4];
        let frame = Frame::new(data, Context::new(34151));
        
        assert_eq!(frame.context().timestamp(), 34151);
    }
}