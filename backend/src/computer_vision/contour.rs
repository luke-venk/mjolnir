use crate::pipeline::Frame;
use opencv::core::{self, CV_32F, Mat, Point, Point2f, Scalar, Vector};
use opencv::imgproc;
use opencv::prelude::*;
use opencv::video::{KalmanFilter, KalmanFilterTrait, KalmanFilterTraitConst};
use std::cell::RefCell;
use std::collections::VecDeque;

const MIN_AREA: f64 = 8.0;
const MAX_AREA: f64 = 50.0;
const MAX_PERIMETER: f64 = 70.0;
const MIN_CIRCULARITY: f64 = 0.63;
const MAX_ASPECT_RATIO: f64 = 1.7;
const ROI_SIZE: i32 = 100;
const ROI_PADDING: i32 = 20;
const MIN_ROI_SIZE: i32 = 100;
const CONSISTENCY_WINDOW: usize = 3;
const MAX_DISTANCE_VARIATION: f64 = 15.0;
const MIN_CONSISTENT_DETECTIONS: usize = 4;
const MAX_MISSED_FRAMES: usize = 8;
const TRAIL_LENGTH: usize = 60;
const GATE_DISTANCE_PX: f64 = 80.0;
const CLOSE_NON_ROI_DISTANCE_PX: f64 = 20.0;
const EXPECTED_RADIUS_PX: f64 = 6.0;
const GRAVITY_PX_PER_FRAME2: f32 = 0.01;

#[derive(Debug, Clone, PartialEq)]
struct Candidate {
    cx: f64,
    cy: f64,
    radius: f64,
    circularity: f64,
}

#[derive(Debug, Clone, Copy)]
struct RoiWindow {
    left: i32,
    right: i32,
    top: i32,
    bottom: i32,
}

pub struct ContourTracker {
    kf: KalmanFilter,
    initialized: bool,
    missed: usize,
    trail: VecDeque<(f64, f64)>,
    predicted: Option<(f64, f64)>,
    last_position: Option<(f64, f64)>,
    recent_positions: VecDeque<(f64, f64)>,
    use_roi: bool,
    consistent_detection_count: usize,
}

impl ContourTracker {
    pub fn new() -> Self {
        Self {
            kf: make_kalman().expect("failed to create kalman filter"),
            initialized: false,
            missed: 0,
            trail: VecDeque::with_capacity(TRAIL_LENGTH),
            predicted: None,
            last_position: None,
            recent_positions: VecDeque::with_capacity(CONSISTENCY_WINDOW),
            use_roi: false,
            consistent_detection_count: 0,
        }
    }

    pub fn process_frame(&mut self, mut frame: Frame) -> Frame {
        let Some(mask) = frame.downsampled_image() else {
            self.mark_missed();
            frame.context_mut().set_detected(Some(false));
            frame.context_mut().set_centroid(None);
            return frame;
        };

        let roi = self.get_roi_from_prediction(mask.rows(), mask.cols());
        let candidates = detect_candidates(&mask);
        let best = self.pick_best_candidate_with_roi_priority(&candidates, roi);

        match best {
            Some(candidate) => {
                self.correct(candidate.cx, candidate.cy);
                frame.context_mut().set_detected(Some(true));
                frame.context_mut().set_centroid(Some((candidate.cx, candidate.cy)));
            }
            None => {
                self.mark_missed();
                frame.context_mut().set_detected(Some(false));
                frame.context_mut().set_centroid(None);
            }
        }

        frame
    }

    fn predict(&mut self) -> Option<(f64, f64)> {
        let pred = self.kf.predict_def().ok()?;
        let mut state_post = self.kf.state_post();
        if let Ok(vy) = state_post.at_2d_mut::<f32>(3, 0) {
            *vy += GRAVITY_PX_PER_FRAME2;
        }
        self.kf.set_state_post(state_post);

        let x = pred.at_2d::<f32>(0, 0).ok().copied()? as f64;
        let y = pred.at_2d::<f32>(1, 0).ok().copied()? as f64;
        let predicted = (x, y);
        self.predicted = Some(predicted);
        Some(predicted)
    }

