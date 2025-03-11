use crate::core::oscillator::{Waveform, FilterType, ModulationTarget};
use crate::core::synth::preset::SynthPreset;
use crate::core::oscillator::OscillatorCombinationMode;
use std::path::PathBuf;

/// Message types for communication between UI and audio engine
#[derive(Debug, Clone)]
pub enum SynthMessage {
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
    SavePreset(SynthPreset),
    LoadPreset(String),
    DeletePreset(String),
    ListPresets,
}
