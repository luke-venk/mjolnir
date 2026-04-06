use super::{CameraSettings, FrameData};
use eframe::egui;
use eframe::egui::TextureHandle;
/// LiveViewApp implementation required for eframe to handle window creation
/// for our egui to render live streaming.
use std::{
    sync::{Arc, Mutex},
    time::Instant,
};

pub struct LiveViewApp {
    // Latest frame from the capture thread.
    pub latest_frame: Arc<Mutex<Option<FrameData>>>,
    // Measured frames per second, to show to user.
    pub actual_frame_rate: f32,
    // When latest frame was received, for FPS calculation.
    pub last_frame_time: Option<Instant>,
    // egui texture for the current frame.
    texture: Option<TextureHandle>,

    // Camera settings to be shared with this UI thread and background
    // capture thread. Stores settings that can be controlled
    // by the user.
    pub camera_settings: Arc<Mutex<CameraSettings>>,
    // Camera shutter speed, controlled via slider.
    pub shutter_speed: f64,
    // Desired frames per second, controlled via slider.
    pub desired_frame_rate: f64,
}

impl LiveViewApp {
    pub fn new(
        latest_frame: Arc<Mutex<Option<FrameData>>>,
        camera_settings: Arc<Mutex<CameraSettings>>,
    ) -> Self {
        let settings = camera_settings.lock().unwrap();
        let shutter_speed = settings.exposure_us;
        let desired_frame_rate = settings.frame_rate_hz;
        drop(settings);

        Self {
            latest_frame,
            actual_frame_rate: 0.0,
            last_frame_time: None,
            texture: None,
            camera_settings,
            shutter_speed,
            desired_frame_rate,
        }
    }
}

// See https://docs.rs/eframe/latest/eframe/trait.App.html.
impl eframe::App for LiveViewApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Check for a new frame from the capture thread.
        // Try to lock the mutex. If successful, take the frame out and leave none.
        // Otherwise, return none.
        let new_frame = self
            .latest_frame
            .lock()
            .ok()
            .and_then(|mut lock| lock.take());

        // If new_frame exists, update the state and UI.
        if let Some(frame) = new_frame {
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

            // Show sliders for user control of camera intrinsics.
            ui.horizontal(|ui| {
                ui.label("Exposure time (µs):");
                let shutter_speed_slider = ui.add(egui::Slider::new(&mut self.shutter_speed, 25.4..=20000.0));

                ui.add_space(16.0);
                ui.label("Desired frame rate (Hz):");
                let frame_rate_slider = ui.add(egui::Slider::new(&mut self.desired_frame_rate, 1.0..=42.5));

                // If either the sliders for exposure time or FPS were changed, update the shared camera settings
                // so capture thread can update the stream.
                if shutter_speed_slider.changed() || frame_rate_slider.changed() {
                    if let Ok(mut lock) = self.camera_settings.lock() {
                        *lock = CameraSettings::new(
                            self.shutter_speed,
                            self.desired_frame_rate,
                        );
                    }
                }
            });
            ui.add_space(8.0);

            // Render frame using the texture we made.
            if let Some(texture) = &self.texture {
                let available = ui.available_size();
                ui.image((texture.id(), available));
            } else {
                ui.centered_and_justified(|ui| {
                    ui.label("Waiting for camera stream...");
                });
            }

            // Show actual FPS at bottom left of screen.
            ui.add_space(4.0);
            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                ui.add_space(8.0);
                ui.label(egui::RichText::new(format!("Actual FPS: {:.1}", self.actual_frame_rate)).size(14.0));
            });
        });

        // Update the UI.
        ui.ctx().request_repaint();
    }
}
