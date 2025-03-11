use std::fs;
use std::path::Path;
use anyhow::{Result, Context};

/// Normalize a vector of audio samples to the range [-1.0, 1.0]
pub fn normalize_samples(samples: &mut [f32]) {
    if samples.is_empty() {
        return;
    }
    
    // Find the maximum absolute value
    let max_abs = samples.iter()
        .fold(0.0f32, |max, &sample| max.max(sample.abs()));
    
    if max_abs > 0.0 {
        // Normalize all samples
        for sample in samples.iter_mut() {
            *sample /= max_abs;
        }
    }
}

/// Format a frequency value with appropriate unit suffix (Hz, kHz)
pub fn format_frequency(freq: f32) -> String {
    if freq >= 1000.0 {
        format!("{:.2} kHz", freq / 1000.0)
    } else {
        format!("{:.1} Hz", freq)
    }
}

/// Format a time value with appropriate unit suffix (ms, s)
pub fn format_time(time_in_seconds: f32) -> String {
    if time_in_seconds >= 1.0 {
        format!("{:.2} s", time_in_seconds)
    } else {
        format!("{:.0} ms", time_in_seconds * 1000.0)
    }
}

/// Load an audio file and convert it to a mono sample vector
pub fn load_audio_file(path: &Path) -> Result<(Vec<f32>, u32)> {
    // Read file data
    let _file_data = fs::read(path)
        .with_context(|| format!("Failed to read file: {}", path.display()))?;
    
    // For WAV files (simplified implementation)
    if let Some(extension) = path.extension() {
        if extension.to_string_lossy().to_lowercase() == "wav" {
            // This is a placeholder for actual WAV file parsing
            // A real implementation would use a proper audio file library
            let samples = vec![0.0; 1024]; // Dummy sample data
            let sample_rate = 44100;
            
            return Ok((samples, sample_rate));
        }
    }
    
    // For other formats or if WAV parsing failed
    Err(anyhow::anyhow!("Unsupported audio file format"))
}

/// Convert a MIDI note number to its frequency in Hz
pub fn midi_note_to_freq(note: u8) -> f32 {
    // A4 (note 69) is 440 Hz
    440.0 * 2.0f32.powf((note as f32 - 69.0) / 12.0)
}

/// Convert a frequency in Hz to the closest MIDI note number
pub fn freq_to_midi_note(freq: f32) -> u8 {
    // A4 (note 69) is 440 Hz
    let note = 69.0 + 12.0 * (freq / 440.0).log2();
    note.round().clamp(0.0, 127.0) as u8
}

/// Calculate decibels from a linear amplitude value
pub fn amplitude_to_db(amplitude: f32) -> f32 {
    // Avoid log of zero
    if amplitude <= 0.0 {
        -96.0 // Minimum dB value (near silence)
    } else {
        20.0 * amplitude.log10()
    }
}

/// Convert decibels to a linear amplitude value
pub fn db_to_amplitude(db: f32) -> f32 {
    10.0f32.powf(db / 20.0)
}
