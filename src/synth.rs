use anyhow::Result;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::Instant;
use std::f32::consts::PI;
use serde::{Serialize, Deserialize};
use crate::note::*;
use crate::oscillator::*;

const SAMPLE_BUFFER_SIZE: usize = 1024;
const MAX_CUSTOM_WAVETABLES: usize = 8;
pub const WAVEFORM_DISPLAY_POINTS: usize = 200;

fn midi_note_to_freq(note: u8) -> f32 {
    const A4_MIDI: f32 = 69.0;
    const A4_FREQ: f32 = 440.0;
    
    A4_FREQ * 2.0f32.powf((note as f32 - A4_MIDI) / 12.0)
}

struct Analyzer {
    current_waveform_samples: VecDeque<f32>,
    fft_buffer: Vec<f32>,
    last_update: Instant,
}

// Preset structure for saving/loading synth settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SynthPreset {
    pub name: String,
    pub oscillators: [Oscillator; 3],
    pub oscillator_combination_mode: OscillatorCombinationMode,
    pub attack: f32,
    pub decay: f32,
    pub sustain: f32,
    pub release: f32,
    pub master_filter_type: FilterType,
    pub master_filter_cutoff: f32,
    pub master_filter_resonance: f32,
    pub volume: f32,
}

// Main synthesizer state
pub struct Synth {
    pub sample_rate: f32,
    pub volume: f32,
    pub active_notes: Vec<Note>,
    pub oscillators: [Oscillator; 3], // Three oscillators
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
    
    pub custom_wavetables: Vec<CustomWavetable>,
    pub analyzer: Analyzer,
    
    // Filter state variables (for each oscillator and master)
    pub filter_states: [[f32; 4]; 4], // 3 oscillators + 1 master, 4 states per filter
}

impl Synth {
    pub fn new(sample_rate: f32) -> Self {
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
                    pitch_bend: 0.0,
                    aftertouch: 0.0,
                    channel_pressure: 0.0,
                    note: None,
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
                    pitch_bend: 0.0,
                    aftertouch: 0.0,
                    channel_pressure: 0.0,
                    note: None,
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
                    pitch_bend: 0.0,
                    aftertouch: 0.0,
                    channel_pressure: 0.0,
                    note: None,
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

    pub fn note_on(&mut self, midi_note: u8, velocity: u8) {
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

    pub fn note_off(&mut self, midi_note: u8) {
        for note in self.active_notes.iter_mut() {
            if note.midi_note == midi_note {
                note.state = NoteState::Release;
                note.time_in_state = 0.0;
            }
        }
    }

    pub fn load_sample(&mut self, path: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
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
    pub fn apply_filter(&self, sample: f32, filter_type: &FilterType, cutoff: f32, resonance: f32, _filter_index: usize) -> f32 {
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
    pub fn get_sample(&mut self, _sample_time: f32) -> f32 {
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
    pub fn generate_waveform_display(&self) -> Vec<[f32; 2]> {
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
    pub fn generate_wavetable_display(&self) -> Vec<[f32; 2]> {
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
    pub fn create_preset(&self, name: String) -> SynthPreset {
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
    pub fn apply_preset(&mut self, preset: &SynthPreset) {
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
