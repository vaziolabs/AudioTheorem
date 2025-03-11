use midir::{MidiOutput, MidiOutputConnection};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Handles MIDI output to connected devices
pub struct MidiOutputHandler {
    connection: Option<MidiOutputConnection>,
    active_ports: Vec<String>,
    last_sent: HashMap<u8, Instant>, // Control number -> last sent time
    throttle_ms: u64, // Minimum ms between messages for the same control
}

impl MidiOutputHandler {
    /// Create a new MIDI output handler
    pub fn new() -> Self {
        Self {
            connection: None,
            active_ports: Vec::new(),
            last_sent: HashMap::new(),
            throttle_ms: 20, // Default to 20ms throttling
        }
    }
    
    /// Set the throttle time for sending messages
    pub fn set_throttle(&mut self, ms: u64) {
        self.throttle_ms = ms;
    }

    /// List all available MIDI output ports
    pub fn list_ports(&mut self) -> Vec<String> {
        let mut port_names = Vec::new();
        
        match MidiOutput::new("RustSynth MIDI Output") {
            Ok(midi_out) => {
                let ports = midi_out.ports();
                for port in ports {
                    if let Ok(name) = midi_out.port_name(&port) {
                        port_names.push(name);
                    }
                }
            },
            Err(err) => {
                eprintln!("Error initializing MIDI output: {}", err);
            }
        }
        
        self.active_ports = port_names.clone();
        port_names
    }
    
    /// Connect to a specific MIDI output port by name
    pub fn connect_to_port(&mut self, port_name: &str) -> Result<(), String> {
        // First disconnect if already connected
        self.disconnect();
        
        // Create a new MIDI output instance
        let midi_out = match MidiOutput::new("RustSynth MIDI Output") {
            Ok(m) => m,
            Err(err) => return Err(format!("Failed to create MIDI output: {}", err)),
        };
        
        // Find the port with the given name
        let ports = midi_out.ports();
        let port = ports.into_iter().find(|port| {
            midi_out.port_name(port).map(|name| name == port_name).unwrap_or(false)
        });
        
        let port = match port {
            Some(p) => p,
            None => return Err(format!("MIDI port '{}' not found", port_name)),
        };
        
        // Connect to the port
        match midi_out.connect(&port, "midir-write-output") {
            Ok(conn) => {
                self.connection = Some(conn);
                Ok(())
            },
            Err(err) => Err(format!("Failed to connect to MIDI port: {}", err)),
        }
    }
    
    /// Disconnect from the currently connected MIDI port
    pub fn disconnect(&mut self) {
        self.connection = None;
    }
    
    /// Send a MIDI note on message
    pub fn send_note_on(&mut self, channel: u8, note: u8, velocity: u8) -> Result<(), String> {
        if let Some(conn) = &mut self.connection {
            let message = [0x90 | (channel & 0x0F), note, velocity];
            if let Err(err) = conn.send(&message) {
                return Err(format!("Failed to send MIDI message: {}", err));
            }
        }
        Ok(())
    }
    
    /// Send a MIDI note off message
    pub fn send_note_off(&mut self, channel: u8, note: u8, velocity: u8) -> Result<(), String> {
        if let Some(conn) = &mut self.connection {
            let message = [0x80 | (channel & 0x0F), note, velocity];
            if let Err(err) = conn.send(&message) {
                return Err(format!("Failed to send MIDI message: {}", err));
            }
        }
        Ok(())
    }
    
    /// Send a MIDI control change message (with throttling)
    pub fn send_control_change(&mut self, channel: u8, control: u8, value: u8) -> Result<(), String> {
        // Check if we need to throttle
        let now = Instant::now();
        if let Some(last_time) = self.last_sent.get(&control) {
            let elapsed = now.duration_since(*last_time);
            if elapsed < Duration::from_millis(self.throttle_ms) {
                return Ok(());  // Skip sending if too soon
            }
        }
        
        // Send the message
        if let Some(conn) = &mut self.connection {
            let message = [0xB0 | (channel & 0x0F), control, value];
            if let Err(err) = conn.send(&message) {
                return Err(format!("Failed to send MIDI message: {}", err));
            }
            
            // Update last sent time
            self.last_sent.insert(control, now);
        }
        Ok(())
    }
    
    /// Send a MIDI program change message
    pub fn send_program_change(&mut self, channel: u8, program: u8) -> Result<(), String> {
        if let Some(conn) = &mut self.connection {
            let message = [0xC0 | (channel & 0x0F), program];
            if let Err(err) = conn.send(&message) {
                return Err(format!("Failed to send MIDI message: {}", err));
            }
        }
        Ok(())
    }
    
    /// Send a MIDI pitch bend message
    pub fn send_pitch_bend(&mut self, channel: u8, value: f32) -> Result<(), String> {
        // Convert -1.0 to 1.0 into 0-16383 range
        let bend_value = ((value + 1.0) * 8192.0) as u16;
        let lsb = (bend_value & 0x7F) as u8;
        let msb = (bend_value >> 7) as u8;
        
        if let Some(conn) = &mut self.connection {
            let message = [0xE0 | (channel & 0x0F), lsb, msb];
            if let Err(err) = conn.send(&message) {
                return Err(format!("Failed to send MIDI message: {}", err));
            }
        }
        Ok(())
    }
}
