use serde::{Serialize, Deserialize};

// Types of waveforms
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Waveform {
    Sine,
    Square,
    Saw,
    Triangle,
    WhiteNoise,
    CustomSample(usize), // Index into the custom wavetables
}

// A custom wavetable loaded from a sample
pub struct CustomWavetable {
    pub name: String,
    pub samples: Vec<f32>,
    pub sample_rate: u32,
}

// Define filter types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FilterType {
    None,
    LowPass,
    HighPass,
    BandPass,
}

// Define modulation targets
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ModulationTarget {
    None,
    Pitch,
    FilterCutoff,
    Volume,
    PulseWidth,
}

// Expand the Oscillator struct with additional parameters
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

// Add this enum to define different ways to combine oscillators
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum OscillatorCombinationMode {
    Parallel,   // Simple addition of all oscillators
    FM,         // Frequency modulation (osc1 modulates osc2, which modulates osc3)
    AM,         // Amplitude modulation
    RingMod,    // Ring modulation
    Filter,     // First oscillator filtered by others
}
