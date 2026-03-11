#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod types;
pub mod image_processing;
pub mod drawer;
pub mod gui;

use eframe::egui;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([950.0, 750.0])
            .with_title("VRChat Drawing Script"),
        ..Default::default()
    };
    eframe::run_native(
        "VRChat Drawing Script",
        options,
        Box::new(|_cc| Ok(Box::new(gui::VRChatDrawingApp::default()))),
    )
}
