use eframe::egui;
use global_hotkey::{
    hotkey::{Code, HotKey},
    GlobalHotKeyEvent, GlobalHotKeyManager,
};
use rfd::FileDialog;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use crate::config::AppConfig;
use crate::drawer::Drawer;
use crate::image_processing::process_image;
use crate::types::DrawingStroke;

pub struct VRChatDrawingApp {
    config: AppConfig,
    image_path: Option<PathBuf>,
    strokes: Vec<DrawingStroke>,
    status_msg: String,
    
    drawer: Arc<Drawer>,
    
    #[allow(dead_code)]
    hotkey_manager: GlobalHotKeyManager,
    hotkey_start: HotKey,
    hotkey_stop: HotKey,
}

impl Default for VRChatDrawingApp {
    fn default() -> Self {
        let manager = GlobalHotKeyManager::new().unwrap();
        let hotkey_start = HotKey::new(None, Code::F9);
        let hotkey_stop = HotKey::new(None, Code::F10);
        
        manager.register(hotkey_start).unwrap();
        manager.register(hotkey_stop).unwrap();
        
        Self {
            config: AppConfig::default(),
            image_path: None,
            strokes: Vec::new(),
            status_msg: "Ready. Press F9 to start, F10 to stop.".to_string(),
            drawer: Arc::new(Drawer::new()),
            hotkey_manager: manager,
            hotkey_start,
            hotkey_stop,
        }
    }
}

