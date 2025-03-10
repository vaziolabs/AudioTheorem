use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Sample, SampleFormat, Stream};
use eframe::egui;
use egui_plot::{Plot, PlotPoints, Line};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::Instant;
use hound; // For WAV file loading
use rfd::FileDialog; // For file dialogs
use std::f32::consts::PI;
use serde::{Serialize, Deserialize};
use std::fs::{self, File};
use std::io::Write;
use serde_json;
use dirs;

// Constants
const SAMPLE_BUFFER_SIZE: usize = 1024;
const WAVEFORM_DISPLAY_POINTS: usize = 200;
const MAX_CUSTOM_WAVETABLES: usize = 8;

// Types of waveforms
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
enum Waveform {
    Sine,
    Square,
    Saw,
    Triangle,
    WhiteNoise,
    CustomSample(usize), // Index into the custom wavetables
}

// Note state for envelope
#[derive(Debug, Clone, Copy, PartialEq)]
enum NoteState {
    Attack,
    Decay,
    Sustain,
    Release,
}

// A custom wavetable loaded from a sample
struct CustomWavetable {
    name: String,
    samples: Vec<f32>,
    sample_rate: u32,
}

// Structure representing a single note
#[derive(Debug, Clone)]
struct Note {
    midi_note: u8,
    frequency: f32,
    phase: f32,
    phase_increment: f32,
    velocity: f32,
    state: NoteState,
    time_in_state: f32,
}

// Current analyzer state
struct Analyzer {
    current_waveform_samples: VecDeque<f32>,
    fft_buffer: Vec<f32>,
    last_update: Instant,
}

// Expand the Oscillator struct with additional parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Oscillator {
    waveform: Waveform,
    volume: f32,
    detune: f32,       // Detune in semitones
    octave: i8,        // Octave shift (-4 to +4)
    
    // Per-oscillator envelope
    attack: f32,
    decay: f32,
    sustain: f32,
    release: f32,
    
    // Per-oscillator filter
    filter_type: FilterType,
    filter_cutoff: f32,  // 0.0 to 1.0 (normalized frequency)
    filter_resonance: f32, // 0.0 to 1.0
    
    // Modulation
    mod_amount: f32,    // 0.0 to 1.0
    mod_target: ModulationTarget,
}

// Define filter types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
enum FilterType {
    None,
    LowPass,
    HighPass,
    BandPass,
}

// Define modulation targets
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
enum ModulationTarget {
    None,
    Pitch,
    FilterCutoff,
    Volume,
    PulseWidth,
}

// Add this enum to define different ways to combine oscillators
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
enum OscillatorCombinationMode {
    Parallel,   // Simple addition of all oscillators
    FM,         // Frequency modulation (osc1 modulates osc2, which modulates osc3)
    AM,         // Amplitude modulation
    RingMod,    // Ring modulation
    Filter,     // First oscillator filtered by others
}

// Main synthesizer state
struct Synth {
    sample_rate: f32,
    volume: f32,
    active_notes: Vec<Note>,
    oscillators: [Oscillator; 3], // Three oscillators
    oscillator_combination_mode: OscillatorCombinationMode,
    
    // Master envelope
    attack: f32,
    decay: f32,
    sustain: f32,
    release: f32,
    
    // Master filter
    master_filter_type: FilterType,
    master_filter_cutoff: f32,
    master_filter_resonance: f32,
    
    custom_wavetables: Vec<CustomWavetable>,
    analyzer: Analyzer,
    
    // Filter state variables (for each oscillator and master)
    filter_states: [[f32; 4]; 4], // 3 oscillators + 1 master, 4 states per filter
}

impl Synth {
    fn new(sample_rate: f32) -> Self {
        Synth {
            sample_rate,
            volume: 0.5,
            active_notes: Vec::new(),
            oscillators: [
                Oscillator {
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
                },
                Oscillator {
                    waveform: Waveform::Sine,
                    volume: 0.0,
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
                },
                Oscillator {
                    waveform: Waveform::Sine,
                    volume: 0.0,
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
                },
            ],
            oscillator_combination_mode: OscillatorCombinationMode::Parallel,
            attack: 0.1,
            decay: 0.2,
            sustain: 0.7,
            release: 0.3,
            master_filter_type: FilterType::None,
            master_filter_cutoff: 1.0,
            master_filter_resonance: 0.0,
            custom_wavetables: Vec::new(),
            analyzer: Analyzer {
                current_waveform_samples: VecDeque::with_capacity(SAMPLE_BUFFER_SIZE),
                fft_buffer: vec![0.0; SAMPLE_BUFFER_SIZE],
                last_update: Instant::now(),
            },
            filter_states: [[0.0; 4]; 4],
        }
    }

    fn note_on(&mut self, midi_note: u8, velocity: u8) {
        let freq = midi_note_to_freq(midi_note);
        let vel = velocity as f32 / 127.0;
        
        // Remove any existing instances of this note
        self.active_notes.retain(|n| n.midi_note != midi_note);
        
        // Calculate phase increment based on frequency and sample rate
        let phase_increment = freq / self.sample_rate;
        
        self.active_notes.push(Note {
            midi_note,
            frequency: freq,
            phase: 0.0,
            phase_increment,
            velocity: vel,
            state: NoteState::Attack,
            time_in_state: 0.0,
        });
    }

    fn note_off(&mut self, midi_note: u8) {
        for note in self.active_notes.iter_mut() {
            if note.midi_note == midi_note {
                note.state = NoteState::Release;
                note.time_in_state = 0.0;
            }
        }
    }

    fn load_sample(&mut self, path: PathBuf) -> Result<()> {
        // Use hound to read WAV file
        let reader = hound::WavReader::open(&path)?;
        let spec = reader.spec();
        let samples: Vec<f32> = match spec.sample_format {
            hound::SampleFormat::Float => {
                reader.into_samples::<f32>().filter_map(Result::ok).collect()
            },
            hound::SampleFormat::Int => {
                reader.into_samples::<i32>()
                    .filter_map(Result::ok)
                    .map(|s| s as f32 / i32::MAX as f32)
                    .collect()
            }
        };

        // Get filename for display
        let filename = path.file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| "Unknown Sample".to_string());

        // Normalize samples to -1.0 to 1.0 range
        let max_amplitude = samples.iter().fold(0.0f32, |max, &s| max.max(s.abs()));
        let normalized_samples: Vec<f32> = if max_amplitude > 0.0 {
            samples.iter().map(|s| s / max_amplitude).collect()
        } else {
            samples
        };

        // Add to wavetables
        if self.custom_wavetables.len() >= MAX_CUSTOM_WAVETABLES {
            self.custom_wavetables.remove(0); // Remove oldest if we're at the limit
        }
        
        self.custom_wavetables.push(CustomWavetable {
            name: filename,
            samples: normalized_samples,
            sample_rate: spec.sample_rate,
        });

