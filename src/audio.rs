use crate::oscillator::*;
use crate::note::*;

pub fn process_oscillator(
    oscillator: &Oscillator,
    phase: f32,
    sample_time: f32,
    custom_wavetables: &[CustomWavetable],
    note_state: NoteState,
    time_in_state: f32,
) -> f32 {
    if oscillator.volume <= 0.0 {
        return 0.0;
    }
    
    // Apply octave shift and detune to the frequency
    let octave_factor = 2.0f32.powf(oscillator.octave as f32);
    let detune_factor = 2.0f32.powf(oscillator.detune / 12.0);
    let frequency_factor = octave_factor * detune_factor;
    
    // Calculate modulated phase
    let mod_phase = match oscillator.mod_target {
        ModulationTarget::Pitch => {
            // Apply pitch modulation (simple LFO)
            let lfo = (sample_time * 5.0).sin() * oscillator.mod_amount;
            (phase * frequency_factor * (1.0 + lfo)) % 1.0
        },
        _ => (phase * frequency_factor) % 1.0
    };
    
    // Get waveform value based on the oscillator's waveform type
    let waveform_value = match &oscillator.waveform {
        Waveform::Sine => (2.0 * std::f32::consts::PI * mod_phase).sin(),
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
            if let Some(wavetable) = custom_wavetables.get(*index) {
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
    let osc_envelope = match note_state {
        NoteState::Attack => {
            let value = time_in_state / oscillator.attack;
            if value >= 1.0 { 1.0 } else { value }
        },
        NoteState::Decay => {
            1.0 - (1.0 - oscillator.sustain) * (time_in_state / oscillator.decay)
        },
        NoteState::Sustain => oscillator.sustain,
        NoteState::Release => {
            oscillator.sustain * (1.0 - time_in_state / oscillator.release)
        },
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
        FilterType::None => waveform_value,
    };
    
    filtered_sample * oscillator.volume * volume_mod * osc_envelope
}

pub fn combine_oscillators(
    osc_outputs: &[f32; 3],
    oscillators: &[Oscillator; 3],
    mode: &OscillatorCombinationMode,
    phase: f32,
    _sample_time: f32,
) -> f32 {
    match mode {
        OscillatorCombinationMode::Parallel => {
            // Simple addition of all oscillators
            osc_outputs[0] + osc_outputs[1] + osc_outputs[2]
        },
        OscillatorCombinationMode::FM => {
            // Simplified FM approach
            let mod_depth = 0.5;
            
            // Apply osc3 to modulate osc2
            let mod_phase2 = (phase + osc_outputs[2] * mod_depth) % 1.0;
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
            let mod_phase1 = (phase + osc2_mod * mod_depth) % 1.0;
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
    }
}

pub fn apply_envelope(
    state: NoteState,
    time_in_state: f32,
    attack: f32,
    decay: f32,
    sustain: f32,
    release: f32,
) -> f32 {
    match state {
        NoteState::Attack => {
            let value = time_in_state / attack;
            if value >= 1.0 { 1.0 } else { value }
        },
        NoteState::Decay => {
            1.0 - (1.0 - sustain) * (time_in_state / decay)
        },
        NoteState::Sustain => sustain,
        NoteState::Release => {
            sustain * (1.0 - time_in_state / release)
        },
    }
}

pub fn apply_filter(
    sample: f32,
    filter_type: &FilterType,
    cutoff: f32,
    _resonance: f32,
    _sample_rate: f32,
) -> f32 {
    // Skip processing if no filter is selected
    if *filter_type == FilterType::None {
        return sample;
    }
    
    // Normalize cutoff to 0.0-1.0 range
    let cutoff = cutoff.clamp(0.01, 0.99);
    
    // Simplified filter implementation
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