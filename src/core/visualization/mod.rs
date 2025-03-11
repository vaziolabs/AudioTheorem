//! Visualization utilities for waveforms and audio analysis

use crate::core::oscillator::{Waveform, Oscillator, OscillatorCombinationMode, CustomWavetable};
use std::f32::consts::PI;

/// Generate a waveform preview for a single oscillator
pub fn generate_waveform_preview(
    waveform: &Waveform,
    detune: f32,
    octave: i8,
    custom_wavetables: &[CustomWavetable],
) -> Vec<[f32; 2]> {
    const PREVIEW_POINTS: usize = 100;
    let mut points = Vec::with_capacity(PREVIEW_POINTS);
    
    // Apply octave shift and detune to the phase
    let octave_factor = 2.0f32.powf(octave as f32);
    let detune_factor = 2.0f32.powf(detune / 12.0);
    let frequency_factor = octave_factor * detune_factor;
    
    for i in 0..PREVIEW_POINTS {
        let phase = i as f32 / PREVIEW_POINTS as f32;
        let mod_phase = (phase * frequency_factor) % 1.0;
        
        // Get waveform value based on the oscillator's waveform type
        let value = match waveform {
            Waveform::Sine => (2.0 * PI * mod_phase).sin(),
            Waveform::Square => if mod_phase < 0.5 { 1.0 } else { -1.0 },
            Waveform::Saw => 2.0 * mod_phase - 1.0,
            Waveform::Triangle => {
                if mod_phase < 0.25 {
                    4.0 * mod_phase
                } else if mod_phase < 0.75 {
                    2.0 - 4.0 * mod_phase
                } else {
                    -4.0 + 4.0 * mod_phase
                }
            },
            Waveform::WhiteNoise => {
                // For visualization, use a deterministic "random" function
                let seed = (i * 100) as f32;
                (seed.sin() * 12.5).sin()
            },
            Waveform::CustomSample(index) => {
                if let Some(wavetable) = custom_wavetables.get(*index) {
                    let sample_pos = mod_phase * wavetable.samples.len() as f32;
                    let index = sample_pos.floor() as usize % wavetable.samples.len();
                    wavetable.samples[index]
                } else {
                    0.0
                }
            }
        };
        
        points.push([phase, value]);
    }
    
    points
}

/// Generate a combined waveform display for all oscillators
pub fn generate_wavetable_display(
    oscillators: &[Oscillator; 3],
    oscillator_combination_mode: &OscillatorCombinationMode,
    custom_wavetables: &[CustomWavetable],
) -> Vec<[f32; 2]> {
    const DISPLAY_POINTS: usize = 200;
    let mut points = Vec::with_capacity(DISPLAY_POINTS);
    
    for i in 0..DISPLAY_POINTS {
        let phase = i as f32 / DISPLAY_POINTS as f32;
        
        // Get samples from each oscillator
        let mut osc_samples = [0.0; 3];
        
        for (i, oscillator) in oscillators.iter().enumerate() {
            if oscillator.volume > 0.0 {
                // Apply detune to the phase
                let detuned_phase = (phase * 2.0f32.powf(oscillator.detune / 12.0)) % 1.0;
                
                let osc_value = match &oscillator.waveform {
                    Waveform::Sine => (2.0 * PI * detuned_phase).sin(),
                    Waveform::Square => if detuned_phase < 0.5 { 1.0 } else { -1.0 },
                    Waveform::Saw => 2.0 * detuned_phase - 1.0,
                    Waveform::Triangle => {
                        if detuned_phase < 0.25 {
                            4.0 * detuned_phase
                        } else if detuned_phase < 0.75 {
                            2.0 - 4.0 * detuned_phase
                        } else {
                            -4.0 + 4.0 * detuned_phase
                        }
                    },
                    Waveform::WhiteNoise => {
                        // For noise, we'll use a pre-calculated random set for visualization
                        let seed = i as f32 / 10.0;
                        (seed.sin() * 12.5).sin()
                    },
                    Waveform::CustomSample(index) => {
                        if let Some(wavetable) = custom_wavetables.get(*index) {
                            let sample_pos = detuned_phase * wavetable.samples.len() as f32;
                            let index = sample_pos.floor() as usize % wavetable.samples.len();
                            wavetable.samples[index]
                        } else {
                            0.0
                        }
                    }
                };
                
                osc_samples[i] = osc_value * oscillator.volume;
            }
        }
        
        // Combine oscillator outputs based on the selected mode
        let value = crate::core::audio::combine_oscillators(&osc_samples, oscillators, oscillator_combination_mode, phase, 0.0);
        
        points.push([phase, value]);
    }
    
    points
}

/// Generate a spectrum display based on FFT analysis of the waveform
pub fn generate_spectrum_display(samples: &[f32], sample_rate: f32) -> Vec<[f32; 2]> {
    // For now, a very simplified spectrum display
    // In a real implementation, we would use FFT here
    const SPECTRUM_POINTS: usize = 128;
    let mut spectrum = Vec::with_capacity(SPECTRUM_POINTS);
    
    // Placeholder implementation
    for i in 0..SPECTRUM_POINTS {
        let freq = i as f32 / SPECTRUM_POINTS as f32 * sample_rate * 0.5;
        let magnitude = (i as f32 / SPECTRUM_POINTS as f32).powf(2.0) * 0.5;
        spectrum.push([freq, magnitude]);
    }
    
    spectrum
}
