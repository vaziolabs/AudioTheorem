use eframe::egui;
use anyhow::Result;

mod at2;

fn main() -> Result<()> {
    // Initialize logging for better debugging
    env_logger::init();
    println!("[MAIN] Starting AudioTheorem 2 Synthesizer");
    
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_icon(eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon.png"))?),
        ..Default::default()
    };
    
    println!("[MAIN] Attempting to run native GUI");
    eframe::run_native(
        "AT2 Synthesizer",
        options,
        Box::new(|_cc| {
            println!("[MAIN] Creating SynthApp instance");
            let app = match at2::SynthApp::new() {
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
