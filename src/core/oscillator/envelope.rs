use serde::{Serialize, Deserialize};
use crate::core::oscillator::NoteState;

#[derive(Debug, Clone, Copy)]
pub enum EnvelopeStage {
    Attack,
    Decay,
    Sustain,
    Release,
    Idle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Envelope {
    pub attack: f32,
    pub decay: f32,
    pub sustain: f32,
    pub release: f32,
}

impl Envelope {
    pub fn new() -> Self {
        Self {
            attack: 0.1,
            decay: 0.2,
            sustain: 0.7,
            release: 0.3,
        }
    }

    pub fn value_at_stage(&self, stage: EnvelopeStage, time_in_stage: f32) -> f32 {
        match stage {
            EnvelopeStage::Attack => {
                let value = time_in_stage / self.attack;
                if value >= 1.0 { 1.0 } else { value }
            },
            EnvelopeStage::Decay => {
                1.0 - (1.0 - self.sustain) * (time_in_stage / self.decay)
            },
            EnvelopeStage::Sustain => self.sustain,
            EnvelopeStage::Release => {
                self.sustain * (1.0 - time_in_stage / self.release)
            },
            EnvelopeStage::Idle => 0.0,
        }
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
        NoteState::Attack => (time_in_state / attack).min(1.0),
        NoteState::Decay => 1.0 - (1.0 - sustain) * (time_in_state / decay),
        NoteState::Sustain => sustain,
        NoteState::Release => sustain * (1.0 - time_in_state / release),
        NoteState::Off => 0.0,
        NoteState::Pressed => 0.0,
        NoteState::Released => 0.0,
    }
}