        Ok(())
    }

    // Fix the apply_filter method to avoid mutable borrow issues
    fn apply_filter(&self, sample: f32, filter_type: &FilterType, cutoff: f32, resonance: f32, _filter_index: usize) -> f32 {
        // Skip processing if no filter is selected
        if *filter_type == FilterType::None {
            return sample;
        }
        
        // Normalize cutoff to 0.0-1.0 range
        let cutoff = cutoff.clamp(0.01, 0.99);
        
        // Convert resonance to Q factor (0.7 to 20.0)
        let q = 0.7 + resonance * 19.3;
        
        // Calculate filter coefficients (simplified biquad filter)
        let omega = 2.0 * std::f32::consts::PI * cutoff / self.sample_rate;
        let sin_omega = omega.sin();
        let _cos_omega = omega.cos();
        let _alpha = sin_omega / (2.0 * q);
        
        // Since we can't modify filter states here, we'll use a simplified approach
        // This is a non-stateful approximation of the filter
        match filter_type {
            FilterType::LowPass => {
                // Simple lowpass approximation
                let cutoff_factor = cutoff.powf(0.5);
                sample * cutoff_factor
            },
            FilterType::HighPass => {
                // Simple highpass approximation
                let cutoff_factor = 1.0 - cutoff.powf(0.5);
                sample * cutoff_factor
            },
            FilterType::BandPass => {
                // Simple bandpass approximation
                let band_factor = 1.0 - (cutoff - 0.5).abs() * 2.0;
                sample * band_factor
            },
            FilterType::None => sample,
        }
    }

    // Fix the get_sample method to avoid multiple mutable borrows
    fn get_sample(&mut self, _sample_time: f32) -> f32 {
        // Process all active notes and collect their outputs
        let mut note_outputs = Vec::with_capacity(self.active_notes.len());
        let mut notes_to_remove = Vec::new();
        
        // Store oscillator data to avoid multiple borrows
        let oscillators = self.oscillators.clone();
        let osc_combination_mode = self.oscillator_combination_mode.clone();
        let sample_rate = self.sample_rate;
        let attack = self.attack;
        let decay = self.decay;
        let sustain = self.sustain;
        let release = self.release;
        
        // First pass: process all notes and collect their data
        for (i, note) in self.active_notes.iter_mut().enumerate() {
            // Update phase for this note
            note.phase = (note.phase + note.phase_increment) % 1.0;
            
            // Calculate master envelope
            let master_envelope = match note.state {
                NoteState::Attack => {
                    note.time_in_state += 1.0 / sample_rate;
                    let value = note.time_in_state / attack;
                    if value >= 1.0 {
                        note.state = NoteState::Decay;
                        note.time_in_state = 0.0;
                        1.0
                    } else {
                        value
                    }
                },
                NoteState::Decay => {
                    note.time_in_state += 1.0 / sample_rate;
                    let value = 1.0 - (1.0 - sustain) * (note.time_in_state / decay);
                    if value <= sustain || note.time_in_state >= decay {
                        note.state = NoteState::Sustain;
                        note.time_in_state = 0.0;
                        sustain
                    } else {
                        value
                    }
                },
                NoteState::Sustain => sustain,
                NoteState::Release => {
                    note.time_in_state += 1.0 / sample_rate;
                    let value = sustain * (1.0 - note.time_in_state / release);
                    if value <= 0.0 || note.time_in_state >= release {
                        notes_to_remove.push(i);
                        0.0
                    } else {
                        value
                    }
                },
            };
            
            // Process each oscillator for this note
            let mut osc_outputs = [0.0; 3];
            
            for (osc_idx, oscillator) in oscillators.iter().enumerate() {
                if oscillator.volume > 0.0 {
                    // Apply octave shift and detune to the frequency
                    let octave_factor = 2.0f32.powf(oscillator.octave as f32);
                    let detune_factor = 2.0f32.powf(oscillator.detune / 12.0);
                    let frequency_factor = octave_factor * detune_factor;
                    
                    // Calculate modulated phase
                    let mod_phase = match oscillator.mod_target {
                        ModulationTarget::Pitch => {
                            // Apply pitch modulation (simple LFO)
                            let lfo = (_sample_time * 5.0).sin() * oscillator.mod_amount;
                            (note.phase * frequency_factor * (1.0 + lfo)) % 1.0
                        },
                        _ => (note.phase * frequency_factor) % 1.0
                    };
                    
                    // Get waveform value based on the oscillator's waveform type
                    let waveform_value = match &oscillator.waveform {
                        Waveform::Sine => (2.0 * std::f32::consts::PI * mod_phase).sin(),
                        Waveform::Square => {
                            // Apply pulse width modulation if selected
                            let pulse_width = match oscillator.mod_target {
                                ModulationTarget::PulseWidth => {
                                    // Modulate pulse width between 0.1 and 0.9
                                    let lfo = (_sample_time * 3.0).sin() * oscillator.mod_amount;
                                    0.5 + lfo * 0.4
                                },
                                _ => 0.5
                            };
                            if mod_phase < pulse_width { 1.0 } else { -1.0 }
                        },
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
                        Waveform::WhiteNoise => rand::random::<f32>() * 2.0 - 1.0,
                        Waveform::CustomSample(index) => {
                            if let Some(wavetable) = self.custom_wavetables.get(*index) {
                                // Sample from the wavetable
                                let position = mod_phase * wavetable.samples.len() as f32;
                                let index = position.floor() as usize % wavetable.samples.len();
                                let next_index = (index + 1) % wavetable.samples.len();
                                let fraction = position - position.floor();
                                
                                // Linear interpolation between samples
                                wavetable.samples[index] * (1.0 - fraction) + 
                                wavetable.samples[next_index] * fraction
                            } else {
                                0.0
                            }
                        }
                    };
                    
                    // Apply oscillator-specific envelope
                    let osc_envelope = match note.state {
                        NoteState::Attack => {
                            let value = note.time_in_state / oscillator.attack;
                            if value >= 1.0 { 1.0 } else { value }
                        },
                        NoteState::Decay => {
                            1.0 - (1.0 - oscillator.sustain) * (note.time_in_state / oscillator.decay)
                        },
                        NoteState::Sustain => oscillator.sustain,
                        NoteState::Release => {
                            oscillator.sustain * (1.0 - note.time_in_state / oscillator.release)
                        },
                    };
                    
                    // Apply volume modulation if selected
                    let volume_mod = match oscillator.mod_target {
                        ModulationTarget::Volume => {
                            // Tremolo effect
                            1.0 + (_sample_time * 6.0).sin() * oscillator.mod_amount
                        },
                        _ => 1.0
                    };
                    
                    // Apply filter modulation if selected
                    let filter_cutoff_mod = match oscillator.mod_target {
                        ModulationTarget::FilterCutoff => {
                            // Filter cutoff modulation
                            oscillator.filter_cutoff * (1.0 + (_sample_time * 4.0).sin() * oscillator.mod_amount)
                        },
                        _ => oscillator.filter_cutoff
                    };
                    
                    // Apply oscillator's filter (simplified version without state)
                    let filtered_sample = match oscillator.filter_type {
                        FilterType::LowPass => {
                            let cutoff = filter_cutoff_mod.clamp(0.01, 0.99);
                            waveform_value * cutoff.powf(0.5)
                        },
                        FilterType::HighPass => {
                            let cutoff = filter_cutoff_mod.clamp(0.01, 0.99);
                            waveform_value * (1.0 - cutoff.powf(0.5))
                        },
                        FilterType::BandPass => {
                            let cutoff = filter_cutoff_mod.clamp(0.01, 0.99);
                            let band_factor = 1.0 - (cutoff - 0.5).abs() * 2.0;
                            waveform_value * band_factor
                        },
                        FilterType::None => waveform_value,
                    };
                    
                    osc_outputs[osc_idx] = filtered_sample * oscillator.volume * volume_mod * osc_envelope;
                }
            }
            
            // Combine oscillator outputs based on the selected mode
            let osc_sample = match osc_combination_mode {
                OscillatorCombinationMode::Parallel => {
                    // Simple addition of all oscillators
                    osc_outputs[0] + osc_outputs[1] + osc_outputs[2]
                },
                OscillatorCombinationMode::FM => {
                    // Simplified FM approach
                    let mod_depth = 0.5;
                    
                    // Apply osc3 to modulate osc2
                    let mod_phase2 = (note.phase + osc_outputs[2] * mod_depth) % 1.0;
                    let osc2_mod = match &oscillators[1].waveform {
                        Waveform::Sine => (2.0 * std::f32::consts::PI * mod_phase2).sin(),
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
                        _ => osc_outputs[1],
                    } * oscillators[1].volume;
                    
                    // Apply osc2 to modulate osc1
                    let mod_phase1 = (note.phase + osc2_mod * mod_depth) % 1.0;
                    let osc1_mod = match &oscillators[0].waveform {
                        Waveform::Sine => (2.0 * std::f32::consts::PI * mod_phase1).sin(),
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
                        _ => osc_outputs[0],
                    } * oscillators[0].volume;
                    
                    osc1_mod
                },
                OscillatorCombinationMode::AM => {
                    // Amplitude modulation
                    let carrier = osc_outputs[0];
                    let modulator = (1.0 + osc_outputs[1]) * (1.0 + osc_outputs[2]);
                    carrier * modulator * 0.5
                },
                OscillatorCombinationMode::RingMod => {
                    // Ring modulation
                    osc_outputs[0] * osc_outputs[1] * osc_outputs[2]
                },
                OscillatorCombinationMode::Filter => {
                    // Simple filter effect
                    let source = osc_outputs[0];
                    let filter_amount = (osc_outputs[1] + 1.0) * 0.5;
                    let resonance = (osc_outputs[2] + 1.0) * 0.5;
                    
                    source * (1.0 - filter_amount) + source.tanh() * filter_amount * (1.0 + resonance)
                },
            };
            
            note_outputs.push(osc_sample * master_envelope * note.velocity);
        }
        
        // Sum all note outputs
        let mut sample = 0.0;
        for output in note_outputs {
            sample += output;
        }
        
        // Remove finished notes
        for i in notes_to_remove.iter().rev() {
            self.active_notes.remove(*i);
        }
        
        // Apply master filter
        let filtered_sample = self.apply_filter(
            sample,
            &self.master_filter_type,
            self.master_filter_cutoff,
            self.master_filter_resonance,
            3 // Use index 3 for master filter
        );
        
        // Apply master volume
        let final_sample = filtered_sample * self.volume;
        
        // Update our analyzer with this sample
        if self.analyzer.current_waveform_samples.len() >= SAMPLE_BUFFER_SIZE {
            self.analyzer.current_waveform_samples.pop_front();
        }
        self.analyzer.current_waveform_samples.push_back(final_sample);
        
        final_sample
    }

    // Generate waveform visualization data
    fn generate_waveform_display(&self) -> Vec<[f32; 2]> {
        let mut points = Vec::with_capacity(WAVEFORM_DISPLAY_POINTS);
        let samples = &self.analyzer.current_waveform_samples;
        
        if samples.is_empty() {
            // Generate a flat line if no samples
            for i in 0..WAVEFORM_DISPLAY_POINTS {
                points.push([i as f32 / WAVEFORM_DISPLAY_POINTS as f32, 0.0]);
            }
            return points;
        }
        
        // Sample from our buffer to create the display points
        let step = samples.len() as f32 / WAVEFORM_DISPLAY_POINTS as f32;
        for i in 0..WAVEFORM_DISPLAY_POINTS {
            let pos = (i as f32 * step) as usize;
            if pos < samples.len() {
                points.push([i as f32 / WAVEFORM_DISPLAY_POINTS as f32, samples[pos]]);
            }
        }
        
        points
    }

    // Generate a visualization of the current waveform table
    fn generate_wavetable_display(&self) -> Vec<[f32; 2]> {
        let mut points = Vec::with_capacity(WAVEFORM_DISPLAY_POINTS);
        
        for i in 0..WAVEFORM_DISPLAY_POINTS {
            let phase = i as f32 / WAVEFORM_DISPLAY_POINTS as f32;
            
            // Get samples from each oscillator
            let mut osc_samples = [0.0; 3];
            
            for (i, oscillator) in self.oscillators.iter().enumerate() {
                if oscillator.volume > 0.0 {
                    // Apply detune to the phase
                    let detuned_phase = (phase * 2.0f32.powf(oscillator.detune / 12.0)) % 1.0;
                    
                    let osc_value = match &oscillator.waveform {
                        Waveform::Sine => (2.0 * std::f32::consts::PI * detuned_phase).sin(),
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
                            if let Some(wavetable) = self.custom_wavetables.get(*index) {
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
            let value = match self.oscillator_combination_mode {
                OscillatorCombinationMode::Parallel => {
                    osc_samples[0] + osc_samples[1] + osc_samples[2]
                },
                OscillatorCombinationMode::FM => {
                    // Simplified FM for visualization
                    let mod_depth = 0.5;
                    
                    let mod_phase2 = (phase + osc_samples[2] * mod_depth) % 1.0;
                    let osc2_mod = match &self.oscillators[1].waveform {
                        Waveform::Sine => (2.0 * std::f32::consts::PI * mod_phase2).sin(),
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
                        _ => osc_samples[1],
                    } * self.oscillators[1].volume;
                    
                    let mod_phase1 = (phase + osc2_mod * mod_depth) % 1.0;
                    let osc1_mod = match &self.oscillators[0].waveform {
                        Waveform::Sine => (2.0 * std::f32::consts::PI * mod_phase1).sin(),
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
                        _ => osc_samples[0],
                    } * self.oscillators[0].volume;
                    
                    osc1_mod
                },
                OscillatorCombinationMode::AM => {
                    let carrier = osc_samples[0];
                    let modulator = (1.0 + osc_samples[1]) * (1.0 + osc_samples[2]);
                    carrier * modulator * 0.5
                },
                OscillatorCombinationMode::RingMod => {
                    osc_samples[0] * osc_samples[1] * osc_samples[2]
                },
                OscillatorCombinationMode::Filter => {
                    let source = osc_samples[0];
                    let filter_amount = (osc_samples[1] + 1.0) * 0.5;
                    let resonance = (osc_samples[2] + 1.0) * 0.5;
                    
                    source * (1.0 - filter_amount) + source.tanh() * filter_amount * (1.0 + resonance)
                },
            };
            
            points.push([phase, value]);
        }
        
        points
    }

    // Generate a 2D representation of the wavetable for different pitches
    fn generate_pitch_wavetable(&self) -> Vec<Vec<[f64; 2]>> {
        const PITCH_CLASSES: usize = 12;
        const SAMPLES_PER_CYCLE: usize = 64;
        
        let mut pitch_lines = Vec::with_capacity(PITCH_CLASSES);
        
        for pitch_class in 0..PITCH_CLASSES {
            let mut points = Vec::with_capacity(SAMPLES_PER_CYCLE);
            
            // Calculate the MIDI note number (middle C = 60)
            let midi_note = 60 + pitch_class;
            
            // Generate one cycle of the waveform for this pitch
            for i in 0..SAMPLES_PER_CYCLE {
                let phase = i as f32 / SAMPLES_PER_CYCLE as f32;
                
                // Get the waveform value based on the current waveform type
                let value = match &self.oscillators[0].waveform {
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
                        // Use a deterministic "random" function for visualization
                        let seed = (pitch_class * 100 + i) as f32;
                        (seed.sin() * 12.5).sin()
                    },
                    Waveform::CustomSample(index) => {
                        if let Some(wavetable) = self.custom_wavetables.get(*index) {
                            let sample_pos = phase * wavetable.samples.len() as f32;
                            let index = sample_pos.floor() as usize % wavetable.samples.len();
                            wavetable.samples[index]
                        } else {
                            0.0
                        }
                    }
                };
                
                // Apply any active note modulation if this pitch is being played
                let modulated_value = if let Some(note) = self.active_notes.iter()
                    .find(|n| n.midi_note as usize == midi_note) {
                    // Apply envelope modulation
                    let envelope = match note.state {
                        NoteState::Attack => note.time_in_state / self.attack,
                        NoteState::Decay => 1.0 - (1.0 - self.sustain) * (note.time_in_state / self.decay),
                        NoteState::Sustain => self.sustain,
                        NoteState::Release => self.sustain * (1.0 - note.time_in_state / self.release),
                    };
                    value * envelope * note.velocity
                } else {
                    value * 0.3 // Lower amplitude for non-playing notes
                };
                
                // Add the point to our line
                points.push([i as f64 / SAMPLES_PER_CYCLE as f64, modulated_value as f64]);
            }
            
            pitch_lines.push(points);
        }
        
        pitch_lines
    }

    // Add a method to create a preset from current settings
    fn create_preset(&self, name: String) -> SynthPreset {
        SynthPreset {
            name,
            oscillators: self.oscillators.clone(),
            oscillator_combination_mode: self.oscillator_combination_mode.clone(),
            attack: self.attack,
            decay: self.decay,
            sustain: self.sustain,
            release: self.release,
            master_filter_type: self.master_filter_type.clone(),
            master_filter_cutoff: self.master_filter_cutoff,
            master_filter_resonance: self.master_filter_resonance,
            volume: self.volume,
        }
    }
    
    // Add a method to apply a preset
    fn apply_preset(&mut self, preset: &SynthPreset) {
        self.oscillators = preset.oscillators.clone();
        self.oscillator_combination_mode = preset.oscillator_combination_mode.clone();
        self.attack = preset.attack;
        self.decay = preset.decay;
        self.sustain = preset.sustain;
        self.release = preset.release;
        self.master_filter_type = preset.master_filter_type.clone();
        self.master_filter_cutoff = preset.master_filter_cutoff;
        self.master_filter_resonance = preset.master_filter_resonance;
        self.volume = preset.volume;
    }
}

fn midi_note_to_freq(note: u8) -> f32 {
    const A4_MIDI: f32 = 69.0;
    const A4_FREQ: f32 = 440.0;
    
    A4_FREQ * 2.0f32.powf((note as f32 - A4_MIDI) / 12.0)
}

// Message types for our threaded architecture
enum SynthMessage {
    NoteOn(u8, u8),
    NoteOff(u8),
    ChangeOscillator(usize, Waveform, f32, f32, i8), // (osc_index, waveform, volume, detune, octave)
    ChangeOscillatorEnvelope(usize, f32, f32, f32, f32), // (osc_index, attack, decay, sustain, release)
    ChangeOscillatorFilter(usize, FilterType, f32, f32), // (osc_index, filter_type, cutoff, resonance)
    ChangeOscillatorModulation(usize, f32, ModulationTarget), // (osc_index, amount, target)
    ChangeOscillatorCombinationMode(OscillatorCombinationMode),
    ChangeMasterEnvelope(f32, f32, f32, f32), // (attack, decay, sustain, release)
    ChangeMasterFilter(FilterType, f32, f32), // (filter_type, cutoff, resonance)
    ChangeVolume(f32),
    LoadSample(PathBuf),
    SetVolume(f32),
    SetModulation(f32),
    SetSustainPedal(bool),
    SetPitchBend(f32),
    SetAftertouch(u8, f32),
    SetChannelPressure(f32),
}

// Main app state
pub struct SynthApp {
    synth: Arc<RwLock<Synth>>,
    sender: crossbeam_channel::Sender<SynthMessage>,
    receiver: crossbeam_channel::Receiver<SynthMessage>,
    _stream: Option<Stream>,
    _midi_connection: Option<midir::MidiInputConnection<()>>,
    midi_ports: Vec<String>,
    selected_midi_port: usize,
    show_sample_dialog: bool,
    available_output_devices: Vec<cpal::Device>,
    selected_output_device_idx: usize,
    available_input_devices: Vec<cpal::Device>,
    selected_input_device_idx: usize,
    current_tab: Tab,
    last_midi_message: Option<String>,
    presets: Vec<SynthPreset>,
    current_preset_name: String,
    app_settings: AppSettings,
}

impl eframe::App for SynthApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Process any pending messages
        self.process_messages();
        
        // Create the main UI
        egui::CentralPanel::default().show(ctx, |ui| {
            // Add a header with icon
            ui.horizontal(|ui| {
                // Add the title with larger text
                ui.heading("AudioTheorem 2");
                
                // Alternative approach without loading an external image
                ui.label("🎹"); // Use a musical keyboard emoji instead of an image
            });
            
            ui.add_space(8.0); // Add some space between header and tabs
            
            // Add tabs for different sections
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.current_tab, Tab::Synth, "Synthesizer");
                ui.selectable_value(&mut self.current_tab, Tab::Audio, "Audio Settings");
                ui.selectable_value(&mut self.current_tab, Tab::Midi, "MIDI Settings");
            });
            
            ui.separator(); // Add a separator between tabs and content
            
            // Display the appropriate tab content
            match self.current_tab {
                Tab::Synth => self.render_synth_ui(ui),
                Tab::Audio => self.render_audio_settings(ui),
                Tab::Midi => self.render_midi_settings(ui),
            }
        });
        
        // Always request a repaint to keep the UI responsive
        ctx.request_repaint();
    }
}

