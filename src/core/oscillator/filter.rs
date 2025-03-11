use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FilterType {
    None,
    LowPass,
    HighPass,
    BandPass,
    Notch,
}

pub struct FilterState {
    pub filter_type: FilterType,
    pub cutoff: f32,
    pub resonance: f32,
    pub state_vars: [f32; 4], // For more advanced filters
}

impl FilterState {
    pub fn new() -> Self {
        Self {
            filter_type: FilterType::None,
            cutoff: 1.0,
            resonance: 0.0,
            state_vars: [0.0; 4],
        }
    }

    pub fn process(&mut self, input: f32) -> f32 {
        match self.filter_type {
            FilterType::None => input,
            FilterType::LowPass => {
                let cutoff = self.cutoff.clamp(0.01, 0.99);
                input * cutoff.powf(0.5)
            },
            FilterType::HighPass => {
                let cutoff = self.cutoff.clamp(0.01, 0.99);
                input * (1.0 - cutoff.powf(0.5))
            },
            FilterType::BandPass => {
                let cutoff = self.cutoff.clamp(0.01, 0.99);
                let band_factor = 1.0 - (cutoff - 0.5).abs() * 2.0;
                input * band_factor
            },
            FilterType::Notch => {
                let cutoff = self.cutoff.clamp(0.01, 0.99);
                let band_factor = 1.0 - (cutoff - 0.5).abs() * 2.0;
                input * (1.0 - band_factor)
            },
        }
    }
}
