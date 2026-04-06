/// LiveViewApp implementation required for eframe to handle window creation
/// for our egui to render live streaming.
use std::{sync::{Arc, Mutex}, time::Instant};
use super::FrameData;
use eframe::egui;
use eframe::egui::TextureHandle;

pub struct LiveViewApp {
    // Latest frame from the capture thread.
    pub latest_frame: Arc<Mutex<Option<FrameData>>>,
    // Measured frames per second.
    pub fps: f32,
    // When latest frame was received, for FPS calculation.
    pub last_frame_time: Option<Instant>,
    // egui texture for the current frame.
    texture: Option<TextureHandle>,
}

impl LiveViewApp {
    pub fn new(latest_frame: Arc<Mutex<Option<FrameData>>>) -> Self {
        Self {
            latest_frame,
            fps: 0.0,
            last_frame_time: None,
            texture: None,
        }
    }
}

impl eframe::App for LiveViewApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Check for a new frame from the capture thread.
        // Try to lock the mutex. If successful, take the frame out and leave none.
        // Otherwise, return none.
        let new_frame = self.latest_frame.lock().ok().and_then(|mut lock| lock.take());

        // If new_frame exists, update the state and UI.
        if let Some(frame) = new_frame {
            // Update the frames per second.
            let now = Instant::now();
            if let Some(last_time) = self.last_frame_time {
                let elapsed = now.duration_since(last_time).as_secs_f32();
                if elapsed > 0.0 {
                    self.fps = 1.0 / elapsed;
                }
            }
            self.last_frame_time = Some(now);

            // Convert Mono8 to RGBA format for egui.
            // Involves just duplicating the value for red, green, and blue,
            // and setting opacity as 255.
            let rgba: Vec<u8> = frame.pixels.iter()
                .flat_map(|&v| [v, v, v, 255])
                .collect();

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
                        .strong()
                );
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

            // Show FPS at bottom left of screen.
            ui.add_space(4.0);
            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                ui.add_space(8.0);
                ui.label(egui::RichText::new(format!("FPS: {:.1}", self.fps))
                    .size(14.0)
            );
            });
        });

        // Update the UI.
        ui.ctx().request_repaint();
    }
}
