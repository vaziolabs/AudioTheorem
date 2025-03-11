//! Core audio processing utilities

use crate::core::oscillator::{Oscillator, OscillatorCombinationMode};
use std::f32::consts::PI;

/// Combine multiple oscillator outputs based on the selected combination mode
pub fn combine_oscillators(
    osc_samples: &[f32; 3],
    oscillators: &[Oscillator; 3],
    oscillator_combination_mode: &OscillatorCombinationMode,
    phase: f32,
    sample_time: f32,
) -> f32 {
    match oscillator_combination_mode {
        OscillatorCombinationMode::Parallel => {
            // Simple addition of all oscillators
            osc_samples[0] + osc_samples[1] + osc_samples[2]
        },
        OscillatorCombinationMode::FM => {
            // Simplified FM approach
            let mod_depth = 0.5;
            
            // Apply osc3 to modulate osc2
            let mod_phase2 = (phase + osc_samples[2] * mod_depth) % 1.0;
            let osc2_mod = match &oscillators[1].waveform {
                crate::core::oscillator::Waveform::Sine => (2.0 * PI * mod_phase2).sin(),
                crate::core::oscillator::Waveform::Square => if mod_phase2 < 0.5 { 1.0 } else { -1.0 },
                crate::core::oscillator::Waveform::Saw => 2.0 * mod_phase2 - 1.0,
                crate::core::oscillator::Waveform::Triangle => {
                    if mod_phase2 < 0.25 {
                        4.0 * mod_phase2
                    } else if mod_phase2 < 0.75 {
                        2.0 - 4.0 * mod_phase2
                    } else {
                        -4.0 + 4.0 * mod_phase2
                    }
                },
                _ => osc_samples[1],
            } * oscillators[1].volume;
            
            // Apply osc2 to modulate osc1
            let mod_phase1 = (phase + osc2_mod * mod_depth) % 1.0;
            let osc1_mod = match &oscillators[0].waveform {
                crate::core::oscillator::Waveform::Sine => (2.0 * PI * mod_phase1).sin(),
                crate::core::oscillator::Waveform::Square => if mod_phase1 < 0.5 { 1.0 } else { -1.0 },
                crate::core::oscillator::Waveform::Saw => 2.0 * mod_phase1 - 1.0,
                crate::core::oscillator::Waveform::Triangle => {
                    if mod_phase1 < 0.25 {
                        4.0 * mod_phase1
                    } else if mod_phase1 < 0.75 {
                        2.0 - 4.0 * mod_phase1
                    } else {
                        -4.0 + 4.0 * mod_phase1
                    }
                },
                _ => osc_samples[0],
            } * oscillators[0].volume;
            
            osc1_mod
        },
        OscillatorCombinationMode::AM => {
            // Amplitude modulation
            let carrier = osc_samples[0];
            let modulator = (1.0 + osc_samples[1]) * (1.0 + osc_samples[2]);
            carrier * modulator * 0.5
        },
        OscillatorCombinationMode::RingMod => {
            // Ring modulation
            osc_samples[0] * osc_samples[1] * osc_samples[2]
        },
        OscillatorCombinationMode::Filter => {
            // Simple filter effect
            let source = osc_samples[0];
            let filter_amount = (osc_samples[1] + 1.0) * 0.5;
            let resonance = (osc_samples[2] + 1.0) * 0.5;
            
            source * (1.0 - filter_amount) + source.tanh() * filter_amount * (1.0 + resonance)
        },
    }
}
