use egui::Ui;
use crate::core::oscillator::{Oscillator, Waveform, FilterType, ModulationTarget};
use crate::messaging::SynthMessage;
use crate::ui::components::{WaveformPlot, EnvelopeEditor};
use crate::utils::audio_visualizer;
use crossbeam_channel::Sender;

pub struct OscillatorPanel {
    pub index: usize,
    pub sender: Sender<SynthMessage>,
}

impl OscillatorPanel {
    pub fn new(index: usize, sender: Sender<SynthMessage>) -> Self {
        Self { index, sender }
    }
    
    pub fn show(&mut self, ui: &mut Ui, oscillator: &mut Oscillator, custom_wavetables: &[crate::core::oscillator::CustomWavetable]) {
        ui.push_id(self.index, |ui| {
            ui.heading(format!("Oscillator {}", self.index + 1));
            
            // Generate oscillator-specific waveform preview
            let preview_points = audio_visualizer::generate_waveform_preview(
                &oscillator.waveform,
                custom_wavetables,
                256 // Use appropriate number of samples
            );
            
            // Waveform visualization
            WaveformPlot::new(preview_points)
                .height(100.0)
                .show(ui, format!("osc_{}", self.index));
            
            // Waveform type selector
            egui::ComboBox::new(format!("waveform_{}", self.index), "Waveform")
                .selected_text(format!("{:?}", oscillator.waveform))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut oscillator.waveform, Waveform::Sine, "Sine");
                    ui.selectable_value(&mut oscillator.waveform, Waveform::Square, "Square");
                    ui.selectable_value(&mut oscillator.waveform, Waveform::Saw, "Saw");
                    ui.selectable_value(&mut oscillator.waveform, Waveform::Triangle, "Triangle");
                    ui.selectable_value(&mut oscillator.waveform, Waveform::WhiteNoise, "White Noise");
                    for (i, wavetable) in custom_wavetables.iter().enumerate() {
                        let custom_opt = Waveform::CustomSample(i);
                        ui.selectable_value(&mut oscillator.waveform, custom_opt.clone(), format!("Custom: {}", wavetable.name));
                    }
                });
            
            // Volume, detune, octave controls
            let mut volume = oscillator.volume;
            let volume_changed = ui.add(egui::Slider::new(&mut volume, 0.0..=1.0).text("Volume")).changed();
            
            let mut detune = oscillator.detune;
            let detune_changed = ui.add(egui::Slider::new(&mut detune, -12.0..=12.0).text("Detune")).changed();
            
            let mut octave = oscillator.octave;
            let octave_changed = ui.add(egui::Slider::new(&mut octave, -4..=4).text("Octave")).changed();
            
            if volume_changed || detune_changed || octave_changed {
                self.sender.send(SynthMessage::ChangeOscillator(
                    self.index,
                    oscillator.waveform.clone(),
                    volume,
                    detune,
                    octave
                )).ok();
            }
            
            // Envelope controls
            ui.collapsing("Envelope (ADSR)", |ui| {
                if let Some(envelope) = EnvelopeEditor::new(&oscillator.get_envelope()).show(ui) {
                    self.sender.send(SynthMessage::ChangeOscillatorEnvelope(
                        self.index,
                        envelope.attack,
                        envelope.decay,
                        envelope.sustain,
                        envelope.release,
                    )).ok();
                }
            });
            
            // Filter controls
            ui.collapsing("Filter", |ui| {
                let mut filter_type = oscillator.filter_type.clone();
                let mut cutoff = oscillator.filter_cutoff;
                let mut resonance = oscillator.filter_resonance;
                
                egui::ComboBox::new(format!("filter_type_{}", self.index), "Filter")
                    .selected_text(format!("{:?}", filter_type))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut filter_type, FilterType::None, "None");
                        ui.selectable_value(&mut filter_type, FilterType::LowPass, "Low Pass");
                        ui.selectable_value(&mut filter_type, FilterType::HighPass, "High Pass");
                        ui.selectable_value(&mut filter_type, FilterType::BandPass, "Band Pass");
                    });
                
                let cutoff_changed = ui.add(egui::Slider::new(&mut cutoff, 0.01..=1.0).text("Cutoff")).changed();
                let resonance_changed = ui.add(egui::Slider::new(&mut resonance, 0.0..=1.0).text("Resonance")).changed();
                
                if cutoff_changed || resonance_changed {
                    self.sender.send(SynthMessage::ChangeOscillatorFilter(
                        self.index, filter_type.clone(), cutoff, resonance
                    )).ok();
                }
            });
            
            // Modulation controls
            ui.collapsing("Modulation", |ui| {
                let mut amount = oscillator.mod_amount;
                let mut target = oscillator.mod_target.clone();
                
                let amount_changed = ui.add(egui::Slider::new(&mut amount, 0.0..=1.0).text("Amount")).changed();
                
                let target_changed = egui::ComboBox::new(format!("mod_target_{}", self.index), "Target")
                    .selected_text(format!("{:?}", target))
                    .show_ui(ui, |ui| {
                        let mut changed = false;
                        
                        if ui.selectable_value(&mut target, ModulationTarget::None, "None").changed() {
                            changed = true;
                        }
                        if ui.selectable_value(&mut target, ModulationTarget::Pitch, "Pitch").changed() {
                            changed = true;
                        }
                        if ui.selectable_value(&mut target, ModulationTarget::FilterCutoff, "Filter Cutoff").changed() {
                            changed = true;
                        }
                        if ui.selectable_value(&mut target, ModulationTarget::Volume, "Volume").changed() {
                            changed = true;
                        }
                        
                        changed
                    }).inner.unwrap_or(false);
                
                if amount_changed || target_changed {
                    self.sender.send(SynthMessage::ChangeOscillatorModulation(
                        self.index, amount, target.clone()
                    )).ok();
                }
            });
        });
    }
}
