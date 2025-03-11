mod combination;
mod envelope;
mod filter;
mod modulation;
mod waveform;
mod processor;
mod note;

// Re-export key types so they're accessible from core::oscillator
pub use self::combination::OscillatorCombinationMode;
pub use self::waveform::Waveform;
pub use self::filter::FilterType;
pub use self::modulation::ModulationTarget;
pub use self::envelope::Envelope;
pub use self::note::{Note, NoteState};

use serde::{Serialize, Deserialize};

// Create a CustomWavetable struct here that can be re-exported
pub struct CustomWavetable {
    pub name: String,
    pub samples: Vec<f32>,
    pub sample_rate: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Oscillator {
    pub waveform: Waveform,
    pub volume: f32,
    pub detune: f32,       // Detune in semitones
    pub octave: i8,        // Octave shift (-4 to +4)
    
    // Per-oscillator envelope
    pub attack: f32,
    pub decay: f32,
    pub sustain: f32,
    pub release: f32,
    
    // Per-oscillator filter
    pub filter_type: FilterType,
    pub filter_cutoff: f32,  // 0.0 to 1.0 (normalized frequency)
    pub filter_resonance: f32, // 0.0 to 1.0
    
    // Modulation
    pub mod_amount: f32,    // 0.0 to 1.0
    pub mod_target: ModulationTarget,
    pub pitch_bend: f32,
    pub aftertouch: f32,
    pub channel_pressure: f32,
    pub note: Option<u8>,
}

impl Oscillator {
    pub fn new() -> Self {
        Self {
            waveform: Waveform::Sine,
            volume: 0.5,
            detune: 0.0,
            octave: 0,
            attack: 0.1,
            decay: 0.2,
            sustain: 0.7,
            release: 0.3,
            filter_type: FilterType::None,
            filter_cutoff: 1.0,
            filter_resonance: 0.0,
            mod_amount: 0.0,
            mod_target: ModulationTarget::None,
            pitch_bend: 0.0,
            aftertouch: 0.0,
            channel_pressure: 0.0,
            note: None,
        }
    }
    
    pub fn get_frequency_multiplier(&self) -> f32 {
        let octave_factor = 2.0f32.powf(self.octave as f32);
        let detune_factor = 2.0f32.powf(self.detune / 12.0);
        octave_factor * detune_factor
    }
    
    pub fn get_envelope(&self) -> Envelope {
        Envelope {
            attack: self.attack,
            decay: self.decay,
            sustain: self.sustain,
            release: self.release,
        }
    }
}