    fn correct(&mut self, cx: f64, cy: f64) {
        let measurement = Mat::from_slice_2d(&[[cx as f32], [cy as f32]])
            .expect("failed to create kalman measurement matrix");
        let _ = self.kf.correct(&measurement);

        if !self.initialized {
            let mut state_post = self.kf.state_post();
            if let Ok(x) = state_post.at_2d_mut::<f32>(0, 0) {
                *x = cx as f32;
            }
            if let Ok(y) = state_post.at_2d_mut::<f32>(1, 0) {
                *y = cy as f32;
            }
            self.kf.set_state_post(state_post);
            self.initialized = true;
        }

        self.last_position = Some((cx, cy));
        self.trail.push_back((cx, cy));
        while self.trail.len() > TRAIL_LENGTH {
            let _ = self.trail.pop_front();
        }
        self.recent_positions.push_back((cx, cy));
        while self.recent_positions.len() > CONSISTENCY_WINDOW {
            let _ = self.recent_positions.pop_front();
        }
        self.missed = 0;
        self.update_roi_flag();
    }

    fn mark_missed(&mut self) {
        self.missed += 1;
        if self.missed > MAX_MISSED_FRAMES {
            *self = Self::new();
        }
    }

    fn update_roi_flag(&mut self) {
        if self.recent_positions.len() < MIN_CONSISTENT_DETECTIONS {
            self.use_roi = false;
            self.consistent_detection_count = 0;
            return;
        }

        let positions: Vec<(f64, f64)> = self.recent_positions.iter().copied().collect();
        let distances: Vec<f64> = positions
            .windows(2)
            .map(|window| euclidean_distance(window[0], window[1]))
            .collect();

        if distances.len() < 2 {
            self.use_roi = false;
            return;
        }

        let max_dist = distances.iter().copied().fold(f64::MIN, f64::max);
        let min_dist = distances.iter().copied().fold(f64::MAX, f64::min);

        if (max_dist - min_dist) < MAX_DISTANCE_VARIATION
            && max_dist < MAX_DISTANCE_VARIATION * 2.0
        {
            self.consistent_detection_count += 1;
            if self.consistent_detection_count >= MIN_CONSISTENT_DETECTIONS {
                self.use_roi = true;
            }
        } else {
            self.consistent_detection_count = 0;
            self.use_roi = false;
        }
    }

    fn get_roi_from_prediction(&self, height: i32, width: i32) -> RoiWindow {
        let (cx, cy, roi_size) = if self.initialized {
            if let Some((px, py)) = self.predicted {
                let roi_size = if self.trail.len() > 2 {
                    let state_post = self.kf.state_post();
                    let vx = state_post.at_2d::<f32>(2, 0).ok().copied().unwrap_or(0.0) as f64;
                    let vy = state_post.at_2d::<f32>(3, 0).ok().copied().unwrap_or(0.0) as f64;
                    let speed = (vx * vx + vy * vy).sqrt();
                    let dynamic_roi = (ROI_SIZE as f64 * (1.0 + speed / 20.0)) as i32;
                    (ROI_SIZE * 2).min(ROI_SIZE.max(dynamic_roi))
                } else {
                    ROI_SIZE
                };
                (px.round() as i32, py.round() as i32, roi_size)
            } else {
                (width / 2, height / 2, MIN_ROI_SIZE)
            }
        } else {
            (width / 2, height / 2, MIN_ROI_SIZE)
        };

        let left = 0.max(cx - roi_size / 2 - ROI_PADDING);
        let right = width.min(cx + roi_size / 2 + ROI_PADDING);
        let top = 0.max(cy - roi_size / 2 - ROI_PADDING);
        let bottom = height.min(cy + roi_size / 2 + ROI_PADDING);

        RoiWindow {
            left,
            right,
            top,
            bottom,
        }
    }