// Add this enum to track the current tab
#[derive(PartialEq)]
enum Tab {
    Synth,
    Audio,
    Midi,
}

impl SynthApp {
    pub fn new() -> Result<Self> {
        println!("Creating SynthApp instance");
        
        // Set up audio
        let host = cpal::default_host();
        println!("Using audio host: {}", host.id().name());
        
        let device = host.default_output_device()
            .ok_or_else(|| anyhow::anyhow!("No output device available"))?;
            
        println!("Using output device: {:?}", device.name());
        
        let config = device.default_output_config()?;
        println!("Device config: {:?}", config);
        
        let sample_format = config.sample_format();
        let config = cpal::StreamConfig::from(config);
        let sample_rate = config.sample_rate.0 as f32;
        println!("Using sample rate: {}", sample_rate);
        
        // Create a channel for message passing
        let (sender, receiver) = crossbeam_channel::unbounded();
        
        // Create the synth state
        let synth = Arc::new(RwLock::new(Synth::new(sample_rate)));
        
        // Set up audio callback
        let stream = match sample_format {
            SampleFormat::F32 => create_stream::<f32>(&device, &config, Arc::clone(&synth)),
            SampleFormat::I16 => create_stream::<i16>(&device, &config, Arc::clone(&synth)),
            SampleFormat::U16 => create_stream::<u16>(&device, &config, Arc::clone(&synth)),
            _ => anyhow::bail!("Unsupported sample format"),
        }?;
        
        stream.play()?;
        println!("Audio stream started successfully");
        
        // Get output devices - collect first, then count
        let mut available_output_devices = Vec::new();
        let output_devices = host.output_devices()?;
        for device in output_devices {
            available_output_devices.push(device);
        }
        println!("Found {} output devices", available_output_devices.len());
        
        // Get input devices - collect first, then count
        let mut available_input_devices = Vec::new();
        let input_devices = host.input_devices()?;
        for device in input_devices {
            available_input_devices.push(device);
        }
        println!("Found {} input devices", available_input_devices.len());
        
        println!("[MAIN] SynthApp created successfully");
        
        Ok(SynthApp {
            synth,
            sender,
            receiver,
            _stream: Some(stream),
            _midi_connection: None,
            midi_ports: Vec::new(),
            selected_midi_port: 0,
            current_tab: Tab::Synth,
            last_midi_message: None,
            available_output_devices,
            available_input_devices,
            selected_output_device_idx: 0,
            selected_input_device_idx: 0,
            show_sample_dialog: false,
            presets: Vec::new(),
            current_preset_name: String::new(),
            app_settings: AppSettings {
                selected_midi_port: None,
                selected_output_device: None,
                selected_input_device: None,
                volume: 0.5,
                last_preset: None,
            },
        })
    }
    
