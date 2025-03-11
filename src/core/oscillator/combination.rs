use serde::{Serialize, Deserialize};
use crate::core::oscillator::{Oscillator, Waveform};

// Add this enum to define different ways to combine oscillators
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum OscillatorCombinationMode {
    Parallel,   // Simple addition of all oscillators
    FM,         // Frequency modulation (osc1 modulates osc2, which modulates osc3)
    AM,         // Amplitude modulation
    RingMod,    // Ring modulation
    Filter,     // First oscillator filtered by others
}


pub fn combine_oscillators(
    osc_outputs: &[f32; 3],
    oscillators: &[Oscillator; 3],
    mode: &OscillatorCombinationMode,
    phase: f32,
) -> f32 {
    match mode {
        OscillatorCombinationMode::Parallel => osc_outputs[0] + osc_outputs[1] + osc_outputs[2],
        OscillatorCombinationMode::FM => {
            let mod_depth = 0.5;
            let mod_phase2 = (phase + osc_outputs[2] * mod_depth) % 1.0;
            let osc2_mod = match &oscillators[1].waveform {
                Waveform::Sine => (2.0 * std::f32::consts::PI * mod_phase2).sin(),
                Waveform::Square => if mod_phase2 < 0.5 { 1.0 } else { -1.0 },
                Waveform::Saw => 2.0 * mod_phase2 - 1.0,
                Waveform::Triangle => {
                    if mod_phase2 < 0.25 { 4.0 * mod_phase2 }
                    else if mod_phase2 < 0.75 { 2.0 - 4.0 * mod_phase2 }
                    else { -4.0 + 4.0 * mod_phase2 }
                },
                _ => osc_outputs[1]
            } * oscillators[1].volume;

            let mod_phase1 = (phase + osc2_mod * mod_depth) % 1.0;
            let osc1_mod = match &oscillators[0].waveform {
                Waveform::Sine => (2.0 * std::f32::consts::PI * mod_phase1).sin(),
                Waveform::Square => if mod_phase1 < 0.5 { 1.0 } else { -1.0 },
                Waveform::Saw => 2.0 * mod_phase1 - 1.0,
                Waveform::Triangle => {
                    if mod_phase1 < 0.25 { 4.0 * mod_phase1 }
                    else if mod_phase1 < 0.75 { 2.0 - 4.0 * mod_phase1 }
                    else { -4.0 + 4.0 * mod_phase1 }
                },
                _ => osc_outputs[0]
            } * oscillators[0].volume;

            osc1_mod
        },
        OscillatorCombinationMode::AM => (osc_outputs[0] * (1.0 + osc_outputs[1]) * (1.0 + osc_outputs[2])) * 0.5,
        OscillatorCombinationMode::RingMod => osc_outputs[0] * osc_outputs[1] * osc_outputs[2],
        OscillatorCombinationMode::Filter => {
            let filter_amount = (osc_outputs[1] + 1.0) * 0.5;
            let resonance = (osc_outputs[2] + 1.0) * 0.5;
            osc_outputs[0] * (1.0 - filter_amount) + osc_outputs[0].tanh() * filter_amount * (1.0 + resonance)
        },
    }
}
