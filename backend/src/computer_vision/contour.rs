use crate::camera::Resolution;
use crate::pipeline::{Context, Frame};
use opencv::core::{Mat, Scalar, Point, Vector};
use opencv::imgproc;
use opencv::prelude::*;

pub fn contour(frame: Frame) -> Frame {
    // Get the input Mat from the frame
    let input_mat = frame.data();
    
    // Get dimensions from the Mat
    let _width = input_mat.cols();
    let _height = input_mat.rows();
    
    // Apply binary threshold to the grayscale image
    let mut binary_mat = Mat::default();
    if let Err(e) = imgproc::threshold(
        input_mat,
        &mut binary_mat,
        128.0,
        255.0,
        imgproc::THRESH_BINARY,
    ) {
        eprintln!("Error in threshold: {:?}", e);
        return frame;
    }
    
    // Find contours
    let mut contours = Vector::<Vector<Point>>::new();
    
    if let Err(e) = imgproc::find_contours(
        &binary_mat,
        &mut contours,
        imgproc::RETR_EXTERNAL,
        imgproc::CHAIN_APPROX_SIMPLE,
        Point::new(0, 0),
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
    
    // Process each contour
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
                        
                        // Draw valid contour in green
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
    
    // Create new context with same timestamp and resolution
    let output_context = Context::new(
        frame.context().timestamp(),
        frame.context().resolution(),
    );
    
    Frame::new(output_mat, output_context)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    // Helper function to create a test frame
    fn generate_test_frame(width: i32, height: i32, timestamp: u64, resolution: Resolution) -> Frame {
        let mut test_data = vec![0u8; (height * width) as usize];
        
        // white circle in the center (shot put)
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
                    test_data[idx] = 255; 
                }
            }
        }
        let mat_ref = Mat::new_rows_cols_with_data(height, width, &test_data)
            .expect("Failed to create test Mat");
        let mat = mat_ref.try_clone().expect("Failed to clone Mat");
        
        let context = Context::new(timestamp, resolution);
        Frame::new(mat, context)
    }

    #[rstest]
    #[case(Resolution::HD)]
    #[case(Resolution::FullHD)]
    #[case(Resolution::UHD4K)]
    fn test_contour_output_dimensions(#[case] resolution: Resolution) {
        let (height, width) = resolution.dimensions();
        let input_frame = generate_test_frame(width, height, 6969, resolution);
        let output_frame = contour(input_frame);
        
        // Output should be BGR format (3 channels)
        let output_mat = output_frame.data();
        assert_eq!(output_mat.channels(), 3);
        assert_eq!(output_frame.context().timestamp(), 6969);
        assert_eq!(output_frame.context().resolution(), resolution);
    }
    
    #[test]
    fn test_contour_with_blank_frame() {
        let resolution = Resolution::HD;
        let (height, width) = resolution.dimensions();
        
        // All black frame (no contours)
        let test_data = vec![0u8; (height * width) as usize];
        let mat_ref = Mat::new_rows_cols_with_data(height, width, &test_data)
            .expect("Failed to create test Mat");
        let mat = mat_ref.try_clone().expect("Failed to clone Mat");
        let context = Context::new(0, resolution);
        let input_frame = Frame::new(mat, context);
        let output_frame = contour(input_frame);
        
        // Output should still be valid
        assert!(!output_frame.data().empty());
        assert_eq!(output_frame.context().timestamp(), 0);
    }
    
    #[test]
    fn test_contour_with_white_square() {
        let resolution = Resolution::HD;
        let (height, width) = resolution.dimensions();
        
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
        
        let mat_ref = Mat::new_rows_cols_with_data(height, width, &test_data)
            .expect("Failed to create test Mat");
        let mat = mat_ref.try_clone().expect("Failed to clone Mat");
        let context = Context::new(1, resolution);
        let input_frame = Frame::new(mat, context);
        let output_frame = contour(input_frame);
        
        assert!(!output_frame.data().empty());
        assert_eq!(output_frame.context().timestamp(), 1);
    }
    
    #[test]
    fn test_contour_preserves_resolution() {
        let resolution = Resolution::FullHD;
        let (height, width) = resolution.dimensions();
        let timestamp = 12345;
        
        let test_data = vec![128u8; (height * width) as usize];
        let mat_ref = Mat::new_rows_cols_with_data(height, width, &test_data)
            .expect("Failed to create test Mat");
        let mat = mat_ref.try_clone().expect("Failed to clone Mat");
        let context = Context::new(timestamp, resolution);
        let input_frame = Frame::new(mat, context);
        let output_frame = contour(input_frame);
        
        assert_eq!(output_frame.context().resolution(), resolution);
        assert_eq!(output_frame.context().timestamp(), timestamp);
    }   
    
}