    fn process_messages(&mut self) {
        // Process a limited number of messages per frame
        const MAX_MESSAGES_PER_FRAME: usize = 10;
        let mut count = 0;
        
        while let Ok(msg) = self.receiver.try_recv() {
            match msg {
                SynthMessage::NoteOn(note, velocity) => {
                    if let Ok(mut synth) = self.synth.write() {
                        synth.note_on(note, velocity);
                    }
                },
                SynthMessage::NoteOff(note) => {
                    if let Ok(mut synth) = self.synth.write() {
                        synth.note_off(note);
                    }
                },
                SynthMessage::ChangeOscillator(index, waveform, volume, detune, octave) => {
                    if let Ok(mut synth) = self.synth.write() {
                        if index < synth.oscillators.len() {
                            synth.oscillators[index].waveform = waveform;
                            synth.oscillators[index].volume = volume;
                            synth.oscillators[index].detune = detune;
                            synth.oscillators[index].octave = octave;
                        }
                    }
                },
                SynthMessage::ChangeOscillatorEnvelope(index, attack, decay, sustain, release) => {
                    if let Ok(mut synth) = self.synth.write() {
                        if index < synth.oscillators.len() {
                            synth.oscillators[index].attack = attack;
                            synth.oscillators[index].decay = decay;
                            synth.oscillators[index].sustain = sustain;
                            synth.oscillators[index].release = release;
                        }
                    }
                },
                SynthMessage::ChangeOscillatorFilter(index, filter_type, cutoff, resonance) => {
                    if let Ok(mut synth) = self.synth.write() {
                        if index < synth.oscillators.len() {
                            synth.oscillators[index].filter_type = filter_type;
                            synth.oscillators[index].filter_cutoff = cutoff;
                            synth.oscillators[index].filter_resonance = resonance;
                        }
                    }
                },
                SynthMessage::ChangeOscillatorModulation(index, amount, target) => {
                    if let Ok(mut synth) = self.synth.write() {
                        if index < synth.oscillators.len() {
                            synth.oscillators[index].mod_amount = amount;
                            synth.oscillators[index].mod_target = target;
                        }
                    }
                },
                SynthMessage::ChangeOscillatorCombinationMode(mode) => {
                    if let Ok(mut synth) = self.synth.write() {
                        synth.oscillator_combination_mode = mode;
                    }
                },
                SynthMessage::ChangeMasterEnvelope(attack, decay, sustain, release) => {
                    if let Ok(mut synth) = self.synth.write() {
                        synth.attack = attack;
                        synth.decay = decay;
                        synth.sustain = sustain;
                        synth.release = release;
                    }
                },
                SynthMessage::ChangeMasterFilter(filter_type, cutoff, resonance) => {
                    if let Ok(mut synth) = self.synth.write() {
                        synth.master_filter_type = filter_type;
                        synth.master_filter_cutoff = cutoff;
                        synth.master_filter_resonance = resonance;
                    }
                },
                SynthMessage::SetVolume(volume) => {
                    if let Ok(mut synth) = self.synth.write() {
                        synth.volume = volume;
                    }
                },
                // Handle other messages...
                _ => {}
            }
            
            count += 1;
            if count >= MAX_MESSAGES_PER_FRAME {
                break;
            }
        }
    }

