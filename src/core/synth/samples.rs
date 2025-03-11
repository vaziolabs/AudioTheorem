use std::path::PathBuf;
use anyhow::Result;
use crate::core::oscillator::CustomWavetable;
use crate::core::analyzer::MAX_CUSTOM_WAVETABLES;

impl super::Synth {
    /// Load a sample from a WAV file into a custom wavetable
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
}
