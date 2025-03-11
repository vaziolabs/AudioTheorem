use egui::Ui;
use std::sync::mpsc;
use crate::core::oscillator::{Oscillator, FilterType, OscillatorCombinationMode};
use crate::messaging::SynthMessage;
use crate::ui::components::WaveformPlot;
use crate::utils::audio_visualizer;

pub struct MasterPanel {
    sender: mpsc::Sender<SynthMessage>,
}

impl MasterPanel {
    pub fn new(sender: mpsc::Sender<SynthMessage>) -> Self {
        Self { sender }
    }
    
    pub fn show(
        &self, 
        ui: &mut Ui, 
        oscillators: &[Oscillator], 
        combination_mode: &OscillatorCombinationMode,
        custom_wavetables: &[crate::core::oscillator::CustomWavetable],
        volume: f32,
        attack: f32,
        decay: f32,
        sustain: f32,
        release: f32,
        filter_type: &FilterType,
        filter_cutoff: f32,
        filter_resonance: f32,
    ) {
        ui.separator();
        ui.heading("Master Output");
        
        // Oscillator combination mode selector
        let mut current_mode = combination_mode.clone();
        ui.horizontal(|ui| {
            ui.label("Combination Mode:");
            egui::ComboBox::new("combination_mode", "Combination Mode")
                .selected_text(format!("{:?}", current_mode))
                .show_ui(ui, |ui| {
                    for mode in &[
                        OscillatorCombinationMode::Parallel,
                        OscillatorCombinationMode::FM,
                        OscillatorCombinationMode::AM,
                        OscillatorCombinationMode::RingMod,
                        OscillatorCombinationMode::Filter,
                    ] {
                        if ui.selectable_value(&mut current_mode, mode.clone(), 
                                             format!("{:?}", mode)).changed() {
                            // Send message to update the actual synth
                            self.sender.send(SynthMessage::ChangeOscillatorCombinationMode(
                                mode.clone()
                            )).ok();
                        }
                    }
                });
        });
        
        // Generate combined waveform from all oscillators
        let master_points = audio_visualizer::generate_wavetable_display(
            oscillators,
            combination_mode,
            custom_wavetables
        );
        
        // Display the combined waveform
        WaveformPlot::new(master_points)
            .height(150.0)
            .color(egui::Color32::from_rgb(0, 188, 212))
            .show(ui, "master_waveform");
        
        // Master volume control
        let mut master_volume = volume;
        if ui.add(egui::Slider::new(&mut master_volume, 0.0..=1.0).text("Master Volume")).changed() {
            self.sender.send(SynthMessage::SetVolume(master_volume)).ok();
        }
        
        // Master ADSR controls
        ui.collapsing("Master Envelope (ADSR)", |ui| {
            let mut attack_val = attack;
            let mut decay_val = decay;
            let mut sustain_val = sustain;
            let mut release_val = release;
            
            let attack_changed = ui.add(egui::Slider::new(&mut attack_val, 0.01..=2.0).text("Attack")).changed();
            let decay_changed = ui.add(egui::Slider::new(&mut decay_val, 0.01..=2.0).text("Decay")).changed();
            let sustain_changed = ui.add(egui::Slider::new(&mut sustain_val, 0.0..=1.0).text("Sustain")).changed();
            let release_changed = ui.add(egui::Slider::new(&mut release_val, 0.01..=3.0).text("Release")).changed();
            
            if attack_changed || decay_changed || sustain_changed || release_changed {
                self.sender.send(SynthMessage::ChangeMasterEnvelope(
                    attack_val, decay_val, sustain_val, release_val
                )).ok();
            }
        });
        
        // Master filter controls
        ui.collapsing("Master Filter", |ui| {
            let mut filter_type_val = filter_type.clone();
            let mut cutoff_val = filter_cutoff;
            let mut resonance_val = filter_resonance;
            
            ui.horizontal(|ui| {
                ui.label("Type:");
                egui::ComboBox::new("master_filter_type", "Filter Type")
                    .selected_text(format!("{:?}", filter_type_val))
                    .show_ui(ui, |ui| {
                        for ftype in &[FilterType::None, FilterType::LowPass, 
                                    FilterType::HighPass, FilterType::BandPass] {
                            if ui.selectable_value(&mut filter_type_val, ftype.clone(), 
                                                 format!("{:?}", ftype)).changed() {
                                self.sender.send(SynthMessage::ChangeMasterFilter(
                                    ftype.clone(), cutoff_val, resonance_val
                                )).ok();
                            }
                        }
                    });
            });
            
            let cutoff_changed = ui.add(egui::Slider::new(&mut cutoff_val, 0.01..=1.0).text("Cutoff")).changed();
            let resonance_changed = ui.add(egui::Slider::new(&mut resonance_val, 0.0..=1.0).text("Resonance")).changed();
            
            if cutoff_changed || resonance_changed {
                self.sender.send(SynthMessage::ChangeMasterFilter(
                    filter_type_val.clone(), cutoff_val, resonance_val
                )).ok();
            }
        });
    }
}