    fn render_synth_ui(&mut self, ui: &mut egui::Ui) {
        // Main synthesizer controls
        ui.heading("Synthesizer");
        
        let mut synth = self.synth.write().unwrap();
        
        // Oscillator combination mode
        ui.heading("Oscillator Combination Mode");
        ui.horizontal(|ui| {
            let mut changed = false;
            let mut new_mode = synth.oscillator_combination_mode.clone();
            
            if ui.radio_value(&mut new_mode, OscillatorCombinationMode::Parallel, "Parallel").clicked() {
                changed = true;
            }
            if ui.radio_value(&mut new_mode, OscillatorCombinationMode::FM, "FM").clicked() {
                changed = true;
            }
            if ui.radio_value(&mut new_mode, OscillatorCombinationMode::AM, "AM").clicked() {
                changed = true;
            }
            if ui.radio_value(&mut new_mode, OscillatorCombinationMode::RingMod, "Ring Mod").clicked() {
                changed = true;
            }
            if ui.radio_value(&mut new_mode, OscillatorCombinationMode::Filter, "Filter").clicked() {
                changed = true;
            }
            
            if changed {
                self.sender.send(SynthMessage::ChangeOscillatorCombinationMode(new_mode.clone())).ok();
                synth.oscillator_combination_mode = new_mode;
            }
        });
        
        // Oscillator controls
        ui.heading("Oscillators");
        
        // First, collect the data we need from custom wavetables to avoid borrowing issues
        let custom_wavetable_names: Vec<(usize, String)> = synth.custom_wavetables
            .iter()
            .enumerate()
            .map(|(idx, wavetable)| (idx, wavetable.name.clone()))
            .collect();
        
        for (i, oscillator) in synth.oscillators.iter_mut().enumerate() {
            ui.collapsing(format!("Oscillator {}", i + 1), |ui| {
                // Waveform selection
                ui.horizontal(|ui| {
                    ui.label("Waveform:");
                    
                    let mut changed = false;
                    let mut new_waveform = oscillator.waveform.clone();
                    
                    if ui.radio_value(&mut new_waveform, Waveform::Sine, "Sine").clicked() {
                        changed = true;
                    }
                    if ui.radio_value(&mut new_waveform, Waveform::Square, "Square").clicked() {
                        changed = true;
                    }
                    if ui.radio_value(&mut new_waveform, Waveform::Saw, "Saw").clicked() {
                        changed = true;
                    }
                    if ui.radio_value(&mut new_waveform, Waveform::Triangle, "Triangle").clicked() {
                        changed = true;
                    }
                    if ui.radio_value(&mut new_waveform, Waveform::WhiteNoise, "Noise").clicked() {
                        changed = true;
                    }
                    
                    // Custom sample selection using our pre-collected data
                    for (idx, name) in &custom_wavetable_names {
                        if ui.radio_value(&mut new_waveform, Waveform::CustomSample(*idx), name).clicked() {
                            changed = true;
                        }
                    }
                    
                    if changed {
                        self.sender.send(SynthMessage::ChangeOscillator(
                            i, new_waveform.clone(), oscillator.volume, oscillator.detune, oscillator.octave
                        )).ok();
                        oscillator.waveform = new_waveform;
                    }
                });
                
                // Volume control
                ui.horizontal(|ui| {
                    ui.label("Volume:");
                    if ui.add(egui::Slider::new(&mut oscillator.volume, 0.0..=1.0).text("")).changed() {
                        self.sender.send(SynthMessage::ChangeOscillator(
                            i, oscillator.waveform.clone(), oscillator.volume, oscillator.detune, oscillator.octave
                        )).ok();
                    }
                });
                
                // Detune control
                ui.horizontal(|ui| {
                    ui.label("Detune:");
                    if ui.add(egui::Slider::new(&mut oscillator.detune, -12.0..=12.0).text("semitones")).changed() {
                        self.sender.send(SynthMessage::ChangeOscillator(
                            i, oscillator.waveform.clone(), oscillator.volume, oscillator.detune, oscillator.octave
                        )).ok();
                    }
                });
                
                // Octave control
                ui.horizontal(|ui| {
                    ui.label("Octave:");
                    if ui.add(egui::Slider::new(&mut oscillator.octave, -4..=4).text("")).changed() {
                        self.sender.send(SynthMessage::ChangeOscillator(
                            i, oscillator.waveform.clone(), oscillator.volume, oscillator.detune, oscillator.octave
                        )).ok();
                    }
                });
                
                // Oscillator ADSR controls
                ui.collapsing("Envelope", |ui| {
                    let mut attack = oscillator.attack;
                    let mut decay = oscillator.decay;
                    let mut sustain = oscillator.sustain;
                    let mut release = oscillator.release;
                    
                    ui.horizontal(|ui| {
                        ui.label("Attack:");
                        if ui.add(egui::Slider::new(&mut attack, 0.01..=2.0).text("s")).changed() {
                            self.sender.send(SynthMessage::ChangeOscillatorEnvelope(
                                i, attack, decay, sustain, release
                            )).ok();
                            oscillator.attack = attack;
                        }
                    });
                    
                    ui.horizontal(|ui| {
                        ui.label("Decay:");
                        if ui.add(egui::Slider::new(&mut decay, 0.01..=2.0).text("s")).changed() {
                            self.sender.send(SynthMessage::ChangeOscillatorEnvelope(
                                i, attack, decay, sustain, release
                            )).ok();
                            oscillator.decay = decay;
                        }
                    });
                    
                    ui.horizontal(|ui| {
                        ui.label("Sustain:");
                        if ui.add(egui::Slider::new(&mut sustain, 0.0..=1.0).text("")).changed() {
                            self.sender.send(SynthMessage::ChangeOscillatorEnvelope(
                                i, attack, decay, sustain, release
                            )).ok();
                            oscillator.sustain = sustain;
                        }
                    });
                    
                    ui.horizontal(|ui| {
                        ui.label("Release:");
                        if ui.add(egui::Slider::new(&mut release, 0.01..=5.0).text("s")).changed() {
                            self.sender.send(SynthMessage::ChangeOscillatorEnvelope(
                                i, attack, decay, sustain, release
                            )).ok();
                            oscillator.release = release;
                        }
                    });
                });
                
                // Oscillator filter controls
                ui.collapsing("Filter", |ui| {
                    let mut filter_type = oscillator.filter_type.clone();
                    let mut filter_cutoff = oscillator.filter_cutoff;
                    let mut filter_resonance = oscillator.filter_resonance;
                    
                    ui.horizontal(|ui| {
                        ui.label("Type:");
                        let mut changed = false;
                        
                        if ui.radio_value(&mut filter_type, FilterType::None, "None").clicked() {
                            changed = true;
                        }
                        if ui.radio_value(&mut filter_type, FilterType::LowPass, "Low Pass").clicked() {
                            changed = true;
                        }
                        if ui.radio_value(&mut filter_type, FilterType::HighPass, "High Pass").clicked() {
                            changed = true;
                        }
                        if ui.radio_value(&mut filter_type, FilterType::BandPass, "Band Pass").clicked() {
                            changed = true;
                        }
                        
                        if changed {
                            self.sender.send(SynthMessage::ChangeOscillatorFilter(
                                i, filter_type.clone(), filter_cutoff, filter_resonance
                            )).ok();
                            oscillator.filter_type = filter_type.clone();
                        }
                    });
                    
                    ui.horizontal(|ui| {
                        ui.label("Cutoff:");
                        if ui.add(egui::Slider::new(&mut filter_cutoff, 0.01..=1.0).text("")).changed() {
                            self.sender.send(SynthMessage::ChangeOscillatorFilter(
                                i, filter_type.clone(), filter_cutoff, filter_resonance
                            )).ok();
                            oscillator.filter_cutoff = filter_cutoff;
                        }
                    });
                    
                    ui.horizontal(|ui| {
                        ui.label("Resonance:");
                        if ui.add(egui::Slider::new(&mut filter_resonance, 0.0..=1.0).text("")).changed() {
                            self.sender.send(SynthMessage::ChangeOscillatorFilter(
                                i, filter_type.clone(), filter_cutoff, filter_resonance
                            )).ok();
                            oscillator.filter_resonance = filter_resonance;
                        }
                    });
                });
            });
        }
        
        // ADSR controls
        ui.heading("Master Envelope");
        ui.horizontal(|ui| {
            ui.label("Attack:");
            if ui.add(egui::Slider::new(&mut synth.attack, 0.01..=2.0).text("s")).changed() {
                self.sender.send(SynthMessage::ChangeMasterEnvelope(synth.attack, synth.decay, synth.sustain, synth.release)).ok();
            }
            
            ui.label("Decay:");
            if ui.add(egui::Slider::new(&mut synth.decay, 0.01..=2.0).text("s")).changed() {
                self.sender.send(SynthMessage::ChangeMasterEnvelope(synth.attack, synth.decay, synth.sustain, synth.release)).ok();
            }
            
            ui.label("Sustain:");
            if ui.add(egui::Slider::new(&mut synth.sustain, 0.0..=1.0).text("")).changed() {
                self.sender.send(SynthMessage::ChangeMasterEnvelope(synth.attack, synth.decay, synth.sustain, synth.release)).ok();
            }
            
            ui.label("Release:");
            if ui.add(egui::Slider::new(&mut synth.release, 0.01..=5.0).text("s")).changed() {
                self.sender.send(SynthMessage::ChangeMasterEnvelope(synth.attack, synth.decay, synth.sustain, synth.release)).ok();
            }
        });
        
        // Current waveform display
        ui.heading("Current Waveform");
        
        // Create points for the waveform display
        let wavetable_points: Vec<[f64; 2]> = synth.generate_wavetable_display()
            .iter()
            .map(|[x, y]| [*x as f64, *y as f64])
            .collect();
        
        // Display the 2D waveform plot
        Plot::new("wavetable_plot")
            .height(150.0)
            .view_aspect(3.0)
            .allow_drag(false)
            .allow_zoom(false)
            .allow_scroll(false)
            .show_axes([false, true])
            .show_grid([false, true])
            .show(ui, |plot_ui| {
                plot_ui.line(Line::new(
                    PlotPoints::from(wavetable_points)
                ).color(egui::Color32::from_rgb(100, 200, 100)));
            });
        
        // Volume control
        ui.heading("Master Volume");
        ui.horizontal(|ui| {
            if ui.add(egui::Slider::new(&mut synth.volume, 0.0..=1.0).text("Volume")).changed() {
                self.sender.send(SynthMessage::SetVolume(synth.volume)).ok();
            }
        });
        
        // Sample loading button
        ui.heading("Samples");
        if ui.button("Load Sample").clicked() {
            if let Some(path) = FileDialog::new()
                .add_filter("WAV files", &["wav"])
                .pick_file() {
                if let Err(err) = synth.load_sample(path.clone()) {
                    println!("Failed to load sample: {}", err);
                } else {
                    println!("Sample loaded successfully");
                }
            }
        }
        
        // Display loaded samples
        if !synth.custom_wavetables.is_empty() {
            ui.label("Loaded Samples:");
            for (i, wavetable) in synth.custom_wavetables.iter().enumerate() {
                ui.label(format!("{}: {}", i + 1, wavetable.name));
            }
        }
    }
    
