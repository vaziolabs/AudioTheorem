use std::sync::mpsc;
use midir::{MidiInput, MidiInputConnection};
use crate::messaging::SynthMessage;

/// Handles MIDI input from connected devices
pub struct MidiInputHandler {
    connection: Option<MidiInputConnection<()>>,
    active_ports: Vec<String>,
    message_sender: mpsc::Sender<SynthMessage>,
}

impl MidiInputHandler {
    /// Create a new MIDI input handler
    pub fn new(message_sender: mpsc::Sender<SynthMessage>) -> Self {
        Self {
            connection: None,
            active_ports: Vec::new(),
            message_sender,
        }
    }

    /// Initialize and list all available MIDI input ports
    pub fn list_ports(&mut self) -> Vec<String> {
        let mut port_names = Vec::new();
        
        match MidiInput::new("RustSynth MIDI Input") {
            Ok(midi_in) => {
                let ports = midi_in.ports();
                for port in ports {
                    if let Ok(name) = midi_in.port_name(&port) {
                        port_names.push(name);
                    }
                }
            },
            Err(err) => {
                eprintln!("Error initializing MIDI input: {}", err);
            }
        }
        
        self.active_ports = port_names.clone();
        port_names
    }
    
    /// Connect to a specific MIDI input port by name
    pub fn connect_to_port(&mut self, port_name: &str) -> Result<(), String> {
        // First disconnect if already connected
        self.disconnect();
        
        // Create a new MIDI input instance
        let midi_in = match MidiInput::new("RustSynth MIDI Input") {
            Ok(m) => m,
            Err(err) => return Err(format!("Failed to create MIDI input: {}", err)),
        };
        
        // Find the port with the given name
        let ports = midi_in.ports();
        let port = ports.into_iter().find(|port| {
            midi_in.port_name(port).map(|name| name == port_name).unwrap_or(false)
        });
        
        let port = match port {
            Some(p) => p,
            None => return Err(format!("MIDI port '{}' not found", port_name)),
        };
        
        // Clone the sender for the callback closure
        let sender = self.message_sender.clone();
        
        // Connect to the port with a callback function
        match midi_in.connect(&port, "midir-read-input", move |_stamp, message, _| {
            Self::handle_midi_message(message, &sender);
        }, ()) {
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
    
    /// Process incoming MIDI messages and convert to SynthMessages
    fn handle_midi_message(message: &[u8], sender: &mpsc::Sender<SynthMessage>) {
        if message.len() < 2 {
            return;
        }
        
        // Extract the MIDI message type (status byte)
        let status = message[0] & 0xF0;
        let _channel = message[0] & 0x0F;
        
        match status {
            0x80 => { // Note Off
                if message.len() >= 3 {
                    let note = message[1];
                    sender.send(SynthMessage::NoteOff(note)).ok();
                }
            },
            0x90 => { // Note On
                if message.len() >= 3 {
                    let note = message[1];
                    let velocity = message[2];
                    
                    if velocity == 0 {
                        // Note On with velocity 0 is equivalent to Note Off
                        sender.send(SynthMessage::NoteOff(note)).ok();
                    } else {
                        sender.send(SynthMessage::NoteOn(note, velocity)).ok();
                    }
                }
            },
            0xA0 => { // Polyphonic Aftertouch
                if message.len() >= 3 {
                    let note = message[1];
                    let pressure = message[2] as f32 / 127.0;
                    sender.send(SynthMessage::SetAftertouch(note, pressure)).ok();
                }
            },
            0xB0 => { // Control Change
                if message.len() >= 3 {
                    let control = message[1];
                    let value = message[2] as f32 / 127.0;
                    
                    match control {
                        1 => { // Modulation Wheel
                            sender.send(SynthMessage::SetModulation(value)).ok();
                        },
                        7 => { // Volume
                            sender.send(SynthMessage::SetVolume(value)).ok();
                        },
                        64 => { // Sustain Pedal
                            let on = value >= 0.5;
                            sender.send(SynthMessage::SetSustainPedal(on)).ok();
                        },
                        // Handle other control changes based on MIDI mapping
                        _ => {
                            // Custom control handling will be passed through the mapping system
                        }
                    }
                }
            },
            0xC0 => { // Program Change
                if message.len() >= 2 {
                    let _program = message[1];
                    // Could be used for preset selection
                }
            },
            0xD0 => { // Channel Pressure (Aftertouch)
                if message.len() >= 2 {
                    let pressure = message[1] as f32 / 127.0;
                    sender.send(SynthMessage::SetChannelPressure(pressure)).ok();
                }
            },
            0xE0 => { // Pitch Bend
                if message.len() >= 3 {
                    let lsb = message[1] as u16;
                    let msb = message[2] as u16;
                    let bend_value = ((msb << 7) | lsb) as f32;
                    
                    // Convert from 0-16383 to -1.0 to 1.0 range
                    let normalized = (bend_value / 8192.0) - 1.0;
                    sender.send(SynthMessage::SetPitchBend(normalized)).ok();
                }
            },
            _ => {
                // Ignore other MIDI message types
            }
        }
    }
}
