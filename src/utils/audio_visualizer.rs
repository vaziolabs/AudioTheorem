// Audio Analysis / Visualization

use crate::core::oscillator::{Oscillator, Waveform, OscillatorCombinationMode, CustomWavetable};
use rand;
use std::f32::consts::PI;

/// Generate a waveform preview for a single oscillator
pub fn generate_waveform_preview(waveform: &Waveform, custom_wavetables: &[CustomWavetable], samples: usize) -> Vec<[f32; 2]> {
    let mut result = Vec::with_capacity(samples);
    
    for i in 0..samples {
        let phase = i as f32 / samples as f32;
        let value = match waveform {
            Waveform::Sine => (2.0 * PI * phase).sin(),
            Waveform::Square => if phase < 0.5 { 1.0 } else { -1.0 },
            Waveform::Saw => 2.0 * phase - 1.0,
            Waveform::Triangle => {
                if phase < 0.25 {
                    4.0 * phase
                } else if phase < 0.75 {
                    2.0 - 4.0 * phase
                } else {
                    -4.0 + 4.0 * phase
                }
            },
            Waveform::WhiteNoise => {
                rand::random::<f32>() * 2.0 - 1.0
            },
            Waveform::CustomSample(index) => {
                if let Some(wavetable) = custom_wavetables.get(*index) {
                    let sample_pos = phase * wavetable.samples.len() as f32;
                    let index = sample_pos.floor() as usize % wavetable.samples.len();
                    wavetable.samples[index]
                } else {
                    0.0
                }
            },
        };
        
        result.push([phase, value]);
    }
    
    result
}

