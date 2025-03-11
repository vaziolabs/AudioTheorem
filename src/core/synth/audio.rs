use std::f32::consts::PI;
use crate::core::oscillator::{Waveform, FilterType, ModulationTarget, OscillatorCombinationMode, NoteState};
use crate::core::analyzer::SAMPLE_BUFFER_SIZE;

/// Convert MIDI note number to frequency in Hz
pub fn midi_note_to_freq(note: u8) -> f32 {
    440.0 * 2.0f32.powf((note as f32 - 69.0) / 12.0)
}

impl super::Synth {
    /// Process audio filter
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
        let omega = 2.0 * PI * cutoff / self.sample_rate;
        let sin_omega = omega.sin();
        let _cos_omega = omega.cos();
        let _alpha = sin_omega / (2.0 * q);
        
        // Since we can't modify filter states here, we'll use a simplified approach
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
            FilterType::Notch => {
                // Process with notch filter
                let cutoff = cutoff.clamp(0.01, 0.99);
                let band_factor = 1.0 - (cutoff - 0.5).abs() * 2.0;
                sample * (1.0 - band_factor)
            },
            FilterType::None => sample,
        }
    }
    
    /// Generate one audio sample
    pub fn get_sample(&mut self, sample_time: f32) -> f32 {
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
                NoteState::Off => 0.0,
                NoteState::Pressed => 0.0,
                NoteState::Released => 0.0,
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
                            let lfo = (sample_time * 5.0).sin() * oscillator.mod_amount;
                            (note.phase * frequency_factor * (1.0 + lfo)) % 1.0
                        },
                        _ => (note.phase * frequency_factor) % 1.0
                    };
                    
                    // Get waveform value based on the oscillator's waveform type
                    let waveform_value = match &oscillator.waveform {
                        Waveform::Sine => (2.0 * PI * mod_phase).sin(),
                        Waveform::Square => {
                            // Apply pulse width modulation if selected
                            let pulse_width = match oscillator.mod_target {
                                ModulationTarget::PulseWidth => {
                                    // Modulate pulse width between 0.1 and 0.9
                                    let lfo = (sample_time * 3.0).sin() * oscillator.mod_amount;
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
                        NoteState::Off => 0.0,
                        NoteState::Pressed => 0.0,
                        NoteState::Released => 0.0,
                    };
                    
                    // Apply volume modulation if selected
                    let volume_mod = match oscillator.mod_target {
                        ModulationTarget::Volume => {
                            // Tremolo effect
                            1.0 + (sample_time * 6.0).sin() * oscillator.mod_amount
                        },
                        _ => 1.0
                    };
                    
                    // Apply filter modulation if selected
                    let filter_cutoff_mod = match oscillator.mod_target {
                        ModulationTarget::FilterCutoff => {
                            // Filter cutoff modulation
                            oscillator.filter_cutoff * (1.0 + (sample_time * 4.0).sin() * oscillator.mod_amount)
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
                        FilterType::Notch => {
                            let cutoff = filter_cutoff_mod.clamp(0.01, 0.99);
                            let band_factor = 1.0 - (cutoff - 0.5).abs() * 2.0;
                            waveform_value * (1.0 - band_factor)
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
                        _ => osc_outputs[1],
                    } * oscillators[1].volume;
                    
                    // Apply osc2 to modulate osc1
                    let mod_phase1 = (note.phase + osc2_mod * mod_depth) % 1.0;
                    let osc1_mod = match &oscillators[0].waveform {
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
}
