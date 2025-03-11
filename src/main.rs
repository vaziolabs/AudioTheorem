use eframe::egui;
use anyhow::Result;

mod core;
mod messaging;
mod ui;
mod app;
mod utils;

fn main() -> Result<()> {
    // Initialize logging for better debugging
    env_logger::init();
    println!("[MAIN] Starting AudioTheorem 2 Synthesizer");
    
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_min_inner_size([1000.0, 600.0])
            .with_resizable(true)
            .with_icon(eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon.png"))?),
        ..Default::default()
    };
    
    println!("[MAIN] Attempting to run native GUI");
    eframe::run_native(
        "AT2 Synthesizer",
        options,
        Box::new(|cc| {
            cc.egui_ctx.set_pixels_per_point(1.5);
            // Set up responsive frame
            let mut style = (*cc.egui_ctx.style()).clone();
            style.spacing.item_spacing = egui::vec2(10.0, 10.0);
            style.spacing.window_margin = egui::Margin::same(10);
            cc.egui_ctx.set_style(style);
            
            // Enable responsive layout
            cc.egui_ctx.set_visuals(egui::Visuals::dark());
            
            println!("[MAIN] Creating SynthApp instance");
            let app = match app::SynthApp::new() {
                Ok(app) => {
                    println!("[MAIN] SynthApp created successfully");
                    app
                },
                Err(e) => {
                    eprintln!("[MAIN] Failed to create app: {}", e);
                    std::process::exit(1);
                }
            };
            Ok(Box::new(app))
        }),
    ).map_err(|e| anyhow::anyhow!("[MAIN] Application error: {}", e))
}
