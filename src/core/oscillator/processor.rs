use crate::core::oscillator::{Oscillator, CustomWavetable, NoteState, ModulationTarget, Waveform, FilterType};

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
    
    let octave_factor = 2.0f32.powf(oscillator.octave as f32);
    let detune_factor = 2.0f32.powf(oscillator.detune / 12.0);
    let frequency_factor = octave_factor * detune_factor;
    
    let mod_phase = match oscillator.mod_target {
        ModulationTarget::Pitch => {
            let lfo = (sample_time * 5.0).sin() * oscillator.mod_amount;
            (phase * frequency_factor * (1.0 + lfo)) % 1.0
        },
        _ => (phase * frequency_factor) % 1.0
    };

    let waveform_value = match &oscillator.waveform {
        Waveform::Sine => (2.0 * std::f32::consts::PI * mod_phase).sin(),
        Waveform::Square => {
            let pulse_width = match oscillator.mod_target {
                ModulationTarget::PulseWidth => 0.5 + (sample_time * 3.0).sin() * oscillator.mod_amount * 0.4,
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
                let position = mod_phase * wavetable.samples.len() as f32;
                let index = position.floor() as usize % wavetable.samples.len();
                let next_index = (index + 1) % wavetable.samples.len();
                let fraction = position - position.floor();
                wavetable.samples[index] * (1.0 - fraction) + 
                wavetable.samples[next_index] * fraction
            } else {
                0.0
            }
        }
    };

    let osc_envelope = match note_state {
        NoteState::Attack => (time_in_state / oscillator.attack).min(1.0),
        NoteState::Decay => 1.0 - (1.0 - oscillator.sustain) * (time_in_state / oscillator.decay),
        NoteState::Sustain => oscillator.sustain,
        NoteState::Release => oscillator.sustain * (1.0 - time_in_state / oscillator.release),
        NoteState::Off => 0.0,
        NoteState::Pressed => 0.0,
        NoteState::Released => 0.0,
    };

    let volume_mod = match oscillator.mod_target {
        ModulationTarget::Volume => 1.0 + (sample_time * 6.0).sin() * oscillator.mod_amount,
        _ => 1.0
    };

    let filter_cutoff_mod = match oscillator.mod_target {
        ModulationTarget::FilterCutoff => oscillator.filter_cutoff * (1.0 + (sample_time * 4.0).sin() * oscillator.mod_amount),
        _ => oscillator.filter_cutoff
    };

    let filtered_sample = match oscillator.filter_type {
        FilterType::LowPass => waveform_value * filter_cutoff_mod.clamp(0.01, 0.99).powf(0.5),
        FilterType::HighPass => waveform_value * (1.0 - filter_cutoff_mod.clamp(0.01, 0.99).powf(0.5)),
        FilterType::BandPass => waveform_value * (1.0 - (filter_cutoff_mod.clamp(0.01, 0.99) - 0.5).abs() * 2.0),
        FilterType::Notch => {
            let cutoff = filter_cutoff_mod.clamp(0.01, 0.99);
            let band_factor = 1.0 - (cutoff - 0.5).abs() * 2.0;
            waveform_value * band_factor
        },
        FilterType::None => waveform_value,
    };

    filtered_sample * oscillator.volume * volume_mod * osc_envelope
}