impl eframe::App for VRChatDrawingApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Handle hotkeys
        if let Ok(event) = GlobalHotKeyEvent::receiver().try_recv() {
            if event.id == self.hotkey_start.id() {
                if !self.strokes.is_empty() && !self.drawer.active.load(Ordering::SeqCst) {
                    self.status_msg = "Drawing in progress...".to_string();
                    self.drawer.start_drawing(self.strokes.clone(), self.config.drawing.clone());
                }
            } else if event.id == self.hotkey_stop.id() {
                self.drawer.stop_drawing();
                self.status_msg = "Drawing stopped.".to_string();
            }
        }

        // Custom dark theme with accent colors
        let mut visuals = egui::Visuals::dark();
        visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(30, 30, 40);
        visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(45, 45, 60);
        ctx.set_visuals(visuals);

        // Draw UI
        egui::SidePanel::left("control_panel").min_width(320.0).show(ctx, |ui| {
            ui.add_space(5.0);
            ui.heading(egui::RichText::new("VRChat Drawing Script").color(egui::Color32::from_rgb(120, 180, 255)));
            ui.label(egui::RichText::new("Rust Rewrite").small().color(egui::Color32::from_rgb(150, 150, 170)));
            ui.separator();

            // 1. Image Selection
            ui.group(|ui| {
                ui.label(egui::RichText::new("1. Select Image").strong().color(egui::Color32::from_rgb(100, 200, 150)));
                if ui.button("Open Image File").clicked() {
                    if let Some(path) = FileDialog::new()
                        .add_filter("Images", &["png", "jpg", "jpeg", "bmp"])
                        .pick_file()
                    {
                        self.status_msg = format!("Loaded: {}", path.file_name().unwrap().to_string_lossy());
                        self.image_path = Some(path);
                        self.strokes.clear();
                    }
                }
                if let Some(path) = &self.image_path {
                    ui.label(egui::RichText::new(path.file_name().unwrap_or_default().to_string_lossy()).color(egui::Color32::LIGHT_GRAY));
                } else {
                    ui.label(egui::RichText::new("No file selected").color(egui::Color32::GRAY));
                }
            });

            ui.add_space(3.0);

            // 2. Processing Params
            ui.group(|ui| {
                ui.label(egui::RichText::new("2. Image Processing").strong().color(egui::Color32::from_rgb(100, 200, 150)));
                
                ui.horizontal(|ui| {
                    ui.label("Threshold (1-254):");
                    ui.add(egui::Slider::new(&mut self.config.image.threshold_value, 1..=254));
                });
                
                ui.horizontal(|ui| {
                    ui.label("Simplify Epsilon:");
                    ui.add(egui::Slider::new(&mut self.config.contour.epsilon_ratio, 0.1..=10.0));
                });
                
                ui.horizontal(|ui| {
                    ui.label("Blur Size:");
                    ui.add(egui::Slider::new(&mut self.config.image.blur_size, 1..=15));
                });

                if ui.button("Process Image & Generate Strokes").clicked() {
                    if let Some(path) = &self.image_path {
                        if let Some(strokes) = process_image(path, &self.config) {
                            let total_points: usize = strokes.iter().map(|s| s.points.len()).sum();
                            self.status_msg = format!("Done! {} strokes, {} points.", strokes.len(), total_points);
                            self.strokes = strokes;
                        } else {
                            self.status_msg = "Processing failed!".to_string();
                            self.strokes.clear();
                        }
                    } else {
                        self.status_msg = "Please select an image first!".to_string();
                    }
                }
            });

            ui.add_space(3.0);

            // 3. Drawing Params
            ui.group(|ui| {
                ui.label(egui::RichText::new("3. Drawing Parameters").strong().color(egui::Color32::from_rgb(100, 200, 150)));
                
                ui.horizontal(|ui| {
                    ui.label("Sensitivity:");
                    ui.add(egui::Slider::new(&mut self.config.drawing.sensitivity, 0.1..=18.0));
                });
                
                let mut delay_ms = (self.config.drawing.draw_speed * 1000.0) as u32;
                ui.horizontal(|ui| {
                    ui.label("Draw Speed Point Delay (ms):");
                    if ui.add(egui::Slider::new(&mut delay_ms, 1..=200)).changed() {
                        self.config.drawing.draw_speed = (delay_ms as f64) / 1000.0;
                    }
                });

                ui.horizontal(|ui| {
                    ui.label("Max Step (px):");
                    ui.add(egui::Slider::new(&mut self.config.drawing.max_step_px, 1..=20));
                });
                
                ui.horizontal(|ui| {
                    ui.label("Lift Pen Speed (%):");
                    ui.add(egui::Slider::new(&mut self.config.drawing.lift_pen_speed, 1.0..=100.0));
                });
                
                ui.horizontal(|ui| {
                    ui.label("Vertical Stretch:");
                    ui.add(egui::Slider::new(&mut self.config.drawing.vertical_stretch, 0.2..=3.0));
                });
            });

            ui.add_space(3.0);

            // 4. Execution Controls
            ui.group(|ui| {
                ui.label(egui::RichText::new("4. Controls (Global Hotkeys)").strong().color(egui::Color32::from_rgb(100, 200, 150)));
                if ui.button("Start Drawing (F9)").clicked() {
                     if !self.strokes.is_empty() && !self.drawer.active.load(Ordering::SeqCst) {
                        self.status_msg = "Drawing in progress...".to_string();
                        self.drawer.start_drawing(self.strokes.clone(), self.config.drawing.clone());
                     }
                }
                if ui.button("Force Stop (F10)").clicked() {
                     self.drawer.stop_drawing();
                     self.status_msg = "Drawing stopped.".to_string();
                }
            });

            ui.add_space(10.0);

            // Status bar
            egui::Frame::none()
                .fill(egui::Color32::from_rgb(25, 25, 35))
                .rounding(4.0)
                .inner_margin(8.0)
                .show(ui, |ui| {
                    ui.label(egui::RichText::new(format!("Status: {}", self.status_msg)).color(egui::Color32::from_rgb(255, 200, 80)));
                });

            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                ui.label(egui::RichText::new("space.bilibili.com/5145514").small().color(egui::Color32::from_rgb(130, 150, 180)));
            });
        });

        // Main Panel (Preview Canvas)
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading(egui::RichText::new("Stroke Preview (2D)").color(egui::Color32::from_rgb(180, 180, 200)));
            
            let available = ui.available_rect_before_wrap();
            let painter = ui.painter_at(available);
            
            if self.strokes.is_empty() {
                painter.text(
                    available.center(),
                    egui::Align2::CENTER_CENTER,
                    "No Image Processed",
                    egui::FontId::proportional(24.0),
                    egui::Color32::from_rgb(80, 80, 100),
                );
            } else {
                // Background
                painter.rect_filled(available, 4.0, egui::Color32::from_rgb(250, 250, 255));
                
                // Find bounds
                let mut min_x = f64::INFINITY;
                let mut min_y = f64::INFINITY;
                let mut max_x = f64::NEG_INFINITY;
                let mut max_y = f64::NEG_INFINITY;
                for stroke in &self.strokes {
                    for pt in &stroke.points {
                        if pt.x < min_x { min_x = pt.x; }
                        if pt.x > max_x { max_x = pt.x; }
                        if pt.y < min_y { min_y = pt.y; }
                        if pt.y > max_y { max_y = pt.y; }
                    }
                }
                
                let img_width = max_x - min_x;
                let img_height = max_y - min_y;
                let canvas_width = available.width() as f64 - 20.0;
                let canvas_height = available.height() as f64 - 20.0;
                
                let scale = if img_width > 0.0 && img_height > 0.0 {
                    (canvas_width / img_width).min(canvas_height / img_height)
                } else {
                    1.0
                };
                
                let pad_x = (available.width() as f64 - (img_width * scale)) / 2.0;
                let pad_y = (available.height() as f64 - (img_height * scale)) / 2.0;
                
                let stroke_color = egui::Color32::from_rgb(30, 30, 50);
                let stroke_width = 1.5;
                
                for stroke in &self.strokes {
                    if stroke.points.len() > 1 {
                        for i in 0..(stroke.points.len()-1) {
                            let p1 = &stroke.points[i];
                            let p2 = &stroke.points[i+1];
                            
                            let sp1 = egui::pos2(
                                (available.left() as f64 + (p1.x - min_x) * scale + pad_x) as f32,
                                (available.top() as f64 + (p1.y - min_y) * scale + pad_y) as f32,
                            );
                            let sp2 = egui::pos2(
                                (available.left() as f64 + (p2.x - min_x) * scale + pad_x) as f32,
                                (available.top() as f64 + (p2.y - min_y) * scale + pad_y) as f32,
                            );
                            
                            painter.line_segment([sp1, sp2], (stroke_width, stroke_color));
                        }
                    }
                }
            }
        });

        // Request repaint to keep hotkeys responsive
        ctx.request_repaint();
    }
}
