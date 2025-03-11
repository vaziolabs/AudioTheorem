use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ModulationTarget {
    None,
    Pitch,
    FilterCutoff,
    Volume,
    PulseWidth,
}

pub struct ModulationState {
    pub amount: f32,
    pub target: ModulationTarget,
    pub rate: f32,
}

impl ModulationState {
    pub fn new() -> Self {
        Self {
            amount: 0.0,
            target: ModulationTarget::None,
            rate: 1.0,
        }
    }
    
    pub fn apply(&self, base_value: f32, sample_time: f32) -> f32 {
        match self.target {
            ModulationTarget::None => base_value,
            ModulationTarget::Volume => {
                // Tremolo effect
                base_value * (1.0 + (sample_time * self.rate * 6.0).sin() * self.amount)
            },
            ModulationTarget::FilterCutoff => {
                // Filter cutoff modulation
                base_value * (1.0 + (sample_time * self.rate * 4.0).sin() * self.amount)
            },
            ModulationTarget::Pitch => {
                // Vibrato effect
                base_value * (1.0 + (sample_time * self.rate * 5.0).sin() * self.amount * 0.1)
            },
            ModulationTarget::PulseWidth => {
                // PW modulation (handled elsewhere, return unchanged)
                base_value
            }
        }
    }
}
