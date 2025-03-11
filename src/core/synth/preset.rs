use serde::{Serialize, Deserialize};
use std::fs::{self, File};
use std::io::{Write, Read};
use std::path::{Path, PathBuf};
use anyhow::{Result, Context};
use crate::core::oscillator::{Oscillator, FilterType, OscillatorCombinationMode};

/// Represents a complete synth preset with all settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SynthPreset {
    /// Unique name for the preset
    pub name: String,
    /// Description of the preset sound
    pub description: String,
    /// Author of the preset
    pub author: String,
    /// Tags for categorization
    pub tags: Vec<String>,
    /// Creation timestamp
    pub created_at: u64,
    
    /// Master volume setting
    pub master_volume: f32,
    /// Master filter settings
    pub master_filter_type: FilterType,
    pub master_filter_cutoff: f32,
    pub master_filter_resonance: f32,
    
    /// Master envelope settings
    pub master_attack: f32,
    pub master_decay: f32,
    pub master_sustain: f32,
    pub master_release: f32,
    
    /// Oscillator settings
    pub oscillators: Vec<Oscillator>,
    /// How oscillators are combined
    pub oscillator_combination_mode: OscillatorCombinationMode,
}

impl SynthPreset {
    /// Create a new empty preset with default values
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: String::new(),
            author: String::new(),
            tags: Vec::new(),
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            
            master_volume: 0.7,
            master_filter_type: FilterType::None,
            master_filter_cutoff: 1.0,
            master_filter_resonance: 0.0,
            
            master_attack: 0.01,
            master_decay: 0.1,
            master_sustain: 0.7,
            master_release: 0.3,
            
            oscillators: vec![Oscillator::new()],
            oscillator_combination_mode: OscillatorCombinationMode::Parallel,
        }
    }
    
    /// Save preset to a file
    pub fn save_to_file(&self, directory: &Path) -> Result<PathBuf> {
        // Create the directory if it doesn't exist
        fs::create_dir_all(directory)
            .context("Failed to create preset directory")?;
        
        // Generate safe filename from preset name
        let safe_name = self.name.replace(" ", "_")
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
            .collect::<String>();
        
        let file_path = directory.join(format!("{}.json", safe_name));
        
        // Serialize to JSON
        let json = serde_json::to_string_pretty(self)
            .context("Failed to serialize preset")?;
        
        // Write to file
        let mut file = File::create(&file_path)
            .context("Failed to create preset file")?;
        file.write_all(json.as_bytes())
            .context("Failed to write preset data")?;
        
        Ok(file_path)
    }
    
    /// Load preset from a file
    pub fn load_from_file(path: &Path) -> Result<Self> {
        // Read file contents
        let mut file = File::open(path)
            .context("Failed to open preset file")?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)
            .context("Failed to read preset file")?;
        
        // Deserialize from JSON
        let preset: Self = serde_json::from_str(&contents)
            .context("Failed to parse preset data")?;
        
        Ok(preset)
    }
    
    /// List all available presets in a directory
    pub fn list_presets(directory: &Path) -> Result<Vec<String>> {
        // Create the directory if it doesn't exist
        if !directory.exists() {
            fs::create_dir_all(directory)?;
            return Ok(Vec::new());
        }
        
        let mut presets = Vec::new();
        
        for entry in fs::read_dir(directory)? {
            if let Ok(entry) = entry {
                let path = entry.path();
                if path.is_file() && path.extension().map_or(false, |ext| ext == "json") {
                    if let Some(filename) = path.file_stem() {
                        if let Some(name) = filename.to_str() {
                            // Convert filename back to display name
                            let display_name = name.replace("_", " ");
                            presets.push(display_name);
                        }
                    }
                }
            }
        }
        
        Ok(presets)
    }
    
    /// Delete a preset file
    pub fn delete_preset(name: &str, directory: &Path) -> Result<()> {
        let safe_name = name.replace(" ", "_")
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
            .collect::<String>();
        
        let file_path = directory.join(format!("{}.json", safe_name));
        
        if file_path.exists() {
            fs::remove_file(file_path)?;
            Ok(())
        } else {
            Err(anyhow::anyhow!("Preset does not exist"))
        }
    }
}

impl Default for SynthPreset {
    fn default() -> Self {
        Self {
            name: "Default".to_string(),
            description: "Default preset".to_string(),
            author: "Default".to_string(),
            tags: vec!["Default".to_string()],
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            oscillators: vec![Oscillator::new(), Oscillator::new(), Oscillator::new()],
            oscillator_combination_mode: OscillatorCombinationMode::Parallel,
            master_attack: 0.1,
            master_decay: 0.2,
            master_sustain: 0.7,
            master_release: 0.3,
            master_filter_type: FilterType::None,
            master_filter_cutoff: 1.0,
            master_filter_resonance: 0.0,
            master_volume: 0.5,
        }
    }
}