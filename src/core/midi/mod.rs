mod input;
mod output;
mod mapping;

pub use input::MidiInputHandler;
pub use output::MidiOutputHandler;
pub use mapping::{MidiMapping, MidiController, MidiControlTarget};

use std::sync::{Arc, Mutex};
use std::sync::mpsc::Sender;
use crate::messaging::SynthMessage;
use crate::core::synth::Synth;

/// Main MIDI system that handles both input and output
pub struct MidiSystem {
    pub input: MidiInputHandler,
    pub output: MidiOutputHandler,
    pub mapping: Arc<Mutex<MidiMapping>>,
    pub synth_state: Arc<Mutex<Synth>>,
}

impl MidiSystem {
    /// Create a new MIDI system
    pub fn new(message_sender: Sender<SynthMessage>) -> Self {
        Self {
            input: MidiInputHandler::new(message_sender),
            output: MidiOutputHandler::new(),
            mapping: Arc::new(Mutex::new(MidiMapping::new())),
            synth_state: Arc::new(Mutex::new(Synth::new(44100.0))),
        }
    }
    
    /// Initialize the MIDI system
    pub fn initialize(&mut self) -> Result<(), String> {
        // Try to load mapping from default location
        let config_dir = dirs::config_dir()
            .ok_or_else(|| "Could not determine config directory".to_string())?
            .join("rustsynth");
        
        let mapping_path = config_dir.join("midi_mapping.json");
        
        if mapping_path.exists() {
            match MidiMapping::load_from_file(&mapping_path) {
                Ok(mapping) => {
                    if let Ok(mut m) = self.mapping.lock() {
                        *m = mapping;
                    }
                },
                Err(err) => {
                    eprintln!("Failed to load MIDI mapping: {}", err);
                    if let Ok(mut m) = self.mapping.lock() {
                        *m = MidiMapping::create_default();
                    }
                }
            }
        } else {
            let default_mapping = MidiMapping::create_default();
            default_mapping.save_to_file(&mapping_path)?;
            if let Ok(mut m) = self.mapping.lock() {
                *m = default_mapping;
            }
        }
        
        Ok(())
    }
    
    /// Save the current MIDI mapping
    pub fn save_mapping(&self) -> Result<(), String> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| "Could not determine config directory".to_string())?
            .join("rustsynth");
        
        std::fs::create_dir_all(&config_dir)
            .map_err(|e| format!("Failed to create config directory: {}", e))?;
        
        let mapping_path = config_dir.join("midi_mapping.json");
        
        if let Ok(mapping) = self.mapping.lock() {
            mapping.save_to_file(&mapping_path)?;
        }
        
        Ok(())
    }
    
    /// Apply a received MIDI CC message using the current mapping
    pub fn apply_midi_cc(&self, channel: u8, cc: u8, value: u8, sender: &Sender<SynthMessage>) {
        let controller = MidiController { channel, cc_number: cc };
        
        // Get the mapping target and processed value
        if let Ok(mapping) = self.mapping.lock() {
            if let Some(target) = mapping.mappings.get(&controller) {
                if let Some(processed_value) = mapping.process_midi_value(&controller, value) {
                    // Convert MidiControlTarget to appropriate SynthMessage
                    let message = match target {
                        MidiControlTarget::MasterVolume => 
                            Some(SynthMessage::SetVolume(processed_value)),
                            
                        MidiControlTarget::MasterFilterCutoff =>
                            if let Some((_, type_val, _, res)) = self.get_master_filter_params() {
                                Some(SynthMessage::ChangeMasterFilter(type_val, processed_value, res))
                            } else { None },
                            
                        MidiControlTarget::MasterFilterResonance =>
                            if let Some((_, type_val, cut, _)) = self.get_master_filter_params() {
                                Some(SynthMessage::ChangeMasterFilter(type_val, cut, processed_value))
                            } else { None },
                            
                        MidiControlTarget::MasterAttack =>
                            if let Some((_, d, s, r)) = self.get_master_envelope_params() {
                                Some(SynthMessage::ChangeMasterEnvelope(processed_value, d, s, r))
                            } else { None },
                            
                        MidiControlTarget::MasterDecay =>
                            if let Some((a, _, s, r)) = self.get_master_envelope_params() {
                                Some(SynthMessage::ChangeMasterEnvelope(a, processed_value, s, r))
                            } else { None },
                            
                        MidiControlTarget::MasterSustain =>
                            if let Some((a, d, _, r)) = self.get_master_envelope_params() {
                                Some(SynthMessage::ChangeMasterEnvelope(a, d, processed_value, r))
                            } else { None },
                            
                        MidiControlTarget::MasterRelease =>
                            if let Some((a, d, s, _)) = self.get_master_envelope_params() {
                                Some(SynthMessage::ChangeMasterEnvelope(a, d, s, processed_value))
                            } else { None },
                            
                        MidiControlTarget::OscillatorModAmount(idx) =>
                            Some(SynthMessage::ChangeOscillatorModulation(
                                *idx, 
                                processed_value, 
                                crate::core::oscillator::ModulationTarget::Pitch // Default
                            )),
                            
                        // Handle other target types...
                        _ => None
                    };
                    
                    // Send the message if we created one
                    if let Some(msg) = message {
                        sender.send(msg).ok();
                    }
                }
            }
        }
    }
    
    // Helper method to get master filter parameters from other mapped controls
    fn get_master_filter_params(&self) -> Option<(MidiController, crate::core::oscillator::FilterType, f32, f32)> {
        let synth = self.synth_state.lock().unwrap();
        Some((
            MidiController { channel: 0, cc_number: 0 },
            synth.master_filter_type.clone(),
            synth.master_filter_cutoff,
            synth.master_filter_resonance,
        ))
    }
    
    // Helper method to get master envelope parameters from other mapped controls
    fn get_master_envelope_params(&self) -> Option<(f32, f32, f32, f32)> {
        // This would need to track the current envelope values
        // For simplicity, returning defaults
        Some((0.1, 0.2, 0.7, 0.3))
    }
}