    fn render_audio_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("Audio Device Settings");
        
        // Output device selection
        ui.label("Output Device:");
        
        if self.available_output_devices.is_empty() {
            ui.label("No output devices available");
        } else {
            egui::ComboBox::new("output_device_selector", "Select Output")
                .selected_text(self.available_output_devices
                    .get(self.selected_output_device_idx)
                    .and_then(|d| d.name().ok())
                    .unwrap_or_else(|| "No device".to_string()))
                .show_ui(ui, |ui| {
                    for (idx, device) in self.available_output_devices.iter().enumerate() {
                        if let Ok(name) = device.name() {
                            ui.selectable_value(&mut self.selected_output_device_idx, idx, name);
                        } else {
                            ui.selectable_value(&mut self.selected_output_device_idx, idx, format!("Device {}", idx));
                        }
                    }
                });
                
            if ui.button("Apply Audio Device Changes").clicked() {
                self.change_audio_devices();
            }
        }
    }
    
    fn render_midi_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("MIDI Settings");
        
        // MIDI device selection
        ui.label("MIDI Input Device:");
        
        if self.midi_ports.is_empty() {
            ui.label("No MIDI devices available");
            if ui.button("Refresh MIDI Devices").clicked() {
                self.refresh_midi_devices();
            }
        } else {
            egui::ComboBox::new("midi_port_selector", "Select MIDI Port")
                .selected_text(self.midi_ports.get(self.selected_midi_port)
                .cloned()
                .unwrap_or_else(|| "No port".to_string()))
                .show_ui(ui, |ui| {
                    for (idx, port_name) in self.midi_ports.iter().enumerate() {
                        ui.selectable_value(&mut self.selected_midi_port, idx, port_name);
                    }
                });
                
            if ui.button("Connect MIDI Device").clicked() {
                self.connect_midi(self.selected_midi_port);
            }
        }
        
        // Display last MIDI message received
        if let Some(msg) = &self.last_midi_message {
            ui.label(format!("Last MIDI message: {}", msg));
        }
    }

    fn change_audio_devices(&mut self) {
        println!("Changing audio devices");
        
        // Get the selected output device
        if self.selected_output_device_idx < self.available_output_devices.len() {
            let device = &self.available_output_devices[self.selected_output_device_idx];
            
            // Get the device configuration
            match device.default_output_config() {
                Ok(config) => {
                    let sample_format = config.sample_format();
                    let config = cpal::StreamConfig::from(config);
                    let sample_rate = config.sample_rate.0 as f32;
                    
                    // Update the synth's sample rate
                    if let Ok(mut synth) = self.synth.write() {
                        synth.sample_rate = sample_rate;
                    }
                    
                    // Create a new audio stream
                    let synth_clone = Arc::clone(&self.synth);
                    let stream_result = match sample_format {
                        SampleFormat::F32 => create_stream::<f32>(device, &config, synth_clone),
                        SampleFormat::I16 => create_stream::<i16>(device, &config, synth_clone),
                        SampleFormat::U16 => create_stream::<u16>(device, &config, synth_clone),
                        _ => Err(anyhow::anyhow!("Unsupported sample format")),
                    };
                    
                    // Replace the old stream with the new one
                    match stream_result {
                        Ok(stream) => {
                            // Stop the old stream if it exists
                            if let Some(old_stream) = self._stream.take() {
                                drop(old_stream);
                            }
                            
                            // Start the new stream
                            if let Err(err) = stream.play() {
                                println!("Failed to play stream: {}", err);
                            } else {
                                self._stream = Some(stream);
                                println!("Audio device changed successfully");
                            }
                        },
                        Err(err) => {
                            println!("Failed to create stream: {}", err);
                        }
                    }
                },
                Err(err) => {
                    println!("Failed to get device config: {}", err);
                }
            }
        }
    }
    
    fn refresh_midi_devices(&mut self) {
        println!("Refreshing MIDI devices");
        
        // Create a new MIDI input
        let midi_in = match midir::MidiInput::new("rust-synth-midi") {
            Ok(midi_in) => midi_in,
            Err(err) => {
                println!("Failed to create MIDI input: {}", err);
                return;
            }
        };
        
        // Get the available ports
        let ports = midi_in.ports();
        
        // Get the names of the ports
        self.midi_ports.clear();
        for port in ports {
            if let Ok(name) = midi_in.port_name(&port) {
                self.midi_ports.push(name);
            } else {
                self.midi_ports.push(format!("Unknown port {}", self.midi_ports.len()));
            }
        }
        
        // Reset the selected port if needed
        if !self.midi_ports.is_empty() && self.selected_midi_port >= self.midi_ports.len() {
            self.selected_midi_port = 0;
        }
        
        println!("Found {} MIDI devices", self.midi_ports.len());
    }
    
    fn connect_midi(&mut self, port_idx: usize) {
        println!("Connecting to MIDI device {}", port_idx);
        
        // Disconnect any existing connection
        self._midi_connection = None;
        
        // Check if the port index is valid
        if port_idx >= self.midi_ports.len() {
            println!("Invalid MIDI port index");
            return;
        }
        
        // Create a new MIDI input
        let mut midi_in = match midir::MidiInput::new("rust-synth-midi") {
            Ok(midi_in) => midi_in,
            Err(err) => {
                println!("Failed to create MIDI input: {}", err);
                return;
            }
        };
        
        // Configure the MIDI input
        midi_in.ignore(midir::Ignore::None);
        
        // Get the port
        let ports = midi_in.ports();
        if port_idx >= ports.len() {
            println!("MIDI port index out of range");
            return;
        }
        
        let port = &ports[port_idx];
        
        // Clone the sender for the callback
        let sender = self.sender.clone();
        
        // Connect to the port
        match midi_in.connect(port, "midi-connection", move |_stamp, message, _| {
            // Process MIDI messages
            if message.len() >= 3 {
                let status = message[0];
                let data1 = message[1];
                let data2 = message[2];
                
                match status & 0xF0 {
                    0x90 => {
                        // Note On
                        if data2 > 0 {
                            sender.send(SynthMessage::NoteOn(data1, data2)).ok();
                        } else {
                            sender.send(SynthMessage::NoteOff(data1)).ok();
                        }
                    },
                    0x80 => {
                        // Note Off
                        sender.send(SynthMessage::NoteOff(data1)).ok();
                    },
                    0xB0 => {
                        // Control Change
                        match data1 {
                            1 => {
                                // Modulation wheel
                                // Map 0-127 to 0.0-1.0
                                let value = data2 as f32 / 127.0;
                                sender.send(SynthMessage::SetModulation(value)).ok();
                            },
                            7 => {
                                // Volume
                                let value = data2 as f32 / 127.0;
                                sender.send(SynthMessage::SetVolume(value)).ok();
                            },
                            64 => {
                                // Sustain pedal
                                let on = data2 >= 64;
                                sender.send(SynthMessage::SetSustainPedal(on)).ok();
                            },
                            // Add more CC handlers as needed
                            _ => {}
                        }
                    },
                    0xE0 => {
                        // Pitch Bend
                        // Combine the two 7-bit values into one 14-bit value
                        let bend_value = ((data2 as u16) << 7) | (data1 as u16);
                        // Map from 0-16383 to -1.0 to 1.0
                        let normalized = (bend_value as f32 / 8192.0) - 1.0;
                        sender.send(SynthMessage::SetPitchBend(normalized)).ok();
                    },
                    0xA0 => {
                        // Aftertouch (Key Pressure)
                        let note = data1;
                        let pressure = data2 as f32 / 127.0;
                        sender.send(SynthMessage::SetAftertouch(note, pressure)).ok();
                    },
                    0xD0 => {
                        // Channel Pressure
                        let pressure = data1 as f32 / 127.0;
                        sender.send(SynthMessage::SetChannelPressure(pressure)).ok();
                    },
                    // Add more MIDI message handling as needed
                    _ => {}
                }
            }
        }, ()) {
            Ok(conn) => {
                println!("Connected to MIDI device");
                self._midi_connection = Some(conn);
                self.last_midi_message = Some("Connected".to_string());
            },
            Err(err) => {
                println!("Failed to connect to MIDI device: {}", err);
            }
        }
    }

    // Add a method to save a preset
    fn save_preset(&mut self, name: String) -> Result<()> {
        let synth = self.synth.read().unwrap();
        let preset = synth.create_preset(name.clone());
        
        // Check if preset with this name already exists
        if let Some(pos) = self.presets.iter().position(|p| p.name == name) {
            // Replace existing preset
            self.presets[pos] = preset.clone();
        } else {
            // Add new preset
            self.presets.push(preset.clone());
        }
        
        // Save presets to file
        self.save_presets_to_file()?;
        
        // Update current preset name
        self.current_preset_name = name;
        
        // Update app settings
        self.app_settings.last_preset = Some(self.current_preset_name.clone());
        self.save_app_settings()?;
        
        Ok(())
    }
    
    // Add a method to load a preset
    fn load_preset(&mut self, name: &str) -> Result<()> {
        if let Some(preset) = self.presets.iter().find(|p| p.name == name) {
            let mut synth = self.synth.write().unwrap();
            synth.apply_preset(preset);
            
            // Update current preset name
            self.current_preset_name = name.to_string();
            
            // Update app settings
            self.app_settings.last_preset = Some(self.current_preset_name.clone());
            self.save_app_settings()?;
        }
        
        Ok(())
    }
    
    // Add a method to delete a preset
    fn delete_preset(&mut self, name: &str) -> Result<()> {
        if let Some(pos) = self.presets.iter().position(|p| p.name == name) {
            self.presets.remove(pos);
            
            // Save presets to file
            self.save_presets_to_file()?;
            
            // If we deleted the current preset, clear the current preset name
            if self.current_preset_name == name {
                self.current_preset_name = String::new();
                self.app_settings.last_preset = None;
                self.save_app_settings()?;
            }
        }
        
        Ok(())
    }
    
    // Add a method to save presets to file
    fn save_presets_to_file(&self) -> Result<()> {
        // Create presets directory if it doesn't exist
        let presets_dir = Self::get_presets_dir()?;
        fs::create_dir_all(&presets_dir)?;
        
        // Save each preset to a separate file
        for preset in &self.presets {
            let preset_path = presets_dir.join(format!("{}.json", preset.name));
            let preset_json = serde_json::to_string_pretty(preset)?;
            let mut file = File::create(preset_path)?;
            file.write_all(preset_json.as_bytes())?;
        }
        
        Ok(())
    }
    
    fn save_app_settings(&self) -> Result<()> {
        // Create settings directory if it doesn't exist
        let settings_dir = Self::get_settings_dir()?;
        fs::create_dir_all(&settings_dir)?;
        
        // Save settings to file
        let settings_path = settings_dir.join("settings.json");
        let settings_json = serde_json::to_string_pretty(&self.app_settings)?;
        let mut file = File::create(settings_path)?;
        file.write_all(settings_json.as_bytes())?;
        
        Ok(())
    }
    
    fn get_settings_dir() -> Result<PathBuf> {
        let mut path = dirs::config_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not find config directory"))?;
        path.push("audiotheorem2");
        Ok(path)
    }
    
    fn get_presets_dir() -> Result<PathBuf> {
        let mut path = Self::get_settings_dir()?;
        path.push("presets");
        Ok(path)
    }
}

