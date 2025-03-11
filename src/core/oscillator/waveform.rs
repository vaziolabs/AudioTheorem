use serde::{Serialize, Deserialize};
use std::f32::consts::PI;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Waveform {
    Sine,
    Square,
    Saw,
    Triangle,
    WhiteNoise,
    CustomSample(usize), // Index into the custom wavetables
}

impl Waveform {
    pub fn sample(&self, phase: f32, custom_samples: Option<&[f32]>) -> f32 {
        match self {
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
                // Deterministic "random" for waveform preview
                let seed = (phase * 1000.0) as i32;
                ((seed * 15731 + 789221) as f32 * 0.000000000931322574615478515625).sin()
            },
            Waveform::CustomSample(_index) => {
                if let Some(samples) = custom_samples {
                    let sample_pos = phase * samples.len() as f32;
                    let index = sample_pos.floor() as usize % samples.len();
                    samples[index]
                } else {
                    0.0
                }
            }
        }
    }
}
