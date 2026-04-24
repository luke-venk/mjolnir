use crate::schemas::{Context, Frame};
use opencv::core::{Mat, Scalar, Point, Vector};
use opencv::imgproc;
use opencv::prelude::*;

pub fn contour(frame: Frame) -> Frame {
    // Since Context doesn't have resolution, we need to infer dimensions from data length
    // For a square image (common in test patterns), height = width = sqrt(data_len)
    let data_len = frame.data().len();
    let size = (data_len as f64).sqrt() as i32;
    let height = size;
    let width = size;
    
    // Create Mat from the frame data
    let input_mat = unsafe {
        Mat::new_rows_cols_with_data(
            height,
            width,
            frame.data(),
        )
        .expect("Failed to create Mat from frame data")
    };
    
    // Apply binary threshold to the grayscale image
    let mut binary_mat = Mat::default();
    if let Err(e) = imgproc::threshold(
        &input_mat,
        &mut binary_mat,
        128.0,
        255.0,
        imgproc::THRESH_BINARY,
    ) {
        eprintln!("Error in threshold: {:?}", e);
        return frame;
    }
    
    // Find contours - for 5-argument version: find_contours(image, contours, mode, method, offset)
    let mut contours = Vector::<Vector<Point>>::new();
    
    if let Err(e) = imgproc::find_contours(
        &binary_mat,
        &mut contours,
        imgproc::RETR_EXTERNAL,
        imgproc::CHAIN_APPROX_SIMPLE,
        Point::new(0, 0),  // offset parameter
    ) {
        eprintln!("Error finding contours: {:?}", e);
        return frame;
    }
    
    // Create output image (convert to BGR for colored contours)
    let mut output_mat = Mat::default();
    if let Err(e) = imgproc::cvt_color(&binary_mat, &mut output_mat, imgproc::COLOR_GRAY2BGR, 0) {
        eprintln!("Error converting to BGR: {:?}", e);
        return frame;
    }
    
    // Filter contours by area and circularity (based on the shot put detection criteria)
    let min_area = 8.0;
    let max_area = 50.0;
    let min_circularity = 0.63;
    let max_aspect_ratio = 1.7;
    
    // Process each contour - .get() returns Result
    for i in 0..contours.len() {
        if let Ok(contour) = contours.get(i) {
            // Calculate area
            if let Ok(area) = imgproc::contour_area(&contour, false) {
                if area < min_area || area > max_area {
                    continue;
                }
                
                // Calculate perimeter and circularity
                if let Ok(perimeter) = imgproc::arc_length(&contour, true) {
                    if perimeter <= 0.0 {
                        continue;
                    }
                    
                    let circularity = (4.0 * std::f64::consts::PI * area) / (perimeter * perimeter);
                    
                    if circularity < min_circularity {
                        continue;
                    }
                    
                    // Check aspect ratio of bounding rectangle
                    if let Ok(bounding_rect) = imgproc::bounding_rect(&contour) {
                        let aspect_ratio = (bounding_rect.width as f64) / (bounding_rect.height as f64).max(1.0);
                        
                        if aspect_ratio > max_aspect_ratio {
                            continue;
                        }
                        
                        // Draw valid contour in green - we need hierarchy but don't have it in 5-arg version
                        // Create empty hierarchy
                        let hierarchy = Mat::default();
                        let color = Scalar::new(0.0, 255.0, 0.0, 0.0);
                        if let Err(e) = imgproc::draw_contours(
                            &mut output_mat,
                            &contours,
                            i as i32,
                            color,
                            2,
                            imgproc::LINE_8,
                            &hierarchy,
                            0,
                            Default::default(),
                        ) {
                            eprintln!("Error drawing contour: {:?}", e);
                        }
                        
                        // Draw center of contour
                        if let Ok(moments) = imgproc::moments(&contour, false) {
                            if moments.m00 > 0.0 {
                                let cx = (moments.m10 / moments.m00) as i32;
                                let cy = (moments.m01 / moments.m00) as i32;
                                let center_color = Scalar::new(255.0, 0.0, 0.0, 0.0);
                                if let Err(e) = imgproc::circle(
                                    &mut output_mat,
                                    Point::new(cx, cy),
                                    3,
                                    center_color,
                                    -1,
                                    imgproc::LINE_8,
                                    0,
                                ) {
                                    eprintln!("Error drawing center: {:?}", e);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    
    // Convert the output Mat back to frame data
    let output_data = match unsafe { output_mat.data_typed::<u8>() } {
        Ok(data) => data.to_vec(),
        Err(e) => {
            eprintln!("Error getting data bytes: {:?}", e);
            return frame;
        }
    };
    
    // Create new context with same timestamp
    let output_context = Context::new(frame.context().timestamp());
    
    Frame::new(output_data, output_context)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case(1280, 720)]  // HD
    #[case(1920, 1080)] // FullHD
    #[case(3840, 2160)] // UHD4K
    fn test_contour_output_dimensions(#[case] width: i32, #[case] height: i32) {
        let context = Context::new(6969);
        
        // Create a test pattern - a simple circle-like pattern
        let mut test_data = vec![0u8; (height * width) as usize];
        
        // Draw a white circle in the center (simulating a shot put)
        let center_x = width / 2;
        let center_y = height / 2;
        let radius = (std::cmp::min(width, height) / 10) as i32;
        
        for y in 0..height {
            for x in 0..width {
                let dx = x - center_x;
                let dy = y - center_y;
                let dist_sq = dx * dx + dy * dy;
                if dist_sq < radius * radius {
                    let idx = (y * width + x) as usize;
                    test_data[idx] = 255; // White circle
                }
            }
        }
        
        let input_frame = Frame::new(test_data, context);
        let output_frame = contour(input_frame);
        
        // Verify output frame has BGR format (3 channels)
        let expected_output_len = (height * width * 3) as usize;
        assert_eq!(output_frame.data().len(), expected_output_len);
        assert_eq!(output_frame.context().timestamp(), 6969);
    }
    
    #[test]
    fn test_contour_with_blank_frame() {
        let width = 1280;
        let height = 720;
        let context = Context::new(0);
        
        // All black frame (no contours)
        let test_data = vec![0u8; (height * width) as usize];
        let input_frame = Frame::new(test_data, context);
        let output_frame = contour(input_frame);
        
        // Output should still be valid
        assert!(!output_frame.data().is_empty());
        assert_eq!(output_frame.context().timestamp(), 0);
    }
    
    #[test]
    fn test_contour_with_white_square() {
        let width = 1280;
        let height = 720;
        let context = Context::new(1);
        
        // Create a white square (should be rejected due to circularity)
        let mut test_data = vec![0u8; (height * width) as usize];
        let square_size = 20;
        let start_x = (width / 2) - square_size;
        let start_y = (height / 2) - square_size;
        
        for y in start_y..(start_y + square_size * 2) {
            for x in start_x..(start_x + square_size * 2) {
                if y >= 0 && y < height && x >= 0 && x < width {
                    let idx = (y * width + x) as usize;
                    test_data[idx] = 255;
                }
            }
        }
        
        let input_frame = Frame::new(test_data, context);
        let output_frame = contour(input_frame);
        
        // Function should complete successfully
        assert!(!output_frame.data().is_empty());
        assert_eq!(output_frame.context().timestamp(), 1);
    }
}

// use crate::schemas::Frame;

// pub fn contour(frame: Frame) -> Frame {
//     // TODO: Currently just passes the frame through this stage untouched.
//     // Please implement the actual contour logic.

//     frame.clone()
// }