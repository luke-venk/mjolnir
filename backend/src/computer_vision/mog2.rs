use crate::pipeline::{Frame, PipelineStageOptions};
use opencv::core::{Mat, Point, Scalar, Size, BORDER_CONSTANT};
use opencv::imgproc::{self, MORPH_CLOSE, MORPH_ELLIPSE, MORPH_OPEN, THRESH_BINARY};
use opencv::prelude::*;
use opencv::video;
use std::cell::RefCell;

pub const MOG2_HISTORY_FRAMES: usize = 300;
const MOG2_VAR_THRESHOLD: f64 = 60.0;
const MOG2_DETECT_SHADOWS: bool = false;
const MORPH_OPEN_KERNEL_SIZE: i32 = 3;
const MORPH_CLOSE_KERNEL_SIZE: i32 = 40;
// MOG2 marks shadows at 127; threshold above 200 keeps only true foreground (255).
const FOREGROUND_THRESHOLD: f64 = 200.0;

struct Mog2Processor {
    bg_sub: opencv::core::Ptr<dyn video::BackgroundSubtractorMOG2>,
    kernel_open: Mat,
    kernel_close: Mat,
}

impl Mog2Processor {
    fn new() -> Self {
        let bg_sub = video::create_background_subtractor_mog2(
            MOG2_HISTORY_FRAMES as i32,
            MOG2_VAR_THRESHOLD,
            MOG2_DETECT_SHADOWS,
        )
        .expect("failed to create MOG2 background subtractor");

        let kernel_open = imgproc::get_structuring_element(
            MORPH_ELLIPSE,
            Size::new(MORPH_OPEN_KERNEL_SIZE, MORPH_OPEN_KERNEL_SIZE),
            Point::new(-1, -1),
        )
        .expect("failed to create morphological open kernel");

        let kernel_close = imgproc::get_structuring_element(
            MORPH_ELLIPSE,
            Size::new(MORPH_CLOSE_KERNEL_SIZE, MORPH_CLOSE_KERNEL_SIZE),
            Point::new(-1, -1),
        )
        .expect("failed to create morphological close kernel");

        Self {
            bg_sub,
            kernel_open,
            kernel_close,
        }
    }

    fn process_frame(&mut self, frame: Frame, options: PipelineStageOptions) -> Frame {
        let input_image = if options.rerun_4k_mode {
            frame.undistorted_image()
        } else {
            frame.downsampled_image()
        };

        let Some(gray) = input_image else {
            return frame;
        };

        // Background subtraction: learning_rate=-1 uses the MOG2 default schedule.
        let mut fg_mask = Mat::default();
        if self.bg_sub.apply(&gray, &mut fg_mask, -1.0).is_err() {
            return frame;
        }

        // Threshold to binary: eliminates shadow pixels (marked 127 by MOG2).
        let mut fg_binary = Mat::default();
        if imgproc::threshold(
            &fg_mask,
            &mut fg_binary,
            FOREGROUND_THRESHOLD,
            255.0,
            THRESH_BINARY,
        )
        .is_err()
        {
            return frame;
        }

        // Morphological open: removes small noise blobs.
        let mut mask_opened = Mat::default();
        if imgproc::morphology_ex(
            &fg_binary,
            &mut mask_opened,
            MORPH_OPEN,
            &self.kernel_open,
            Point::new(-1, -1),
            1,
            BORDER_CONSTANT,
            Scalar::default(),
        )
        .is_err()
        {
            return frame;
        }

        // Morphological close: fills holes inside the implement blob.
        let mut mask_clean = Mat::default();
        if imgproc::morphology_ex(
            &mask_opened,
            &mut mask_clean,
            MORPH_CLOSE,
            &self.kernel_close,
            Point::new(-1, -1),
            1,
            BORDER_CONSTANT,
            Scalar::default(),
        )
        .is_err()
        {
            return frame;
        }

        // Store the cleaned binary mask as a dedicated MOG2 stage output.
        frame
            .clear_mog2_image()
            .expect("failed to clear MOG2 image before writing MOG2 output");
        frame
            .set_mog2_image(mask_clean)
            .expect("failed to set cleaned mask as MOG2 image");
        frame
    }
}

thread_local! {
    static MOG2_PROCESSOR: RefCell<Mog2Processor> = RefCell::new(Mog2Processor::new());
}

pub fn mog2(frame: Frame) -> Frame {
    mog2_with_options(frame, PipelineStageOptions::default())
}

pub fn mog2_with_options(frame: Frame, options: PipelineStageOptions) -> Frame {
    MOG2_PROCESSOR.with(|processor| processor.borrow_mut().process_frame(frame, options))
}