    fn pick_best_candidate_with_roi_priority<'a>(
        &mut self,
        candidates: &'a [Candidate],
        roi: RoiWindow,
    ) -> Option<&'a Candidate> {
        if candidates.is_empty() {
            return None;
        }

        if self.initialized {
            let _ = self.predict();
        }

        let (roi_candidates, non_roi_candidates): (Vec<&Candidate>, Vec<&Candidate>) = candidates
            .iter()
            .partition(|candidate| self.candidate_in_roi(candidate, roi));

        if self.consistent_detection_count >= MIN_CONSISTENT_DETECTIONS {
            if !roi_candidates.is_empty() {
                if self.initialized && self.predicted.is_some() {
                    return self.best_scored_candidate(roi_candidates);
                }
                return roi_candidates.into_iter().max_by(|a, b| {
                    a.circularity
                        .partial_cmp(&b.circularity)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
            }

            if !non_roi_candidates.is_empty() {
                if let Some((px, py)) = self.predicted {
                    let close_candidates: Vec<&Candidate> = non_roi_candidates
                        .into_iter()
                        .filter(|candidate| {
                            euclidean_distance((candidate.cx, candidate.cy), (px, py))
                                < CLOSE_NON_ROI_DISTANCE_PX
                        })
                        .collect();
                    if !close_candidates.is_empty() {
                        return self.best_scored_candidate(close_candidates);
                    }
                    return None;
                }
            }
        }

        if self.initialized {
            if let Some((px, py)) = self.predicted {
                let mut gated: Vec<&Candidate> = candidates
                    .iter()
                    .filter(|candidate| {
                        euclidean_distance((candidate.cx, candidate.cy), (px, py))
                            < GATE_DISTANCE_PX
                    })
                    .collect();

                if !gated.is_empty() && self.trail.len() > 2 {
                    let avg_speed = average_trail_speed(&self.trail);
                    let max_allowed = avg_speed * 2.5;
                    let speed_gated: Vec<&Candidate> = gated
                        .iter()
                        .copied()
                        .filter(|candidate| {
                            euclidean_distance((candidate.cx, candidate.cy), (px, py))
                                <= max_allowed
                        })
                        .collect();
                    if !speed_gated.is_empty() {
                        gated = speed_gated;
                    }
                }

                if !gated.is_empty() {
                    return self.best_scored_candidate(gated);
                }
            }
        }

        candidates.iter().max_by(|a, b| {
            a.circularity
                .partial_cmp(&b.circularity)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    }

    fn candidate_in_roi(&self, candidate: &Candidate, roi: RoiWindow) -> bool {
        let cx = candidate.cx.round() as i32;
        let cy = candidate.cy.round() as i32;
        roi.left <= cx && cx <= roi.right && roi.top <= cy && cy <= roi.bottom
    }

    fn best_scored_candidate<'a>(&self, candidates: Vec<&'a Candidate>) -> Option<&'a Candidate> {
        candidates.into_iter().min_by(|a, b| {
            self.candidate_score(a)
                .partial_cmp(&self.candidate_score(b))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    }

    fn candidate_score(&self, candidate: &Candidate) -> f64 {
        let Some((px, py)) = self.predicted else {
            return -candidate.circularity;
        };

        let dist = euclidean_distance((candidate.cx, candidate.cy), (px, py));

        let state_post = self.kf.state_post();
        let vx = state_post.at_2d::<f32>(2, 0).ok().copied().unwrap_or(0.0) as f64;
        let vy = state_post.at_2d::<f32>(3, 0).ok().copied().unwrap_or(0.0) as f64;
        let expected_x = px + vx;
        let expected_y = py + vy;
        let vel_err = euclidean_distance((candidate.cx, candidate.cy), (expected_x, expected_y));

        let size_err = if !self.trail.is_empty() {
            (candidate.radius - EXPECTED_RADIUS_PX).abs()
        } else {
            0.0
        };

        dist + 0.5 * vel_err + 0.5 * size_err - 2.0 * candidate.circularity
    }
}

impl Default for ContourTracker {
    fn default() -> Self {
        Self::new()
    }
}

thread_local! {
    static CONTOUR_TRACKER: RefCell<ContourTracker> = RefCell::new(ContourTracker::new());
}

pub fn contour(frame: Frame) -> Frame {
    CONTOUR_TRACKER.with(|tracker| tracker.borrow_mut().process_frame(frame))
}

fn make_kalman() -> opencv::Result<KalmanFilter> {
    let mut kf = KalmanFilter::new(4, 2, 0, CV_32F)?;

    kf.set_transition_matrix(Mat::from_slice_2d(&[
        [1.0f32, 0.0, 1.0, 0.0],
        [0.0, 1.0, 0.0, 1.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ])?);

    kf.set_measurement_matrix(Mat::from_slice_2d(&[
        [1.0f32, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
    ])?);

    let mut process_noise_cov = Mat::eye(4, 4, CV_32F)?.to_mat()?;
    core::set_identity(&mut process_noise_cov, Scalar::all(1e-2))?;
    *process_noise_cov.at_2d_mut::<f32>(2, 2)? = 1.0;
    *process_noise_cov.at_2d_mut::<f32>(3, 3)? = 1.0;
    kf.set_process_noise_cov(process_noise_cov);

    let mut measurement_noise_cov = Mat::eye(2, 2, CV_32F)?.to_mat()?;
    core::set_identity(&mut measurement_noise_cov, Scalar::all(5.0))?;
    kf.set_measurement_noise_cov(measurement_noise_cov);

    let mut error_cov_post = Mat::eye(4, 4, CV_32F)?.to_mat()?;
    core::set_identity(&mut error_cov_post, Scalar::all(1.0))?;
    kf.set_error_cov_post(error_cov_post);

    Ok(kf)
}

fn detect_candidates(mask: &impl opencv::core::ToInputArray) -> Vec<Candidate> {
    let mut contours = Vector::<Vector<Point>>::new();
    if imgproc::find_contours(
        mask,
        &mut contours,
        imgproc::RETR_EXTERNAL,
        imgproc::CHAIN_APPROX_SIMPLE,
        Point::new(0, 0),
    )
    .is_err()
    {
        return Vec::new();
    }

    let mut candidates = Vec::new();

    for contour in contours {
        let Ok(area) = imgproc::contour_area(&contour, false) else {
            continue;
        };
        if !(MIN_AREA..=MAX_AREA).contains(&area) {
            continue;
        }

        let Ok(bounding_rect) = imgproc::bounding_rect(&contour) else {
            continue;
        };
        let min_dim = bounding_rect.width.min(bounding_rect.height).max(1) as f64;
        let max_dim = bounding_rect.width.max(bounding_rect.height) as f64;
        let aspect = max_dim / min_dim;
        if aspect > MAX_ASPECT_RATIO {
            continue;
        }

        let Ok(perimeter) = imgproc::arc_length(&contour, true) else {
            continue;
        };
        if perimeter == 0.0 || perimeter > MAX_PERIMETER {
            continue;
        }

        let circularity = (4.0 * std::f64::consts::PI * area) / (perimeter * perimeter);
        if circularity < MIN_CIRCULARITY {
            continue;
        }

        let Ok(moments) = imgproc::moments(&contour, false) else {
            continue;
        };
        if moments.m00 == 0.0 {
            continue;
        }

        let cx = moments.m10 / moments.m00;
        let cy = moments.m01 / moments.m00;

        let mut center = Point2f::default();
        let mut radius = 0.0f32;
        if imgproc::min_enclosing_circle(&contour, &mut center, &mut radius).is_err() {
            continue;
        }

        candidates.push(Candidate {
            cx,
            cy,
            radius: radius as f64,
            circularity,
        });
    }

    candidates
}

fn euclidean_distance(a: (f64, f64), b: (f64, f64)) -> f64 {
    let dx = a.0 - b.0;
    let dy = a.1 - b.1;
    (dx * dx + dy * dy).sqrt()
}

fn average_trail_speed(trail: &VecDeque<(f64, f64)>) -> f64 {
    let distances: Vec<f64> = trail
        .iter()
        .zip(trail.iter().skip(1))
        .map(|(a, b)| euclidean_distance(*a, *b))
        .collect();

    if distances.is_empty() {
        0.0
    } else {
        distances.iter().sum::<f64>() / distances.len() as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::AtlasATP124SResolution;
    use crate::pipeline::test_utils::{ComputerVisionStage, generate_frame};
    use opencv::core::{Mat, Scalar};
    use rstest::rstest;

    fn blank_downsampled_frame() -> Frame {
        let frame = generate_frame(
            0,
            6969,
            AtlasATP124SResolution::Full,
            ComputerVisionStage::Contour,
        );

        let mask = Mat::new_rows_cols_with_default(540, 960, opencv::core::CV_8U, Scalar::all(0.0))
            .expect("failed to create blank mask");
        frame.clear_downsampled_image().unwrap();
        frame.set_downsampled_image(mask).unwrap();
        frame
    }

    #[rstest]
    fn test_contour_tracker_sets_false_and_none_when_no_candidate() {
        let frame = blank_downsampled_frame();
        let mut tracker = ContourTracker::new();
        let output = tracker.process_frame(frame);

        assert_eq!(output.context().detected(), Some(false));
        assert_eq!(output.context().centroid(), None);
    }

    #[rstest]
    fn test_contour_tracker_sets_true_and_centroid_for_valid_circle() {
        let frame = blank_downsampled_frame();
        let mut mask = frame.downsampled_image().unwrap();

        imgproc::circle(
            &mut mask,
            Point::new(200, 150),
            4,
            Scalar::all(255.0),
            -1,
            imgproc::LINE_8,
            0,
        )
        .unwrap();

        frame.clear_downsampled_image().unwrap();
        frame.set_downsampled_image(mask).unwrap();

        let mut tracker = ContourTracker::new();
        let output = tracker.process_frame(frame);

        assert_eq!(output.context().detected(), Some(true));
        let (cx, cy) = output.context().centroid().expect("expected centroid");
        assert!((cx - 200.0).abs() <= 1.0);
        assert!((cy - 150.0).abs() <= 1.0);
    }

    #[rstest]
    fn test_contour_tracker_rejects_large_blob() {
        let frame = blank_downsampled_frame();
        let mut mask = frame.downsampled_image().unwrap();

        imgproc::circle(
            &mut mask,
            Point::new(300, 200),
            20,
            Scalar::all(255.0),
            -1,
            imgproc::LINE_8,
            0,
        )
        .unwrap();

        frame.clear_downsampled_image().unwrap();
        frame.set_downsampled_image(mask).unwrap();

        let mut tracker = ContourTracker::new();
        let output = tracker.process_frame(frame);
        assert_eq!(output.context().detected(), Some(false));
        assert_eq!(output.context().centroid(), None);
    }

    #[rstest]
    fn test_make_kalman_configuration_matches_python_shape() {
        let kf = make_kalman().expect("expected kalman filter");
        assert_eq!(kf.transition_matrix().rows(), 4);
        assert_eq!(kf.transition_matrix().cols(), 4);
        assert_eq!(kf.measurement_matrix().rows(), 2);
        assert_eq!(kf.measurement_matrix().cols(), 4);
        assert_eq!(kf.process_noise_cov().rows(), 4);
        assert_eq!(kf.measurement_noise_cov().rows(), 2);
    }
}