fn create_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    synth: Arc<RwLock<Synth>>,
) -> Result<Stream>
where
    T: Sample + Send + 'static + cpal::SizedSample + cpal::FromSample<f32>,
{
    let config = config.clone();
    let channels = config.channels as usize;
    let err_fn = |err| eprintln!("an error occurred on the audio stream: {}", err);
    
    let stream = device.build_output_stream(
        &config,
        move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
            // Simple audio callback that won't deadlock
            for frame in data.chunks_mut(channels) {
                let value = match synth.write() {
                    Ok(mut guard) => guard.get_sample(1.0 / 44100.0),
                    Err(_) => 0.0,
                };
                
                let value_t = T::from_sample(value);
                
                for sample in frame.iter_mut() {
                    *sample = value_t;
                }
            }
        },
        err_fn,
        None,
    )?;
    
    Ok(stream)
}

// Preset structure for saving/loading synth settings
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SynthPreset {
    name: String,
    oscillators: [Oscillator; 3],
    oscillator_combination_mode: OscillatorCombinationMode,
    attack: f32,
    decay: f32,
    sustain: f32,
    release: f32,
    master_filter_type: FilterType,
    master_filter_cutoff: f32,
    master_filter_resonance: f32,
    volume: f32,
}

// App settings structure
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AppSettings {
    selected_midi_port: Option<String>,
    selected_output_device: Option<String>,
    selected_input_device: Option<String>,
    volume: f32,
    last_preset: Option<String>,
}
