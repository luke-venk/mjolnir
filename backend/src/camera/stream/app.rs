// LiveViewApp implementation required for eframe to handle window creation
// for our egui to render live streaming.
use super::frame::FrameData;
use backend_lib::camera::{CameraIngestConfig, AtlasATP124SResolution};
use eframe::egui;
use eframe::egui::TextureHandle;
use std::{
    sync::{Arc, Mutex},
    time::Instant,
};

// See camera specs at the following link:
// https://www.edmundoptics.com/p/lucid-vision-labst-atlas-atp124s-mc-sony-imx545-123mp-ip67-monochrome-camera/49821/
const CAMERA_EXPOSURE_TIME_MICROSECONDS_MIN: f64 = 25.4;
const CAMERA_EXPOSURE_TIME_MICROSECONDS_MAX: f64 = 100000000.0;
const CAMERA_FRAMES_PER_SECOND_MIN: f64 = 1.0;
const CAMERA_FRAMES_PER_SECOND_MAX: f64 = 42.5;

pub struct LiveViewApp {
    // Receiver for the latest frame from the capture thread.
    pub frame_rx: crossbeam::channel::Receiver<FrameData>,
    // Measured frames per second, to show to user.
    pub actual_frame_rate: f32,
    // When latest frame was received, for FPS calculation.
    pub last_frame_time: Option<Instant>,
    // egui texture for the current frame.
    texture: Option<TextureHandle>,

    // Camera settings to be shared with this UI thread and background
    // capture thread. Stores settings that can be controlled
    // by the user, like camera name, shutter speed, and desired frame rate.
    pub camera_settings: Arc<Mutex<CameraIngestConfig>>,
}

impl LiveViewApp {
    pub fn new(
        frame_rx: crossbeam::channel::Receiver<FrameData>,
        camera_settings: Arc<Mutex<CameraIngestConfig>>,
    ) -> Self {
        Self {
            frame_rx,
            actual_frame_rate: 0.0,
            last_frame_time: None,
            texture: None,
            camera_settings,
        }
    }
}

// See https://docs.rs/eframe/latest/eframe/trait.App.html.
impl eframe::App for LiveViewApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // If new_frame exists, update the state and UI.
        if let Ok(frame) = self.frame_rx.try_recv() {
            // Update the frames per second.
            let now = Instant::now();
            if let Some(last_time) = self.last_frame_time {
                let elapsed = now.duration_since(last_time).as_secs_f32();
                if elapsed > 0.0 {
                    self.actual_frame_rate = 1.0 / elapsed;
                }
            }
            self.last_frame_time = Some(now);

            // Convert Mono8 to RGBA format for egui.
            // Involves just duplicating the value for red, green, and blue,
            // and setting opacity as 255.
            let rgba: Vec<u8> = frame.pixels.iter().flat_map(|&v| [v, v, v, 255]).collect();

            // Upload color image to texture. Necessary for rendering on UI.
            let color_image = egui::ColorImage::from_rgba_unmultiplied(
                [frame.width as usize, frame.height as usize],
                &rgba,
            );
            self.texture = Some(ui.ctx().load_texture(
                "camera_frame",
                color_image,
                egui::TextureOptions::LINEAR,
            ));
        }

        // Handle frame rendering.
        egui::CentralPanel::default().show_inside(ui, |ui| {
            // Show centered title at top of window.
            ui.add_space(8.0);
            ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new("Mjölnir Live Stream")
                        .size(24.0)
                        .strong(),
                );
            });
            ui.add_space(8.0);

            // Show elements for user control of camera intrinsics.
            ui.horizontal(|ui| {
                let mut settings = self
                    .camera_settings
                    .lock()
                    .expect("Error: Failed to lock camera settings mutex.");

                // Slider for exposure time.
                ui.label("Exposure time (µs):");
                ui.spacing_mut().slider_width = 250.0;
                ui.add(egui::Slider::new(
                    &mut settings.exposure_time_us,
                    CAMERA_EXPOSURE_TIME_MICROSECONDS_MIN..=CAMERA_EXPOSURE_TIME_MICROSECONDS_MAX,
                ).logarithmic(true));

                ui.add_space(16.0);

                // Slider for desired frame rate.
                ui.label("Desired frame rate (Hz):");
                ui.spacing_mut().slider_width = 100.0;
                ui.add(egui::Slider::new(&mut settings.frame_rate_hz, CAMERA_FRAMES_PER_SECOND_MIN..=CAMERA_FRAMES_PER_SECOND_MAX));

                // Button for resolution.
                ui.label("Resolution:");
                ui.selectable_value(&mut settings.resolution, AtlasATP124SResolution::Quarter, "Quarter");
                ui.selectable_value(&mut settings.resolution, AtlasATP124SResolution::Half, "Half");
                ui.selectable_value(&mut settings.resolution, AtlasATP124SResolution::Full, "Full");
            });
            ui.add_space(8.0);

            // Buttons to restart stream and quit the app.
            ui.horizontal(|ui| {
                // Button to restart stream and apply new specifications.
                {
                    let mut settings = self
                        .camera_settings
                        .lock()
                        .expect("Error: Failed to lock camera settings mutex.");

                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new("Apply Changes").color(egui::Color32::BLACK),
                            )
                            .fill(egui::Color32::from_rgb(46, 196, 29))
                            .stroke(egui::Stroke::new(2.0, egui::Color32::BLACK))
                            .min_size(egui::vec2(120.0, 40.0)),
                        )
                        .clicked()
                    {
                        settings.restart_requested = true;
                    }
                }

                // Button to quit app.
                if ui
                    .add(
                        egui::Button::new(
                            egui::RichText::new("Quit Stream").color(egui::Color32::BLACK),
                        )
                        .fill(egui::Color32::from_rgb(237, 33, 33))
                        .stroke(egui::Stroke::new(2.0, egui::Color32::BLACK))
                        .min_size(egui::vec2(120.0, 40.0)),
                    )
                    .clicked()
                {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
            ui.add_space(8.0);

            if let Some(texture) = &self.texture {
                // Render frame using the texture we made.
                let available = ui.available_size();
                ui.image((texture.id(), available));
            } else {
                // Show start screen.
                ui.centered_and_justified(|ui| {
                    ui.label("Waiting for camera stream to start...");
                });
            }

            // Show actual FPS at bottom left of screen.
            ui.add_space(4.0);
            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new(format!("Actual FPS: {:.1}", self.actual_frame_rate))
                        .size(14.0),
                );
            });
        });

        // Update the UI.
        ui.ctx().request_repaint();

        // If user clicks Cmd + W, close window.
        if ui
            .ctx()
            .input(|i| i.modifiers.command && i.key_pressed(egui::Key::W))
        {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    /// When the user closes the window, spit out the command that would begin recording
    /// with the camera intrinsics they currently had selected.
    fn on_exit(&mut self) {
        let settings = self
            .camera_settings
            .lock()
            .expect("Error: Failed to lock camera settings mutex.");
        println!();
        println!("Streaming stopped. See below the specs you had configured when closing:");
        println!(
            "  - Exposure time (nanoseconds): {}",
            settings.exposure_time_us
        );
        println!("  - Frame rate (Hz): {}", settings.frame_rate_hz);
        println!();
        println!(
            "Run the following command to begin recording with the cameras with your specs during streaming:"
        );
        println!();
        println!(
            "bazel run //backend:record -- --resolution {} --exposure-us {} --frame-rate-hz {} --output-dir <output-dir> <stop condition>",
            settings.resolution,
            settings.exposure_time_us,
            settings.frame_rate_hz,
        );
    }
}
