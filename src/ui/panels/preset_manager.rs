use egui::Ui;
use std::sync::mpsc;
use std::path::PathBuf;
use crate::messaging::{MessageBus, SynthMessage};
use crate::core::synth::preset::SynthPreset;

pub struct PresetManagerPanel {
    presets: Vec<SynthPreset>,
    current_preset_name: String,
    message_bus: MessageBus,
    sender: mpsc::Sender<SynthMessage>,
    preset_directory: PathBuf,
    available_presets: Vec<String>,
    new_preset_name: String,
    new_preset_author: String,
    new_preset_description: String,
    status_message: Option<(String, f32)>, // Message and time remaining
}

impl PresetManagerPanel {
    pub fn new(presets: Vec<SynthPreset>, current_preset_name: String, message_bus: MessageBus, sender: mpsc::Sender<SynthMessage>, preset_directory: PathBuf) -> Self {
        Self {
            presets,
            current_preset_name,
            message_bus,
            sender,
            preset_directory,
            available_presets: Vec::new(),
            new_preset_name: String::new(),
            new_preset_author: String::new(),
            new_preset_description: String::new(),
            status_message: None,
        }
    }
    
    /// Update available presets list
    pub fn refresh_presets(&mut self) {
        if let Ok(presets) = SynthPreset::list_presets(&self.preset_directory) {
            self.available_presets = presets;
        }
    }
    
    /// Update status message display
    pub fn update(&mut self, dt: f32) {
        if let Some((_, time_left)) = &mut self.status_message {
            *time_left -= dt;
            if *time_left <= 0.0 {
                self.status_message = None;
            }
        }
    }
    
    /// Show the preset manager UI
    pub fn show(&mut self, ui: &mut Ui) {
        ui.heading("Preset Manager");
        
        ui.horizontal(|ui| {
            ui.label("Preset Name:");
            ui.text_edit_singleline(&mut self.current_preset_name);
            
            if ui.button("Save").clicked() {
                let preset = SynthPreset {
                    name: self.current_preset_name.clone(),
                    // Add other preset fields
                    ..Default::default()
                };
                
                self.message_bus.send(SynthMessage::SavePreset(preset)).ok();
            }
        });
        
        ui.separator();
        
        ui.label("Available Presets:");
        for preset in &self.presets {
            if ui.selectable_label(self.current_preset_name == preset.name, &preset.name).clicked() {
                self.message_bus.send(SynthMessage::LoadPreset(preset.name.clone())).ok();
                self.current_preset_name = preset.name.clone();
            }
        }
    }
    
    fn set_status(&mut self, message: &str, duration: f32) {
        self.status_message = Some((message.to_string(), duration));
    }
}