/// Generate a waveform display that combines multiple oscillators
pub fn generate_combined_waveform(oscillators: &[Oscillator], combination_mode: &OscillatorCombinationMode, custom_wavetables: &[CustomWavetable]) -> Vec<[f32; 2]> {
    const DISPLAY_POINTS: usize = 200;
    let mut points = Vec::with_capacity(DISPLAY_POINTS);
    
    // First, gather individual oscillator waveforms
    let mut osc_samples = Vec::with_capacity(oscillators.len());
    
    for oscillator in oscillators {
        let mut samples = Vec::with_capacity(DISPLAY_POINTS);
        for i in 0..DISPLAY_POINTS {
            let phase = i as f32 / DISPLAY_POINTS as f32;
            
            // Apply oscillator settings
            let octave_factor = 2.0f32.powf(oscillator.octave as f32);
            let detune_factor = 2.0f32.powf(oscillator.detune / 12.0);
            let frequency_factor = octave_factor * detune_factor;
            let mod_phase = (phase * frequency_factor) % 1.0;
            
            // Get sample
            let value = match &oscillator.waveform {
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
                    // Deterministic "random" for visualization
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
            
            samples.push(value * oscillator.volume);
        }
        osc_samples.push(samples);
    }
    
    // Combine waveforms according to the selected mode
    for i in 0..DISPLAY_POINTS {
        let phase = i as f32 / DISPLAY_POINTS as f32;
        
        // Get samples from all oscillators at this position
        let combined_value = match combination_mode {
            OscillatorCombinationMode::Parallel => {
                // Simple addition of all oscillators
                osc_samples.iter().map(|samples| samples[i]).sum()
            },
            OscillatorCombinationMode::FM => {
                // Simplified FM for visualization
                let mod_depth = 0.5;
                
                if osc_samples.len() < 3 {
                    0.0 // Need at least 3 oscillators for proper FM
                } else {
                    // Osc3 modulates Osc2, which modulates Osc1
                    let mod_phase2 = (phase + osc_samples[2][i] * mod_depth) % 1.0;
                    let osc2_mod = match &oscillators[1].waveform {
                        Waveform::Sine => (2.0 * PI * mod_phase2).sin(),
                        Waveform::Square => if mod_phase2 < 0.5 { 1.0 } else { -1.0 },
                        Waveform::Saw => 2.0 * mod_phase2 - 1.0,
                        Waveform::Triangle => {
                            if mod_phase2 < 0.25 {
                                4.0 * mod_phase2
                            } else if mod_phase2 < 0.75 {
                                2.0 - 4.0 * mod_phase2
                            } else {
                                -4.0 + 4.0 * mod_phase2
                            }
                        },
                        _ => osc_samples[1][i],
                    } * oscillators[1].volume;
                    
                    let mod_phase1 = (phase + osc2_mod * mod_depth) % 1.0;
                    (match &oscillators[0].waveform {
                        Waveform::Sine => (2.0 * PI * mod_phase1).sin(),
                        Waveform::Square => if mod_phase1 < 0.5 { 1.0 } else { -1.0 },
                        Waveform::Saw => 2.0 * mod_phase1 - 1.0,
                        Waveform::Triangle => {
                            if mod_phase1 < 0.25 {
                                4.0 * mod_phase1
                            } else if mod_phase1 < 0.75 {
                                2.0 - 4.0 * mod_phase1
                            } else {
                                -4.0 + 4.0 * mod_phase1
                            }
                        },
                        _ => osc_samples[0][i],
                    }) * oscillators[0].volume
                }
            },
            OscillatorCombinationMode::AM => {
                // Amplitude modulation (simplistic)
                if osc_samples.len() < 2 {
                    osc_samples[0][i]
                } else {
                    let carrier = osc_samples[0][i];
                    let modulator = osc_samples[1][i] * 0.5 + 0.5; // Normalize to 0-1
                    carrier * modulator
                }
            },
            OscillatorCombinationMode::RingMod => {
                // Ring modulation (carrier * modulator)
                if osc_samples.len() < 2 {
                    osc_samples[0][i]
                } else {
                    osc_samples[0][i] * osc_samples[1][i]
                }
            },
            OscillatorCombinationMode::Filter => {
                // Very simplified filter visualization
                if osc_samples.len() < 2 {
                    osc_samples[0][i]
                } else {
                    let source = osc_samples[0][i];
                    let filter_mod = (osc_samples[1][i] + 1.0) * 0.5; // 0.0-1.0
                    
                    // Very basic lowpass-like effect for visualization
                    source * (filter_mod + 0.2)
                }
            }
        };
        
        // Normalize to prevent clipping in visualization
        let normalized_value = if combined_value.abs() > 1.0 {
            combined_value / combined_value.abs()
        } else {
            combined_value
        };
        
        points.push([phase, normalized_value]);
    }
    
    points
}

/// Generate a spectrum display based on FFT analysis of the waveform
pub fn generate_spectrum_display(_samples: &[f32], sample_rate: f32) -> Vec<[f32; 2]> {
    // For now, return a placeholder spectrum
    let mut result = Vec::with_capacity(64);
    
    for i in 0..64 {
        let freq = i as f32 * (sample_rate / 128.0);
        let amp = (i as f32 / 64.0) * (1.0 - i as f32 / 64.0) * 2.0;
        result.push([freq, amp]);
    }
    
    result
}

pub fn combine_oscillators(
    oscillators: &[Oscillator; 3],
    _mode: &OscillatorCombinationMode, // TODO: Implement this
    samples: usize
) -> Vec<f32> {
    let mut result = vec![0.0; samples];
    let mut osc_samples = vec![vec![0.0; samples]; 3];
    
    // Generate samples for each oscillator
    for osc_idx in 0..3 {
        for i in 0..samples {
            let phase = i as f32 / samples as f32;
            osc_samples[osc_idx][i] = match &oscillators[osc_idx].waveform {
                Waveform::Sine => (2.0 * PI * phase).sin(),
                Waveform::Square => if phase < 0.5 { 1.0 } else { -1.0 },
                Waveform::Saw => 2.0 * phase - 1.0,
                Waveform::Triangle => {
                    if phase < 0.25 {
                        4.0 * phase
                    } else if phase < 0.75 {
                        2.0 - 4.0 * phase
                    } else {
                        -4.0 + 4.0 * phase
                    }
                },
                _ => 0.0,
            };
        }
    }
    
    // Combine based on mode
    for i in 0..samples {
        let mod_phase1 = i as f32 / samples as f32;
        
        // Fix the parenthesized expression
        let value = (match &oscillators[0].waveform {
            Waveform::Sine => (2.0 * PI * mod_phase1).sin(),
            Waveform::Square => if mod_phase1 < 0.5 { 1.0 } else { -1.0 },
            Waveform::Saw => 2.0 * mod_phase1 - 1.0,
            Waveform::Triangle => {
                if mod_phase1 < 0.25 {
                    4.0 * mod_phase1
                } else if mod_phase1 < 0.75 {
                    2.0 - 4.0 * mod_phase1
                } else {
                    -4.0 + 4.0 * mod_phase1
                }
            },
            _ => osc_samples[0][i],
        }) * oscillators[0].volume;
        
        result[i] = value;
    }
    
    result
}

/// Generate a waveform display for visualization in the master panel
pub fn generate_wavetable_display(
    oscillators: &[Oscillator],
    combination_mode: &OscillatorCombinationMode,
    custom_wavetables: &[CustomWavetable]
) -> Vec<[f32; 2]> {
    const DISPLAY_POINTS: usize = 200;
    let mut points = Vec::with_capacity(DISPLAY_POINTS);
    
    for i in 0..DISPLAY_POINTS {
        let phase = i as f32 / DISPLAY_POINTS as f32;
        
        // Get samples from each oscillator
        let mut osc_samples = [0.0; 3];
        
        for (idx, oscillator) in oscillators.iter().enumerate() {
            if oscillator.volume > 0.0 {
                // Apply detune and octave to the phase
                let octave_factor = 2.0f32.powf(oscillator.octave as f32);
                let detune_factor = 2.0f32.powf(oscillator.detune / 12.0);
                let frequency_factor = octave_factor * detune_factor;
                let detuned_phase = (phase * frequency_factor) % 1.0;
                
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
                        // For visualization, use deterministic noise
                        let seed = (i * 100 + idx * 10) as f32;
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
                
                osc_samples[idx] = osc_value * oscillator.volume;
            }
        }
        
        // Combine oscillator outputs based on the selected mode
        let value = match combination_mode {
            OscillatorCombinationMode::Parallel => {
                osc_samples[0] + osc_samples[1] + osc_samples[2]
            },
            OscillatorCombinationMode::FM => {
                // Simplified FM for visualization
                let mod_depth = 0.5;
                
                if osc_samples[1] == 0.0 && osc_samples[2] == 0.0 {
                    osc_samples[0] // Only using first oscillator
                } else {
                    // Simple FM chain: osc3 modulates osc2, which modulates osc1
                    let mod_phase2 = (phase + osc_samples[2] * mod_depth) % 1.0;
                    let osc2_mod = match &oscillators[1].waveform {
                        Waveform::Sine => (2.0 * PI * mod_phase2).sin(),
                        Waveform::Square => if mod_phase2 < 0.5 { 1.0 } else { -1.0 },
                        Waveform::Saw => 2.0 * mod_phase2 - 1.0,
                        Waveform::Triangle => {
                            if mod_phase2 < 0.25 {
                                4.0 * mod_phase2
                            } else if mod_phase2 < 0.75 {
                                2.0 - 4.0 * mod_phase2
                            } else {
                                -4.0 + 4.0 * mod_phase2
                            }
                        },
                        _ => 0.0
                    } * oscillators[1].volume;
                    
                    let mod_phase1 = (phase + osc2_mod * mod_depth) % 1.0;
                    let result = match &oscillators[0].waveform {
                        Waveform::Sine => (2.0 * PI * mod_phase1).sin(),
                        Waveform::Square => if mod_phase1 < 0.5 { 1.0 } else { -1.0 },
                        Waveform::Saw => 2.0 * mod_phase1 - 1.0,
                        Waveform::Triangle => {
                            if mod_phase1 < 0.25 {
                                4.0 * mod_phase1
                            } else if mod_phase1 < 0.75 {
                                2.0 - 4.0 * mod_phase1
                            } else {
                                -4.0 + 4.0 * mod_phase1
                            }
                        },
                        _ => 0.0
                    } * oscillators[0].volume;
                    
                    result
                }
            },
            OscillatorCombinationMode::AM => {
                // Amplitude modulation
                if osc_samples[1] == 0.0 {
                    osc_samples[0]
                } else {
                    let carrier = osc_samples[0];
                    let modulator = (osc_samples[1] + 1.0) * 0.5; // 0-1 range
                    carrier * modulator
                }
            },
            OscillatorCombinationMode::RingMod => {
                // Ring modulation
                if osc_samples[1] == 0.0 {
                    osc_samples[0]
                } else {
                    osc_samples[0] * osc_samples[1]
                }
            },
            OscillatorCombinationMode::Filter => {
                // Simplified filter visualization
                if osc_samples[1] == 0.0 {
                    osc_samples[0]
                } else {
                    let source = osc_samples[0];
                    let filter_mod = (osc_samples[1] + 1.0) * 0.5; // 0-1 range
                    source * (filter_mod + 0.2)
                }
            }
        };
        
        // Normalize to prevent clipping in visualization
        let normalized_value = if value.abs() > 1.0 {
            value / value.abs()
        } else {
            value
        };
        
        points.push([phase, normalized_value]);
    }
    
    points
}