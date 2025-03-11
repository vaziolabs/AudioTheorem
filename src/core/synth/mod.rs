pub mod preset;
pub mod audio;
pub mod samples;

use preset::SynthPreset;
use crate::core::oscillator::{Oscillator, FilterType, OscillatorCombinationMode};
use crate::core::analyzer::Analyzer;
use crate::core::oscillator::Note;
use crate::core::synth::audio::midi_note_to_freq;

/// Main synthesizer engine
pub struct Synth {
    pub sample_rate: f32,
    pub volume: f32,
    pub active_notes: Vec<Note>,
    pub oscillators: Vec<Oscillator>, // Three oscillators
    pub oscillator_combination_mode: OscillatorCombinationMode,
    
    // Master envelope
    pub attack: f32,
    pub decay: f32,
    pub sustain: f32,
    pub release: f32,
    
    // Master filter
    pub master_filter_type: FilterType,
    pub master_filter_cutoff: f32,
    pub master_filter_resonance: f32,
    
    pub custom_wavetables: Vec<crate::core::oscillator::CustomWavetable>,
    pub analyzer: Analyzer,
    
    // Filter state variables (for each oscillator and master)
    pub filter_states: [[f32; 4]; 4], // 3 oscillators + 1 master, 4 states per filter
}

impl Synth {
    /// Create a new synthesizer instance
    pub fn new(sample_rate: f32) -> Self {
        Synth {
            sample_rate,
            volume: 0.5,
            active_notes: Vec::new(),
            oscillators: vec![
                Oscillator::new(),
                Oscillator::new(),
                Oscillator::new(),
            ],
            oscillator_combination_mode: OscillatorCombinationMode::Parallel,
            attack: 0.01,
            decay: 0.1,
            sustain: 0.7,
            release: 0.3,
            master_filter_type: FilterType::None,
            master_filter_cutoff: 1.0,
            master_filter_resonance: 0.0,
            custom_wavetables: Vec::new(),
            analyzer: Analyzer::new(),
            filter_states: [[0.0; 4]; 4],
        }
    }
    
    /// Apply settings from a preset
    pub fn apply_preset(&mut self, preset: &SynthPreset) {
        self.volume = preset.master_volume;
        self.attack = preset.master_attack;
        self.decay = preset.master_decay;
        self.sustain = preset.master_sustain;
        self.release = preset.master_release;
        
        self.master_filter_type = preset.master_filter_type.clone();
        self.master_filter_cutoff = preset.master_filter_cutoff;
        self.master_filter_resonance = preset.master_filter_resonance;
        
        self.oscillator_combination_mode = preset.oscillator_combination_mode.clone();
        
        // Copy oscillators from preset (assuming preset.oscillators is also an array)
        self.oscillators = preset.oscillators.clone();
    }
    
    /// Create a preset from current settings
    pub fn create_preset(&self, name: &str, author: &str, description: &str) -> SynthPreset {
        SynthPreset {
            name: name.to_string(),
            description: description.to_string(),
            author: author.to_string(),
            tags: Vec::new(),
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            
            master_volume: self.volume,
            master_filter_type: self.master_filter_type.clone(),
            master_filter_cutoff: self.master_filter_cutoff,
            master_filter_resonance: self.master_filter_resonance,
            
            master_attack: self.attack,
            master_decay: self.decay,
            master_sustain: self.sustain,
            master_release: self.release,
            
            oscillators: self.oscillators.clone(),
            oscillator_combination_mode: self.oscillator_combination_mode.clone(),
        }
    }
    
    /// Handle note on event
    pub fn note_on(&mut self, note: u8, velocity: u8) {
        if !self.active_notes.iter().any(|n| n.midi_note == note) {
            self.active_notes.push(Note {
                midi_note: note,
                frequency: midi_note_to_freq(note),
                phase: 0.0,
                phase_increment: 0.0,
                velocity: velocity as f32,
                ..Default::default()
            });
            
            // Assign note to an available oscillator
            for osc in &mut self.oscillators {
                if osc.note.is_none() {
                    osc.note = Some(note);
                    break;
                }
            }
        }
    }
    
    /// Handle note off event
    pub fn note_off(&mut self, note: u8) {
        if let Some(pos) = self.active_notes.iter().position(|n| n.midi_note == note) {
            self.active_notes.remove(pos);
            
            // Release the note from oscillators
            for osc in &mut self.oscillators {
                if osc.note == Some(note) {
                    osc.note = None;
                }
            }
        }
    }
    
    /// Set sustain pedal state
    pub fn set_sustain_pedal(&mut self, _on: bool) {
        // Implementation depends on your note handling logic
    }

    /// Add a proper setter for volume
    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
    }
}

// Add the missing constant
pub const WAVEFORM_DISPLAY_POINTS: usize = 256;
