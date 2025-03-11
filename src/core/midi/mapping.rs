use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

/// Represents a parameter that can be controlled via MIDI
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MidiControlTarget {
    // Master controls
    MasterVolume,
    MasterFilterCutoff,
    MasterFilterResonance,
    MasterAttack,
    MasterDecay,
    MasterSustain,
    MasterRelease,
    
    // Per-oscillator controls (with oscillator index)
    OscillatorVolume(usize),
    OscillatorDetune(usize),
    OscillatorOctave(usize),
    OscillatorAttack(usize),
    OscillatorDecay(usize),
    OscillatorSustain(usize),
    OscillatorRelease(usize),
    OscillatorFilterCutoff(usize),
    OscillatorFilterResonance(usize),
    OscillatorModAmount(usize),
}

/// Represents a MIDI controller (CC number, channel)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MidiController {
    pub channel: u8,   // MIDI channel (0-15)
    pub cc_number: u8, // CC number (0-127)
}

/// Stores all MIDI mappings for the synth
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MidiMapping {
    // Map from controller to parameter
    pub mappings: HashMap<MidiController, MidiControlTarget>,
    // Range and curve settings for each mapping
    pub ranges: HashMap<MidiController, (f32, f32)>, // (min, max)
    pub invert: HashMap<MidiController, bool>,       // Whether to invert the control
}

impl MidiMapping {
    /// Create a new empty MIDI mapping
    pub fn new() -> Self {
        Self {
            mappings: HashMap::new(),
            ranges: HashMap::new(),
            invert: HashMap::new(),
        }
    }
    
    /// Add a new mapping from controller to parameter
    pub fn add_mapping(&mut self, controller: MidiController, target: MidiControlTarget) {
        self.mappings.insert(controller.clone(), target);
        
        // Set default range (0.0-1.0) if not already set
        if !self.ranges.contains_key(&controller) {
            self.ranges.insert(controller.clone(), (0.0, 1.0));
        }
        
        // Set default invert (false) if not already set
        if !self.invert.contains_key(&controller) {
            self.invert.insert(controller, false);
        }
    }
    
    /// Remove a mapping
    pub fn remove_mapping(&mut self, controller: &MidiController) {
        self.mappings.remove(controller);
        self.ranges.remove(controller);
        self.invert.remove(controller);
    }
    
    /// Set the range for a mapped controller
    pub fn set_range(&mut self, controller: &MidiController, min: f32, max: f32) {
        if self.mappings.contains_key(controller) {
            self.ranges.insert(controller.clone(), (min, max));
        }
    }
    
    /// Set whether a controller should be inverted
    pub fn set_invert(&mut self, controller: &MidiController, invert: bool) {
        if self.mappings.contains_key(controller) {
            self.invert.insert(controller.clone(), invert);
        }
    }
    
    /// Process a MIDI control change value into a parameter value
    pub fn process_midi_value(&self, controller: &MidiController, value: u8) -> Option<f32> {
        let normalized = value as f32 / 127.0;
        let (min, max) = self.ranges.get(controller).unwrap_or(&(0.0, 1.0));
        let mut scaled = min + (max - min) * normalized;
        if self.invert.get(controller).copied().unwrap_or(false) {
            scaled = 1.0 - scaled;
        }
        Some(scaled.clamp(*min, *max))
    }
    
    /// Save mapping to a file
    pub fn save_to_file(&self, path: &Path) -> Result<(), String> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize MIDI mapping: {}", e))?;
        
        let mut file = File::create(path)
            .map_err(|e| format!("Failed to create file: {}", e))?;
        
        file.write_all(json.as_bytes())
            .map_err(|e| format!("Failed to write mapping data: {}", e))?;
        
        Ok(())
    }
    
    /// Load mapping from a file
    pub fn load_from_file(path: &Path) -> Result<Self, String> {
        if !path.exists() {
            return Err(format!("File does not exist: {}", path.display()));
        }
        
        let mut file = File::open(path)
            .map_err(|e| format!("Failed to open file: {}", e))?;
        
        let mut contents = String::new();
        file.read_to_string(&mut contents)
            .map_err(|e| format!("Failed to read file: {}", e))?;
        
        let mapping: Self = serde_json::from_str(&contents)
            .map_err(|e| format!("Failed to parse mapping data: {}", e))?;
        
        Ok(mapping)
    }
    
    /// Create a default MIDI mapping with common controller assignments
    pub fn create_default() -> Self {
        let mut mapping = Self::new();
        
        // Add some common mappings
        mapping.add_mapping(
            MidiController { channel: 0, cc_number: 7 },
            MidiControlTarget::MasterVolume
        );
        
        mapping.add_mapping(
            MidiController { channel: 0, cc_number: 1 },
            MidiControlTarget::OscillatorModAmount(0)
        );
        
        mapping.add_mapping(
            MidiController { channel: 0, cc_number: 74 },
            MidiControlTarget::MasterFilterCutoff
        );
        
        mapping.add_mapping(
            MidiController { channel: 0, cc_number: 71 },
            MidiControlTarget::MasterFilterResonance
        );
        
        mapping
    }
}
