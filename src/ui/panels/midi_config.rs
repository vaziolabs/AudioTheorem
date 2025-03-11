use egui::{Ui, Grid, ComboBox, ScrollArea};
use std::sync::{Arc, Mutex};
use std::sync::mpsc;
use crate::core::midi::{MidiSystem, MidiController, MidiControlTarget};
use crate::messaging::SynthMessage;

pub struct MidiConfigPanel {
    midi_system: Arc<Mutex<MidiSystem>>,
    message_sender: mpsc::Sender<SynthMessage>,
    input_ports: Vec<String>,
    output_ports: Vec<String>,
    selected_input: Option<String>,
    selected_output: Option<String>,
    last_received_cc: Option<(u8, u8, u8)>, // (channel, cc, value)
    learn_mode: bool,
    learn_target: Option<MidiControlTarget>,
    status_message: Option<String>,
}

impl MidiConfigPanel {
    pub fn new(
        midi_system: Arc<Mutex<MidiSystem>>,
        message_sender: mpsc::Sender<SynthMessage>,
    ) -> Self {
        Self {
            midi_system,
            message_sender,
            input_ports: Vec::new(),
            output_ports: Vec::new(),
            selected_input: None,
            selected_output: None,
            last_received_cc: None,
            learn_mode: false,
            learn_target: None,
            status_message: None,
        }
    }
    
    pub fn refresh_ports(&mut self) {
        if let Ok(mut midi) = self.midi_system.lock() {
            self.input_ports = midi.input.list_ports().clone();
            self.output_ports = midi.output.list_ports().clone();
        }
    }
    
    pub fn update_last_cc(&mut self, channel: u8, cc: u8, value: u8) {
        self.last_received_cc = Some((channel, cc, value));
        
        // If in learn mode, create mapping
        if self.learn_mode {
            let target_copy = self.learn_target.clone();
            if let Some(target) = &target_copy {
                if let Ok(midi) = self.midi_system.lock() {
                    if let Ok(mut mapping) = midi.mapping.lock() {
                        let controller = MidiController { channel, cc_number: cc };
                        mapping.add_mapping(controller, target.clone());
                        
                        // Exit learn mode
                        self.learn_mode = false;
                        self.learn_target = None;
                        self.status_message = Some(format!("Mapped CC {} to {:?}", cc, target));
                        
                        // Save mapping
                        if let Err(err) = midi.save_mapping() {
                            self.status_message = Some(format!("Error saving mapping: {}", err));
                        }
                    }
                }
            }
        }
    }
    
    pub fn show(&mut self, ui: &mut Ui) {
        ui.heading("MIDI Configuration");
        
        // Status message
        if let Some(msg) = &self.status_message {
            ui.label(msg);
            ui.separator();
        }
        
        // Port selection
        ui.collapsing("MIDI Ports", |ui| {
            ui.horizontal(|ui| {
                if ui.button("Refresh Ports").clicked() {
                    self.refresh_ports();
                }
            });
            
            ui.group(|ui| {
                ui.heading("MIDI Input");
                let mut current_input = self.selected_input.clone().unwrap_or_else(|| "None".to_string());
                
                ComboBox::new("midi_input_selector", "MIDI Input")
                    .selected_text(&current_input)
                    .show_ui(ui, |ui| {
                        for port in &self.input_ports {
                            if ui.selectable_value(&mut current_input, port.clone(), port).changed() {
                                self.selected_input = Some(port.clone());
                                
                                // Connect to selected port
                                if let Ok(mut midi) = self.midi_system.lock() {
                                    if let Err(err) = midi.input.connect_to_port(&port) {
                                        self.status_message = Some(format!("Error connecting to input port: {}", err));
                                    } else {
                                        self.status_message = Some(format!("Connected to input port: {}", port));
                                    }
                                }
                            }
                        }
                    });
            });
            
            ui.group(|ui| {
                ui.heading("MIDI Output");
                let mut current_output = self.selected_output.clone().unwrap_or_else(|| "None".to_string());
                
                ComboBox::new("midi_output_selector", "MIDI Output")
                    .selected_text(&current_output)
                    .show_ui(ui, |ui| {
                        for port in &self.output_ports {
                            if ui.selectable_value(&mut current_output, port.clone(), port).changed() {
                                self.selected_output = Some(port.clone());
                                
                                // Connect to selected port
                                if let Ok(mut midi) = self.midi_system.lock() {
                                    if let Err(err) = midi.output.connect_to_port(&port) {
                                        self.status_message = Some(format!("Error connecting to output port: {}", err));
                                    } else {
                                        self.status_message = Some(format!("Connected to output port: {}", port));
                                    }
                                }
                            }
                        }
                    });
            });
        });
        
        // MIDI Monitor
        ui.collapsing("MIDI Monitor", |ui| {
            ui.group(|ui| {
                ui.heading("Last Received CC");
                if let Some((channel, cc, value)) = self.last_received_cc {
                    ui.label(format!("Channel: {}, CC: {}, Value: {}", channel, cc, value));
                } else {
                    ui.label("No MIDI messages received yet");
                }
            });
        });
        
        // MIDI Mapping
        ui.collapsing("MIDI Mapping", |ui| {
            if ui.button("Start MIDI Learn").clicked() {
                self.learn_mode = true;
                self.status_message = Some("MIDI Learn mode active - move a control to map it".to_string());
            }
            
            if self.learn_mode {
                ui.colored_label(egui::Color32::GREEN, "MIDI Learn mode active");
                if ui.button("Cancel").clicked() {
                    self.learn_mode = false;
                    self.learn_target = None;
                    self.status_message = Some("MIDI Learn cancelled".to_string());
                }
                
                // Target selector
                ui.group(|ui| {
                    ui.heading("Select Target Parameter");
                    ui.horizontal(|ui| {
                        let targets = [
                            ("Master Volume", MidiControlTarget::MasterVolume),
                            ("Master Filter Cutoff", MidiControlTarget::MasterFilterCutoff),
                            ("Master Filter Resonance", MidiControlTarget::MasterFilterResonance),
                            ("Master Attack", MidiControlTarget::MasterAttack),
                            ("Master Decay", MidiControlTarget::MasterDecay),
                            ("Master Sustain", MidiControlTarget::MasterSustain),
                            ("Master Release", MidiControlTarget::MasterRelease),
                            ("Osc 1 Volume", MidiControlTarget::OscillatorVolume(0)),
                            ("Osc 1 Mod Amount", MidiControlTarget::OscillatorModAmount(0)),
                            // Add more targets as needed
                        ];
                        
                        for (name, target) in &targets {
                            if ui.button(*name).clicked() {
                                self.learn_target = Some(target.clone());
                                self.status_message = Some(format!("Move a MIDI control to map to {}", name));
                            }
                        }
                    });
                });
            }
            
            // Current mappings
            ui.group(|ui| {
                ui.heading("Current Mappings");
                ScrollArea::vertical().show(ui, |ui| {
                    if let Ok(midi) = self.midi_system.lock() {
                        if let Ok(mapping) = midi.mapping.lock() {
                            Grid::new("midi_mapping_grid").show(ui, |ui| {
                                ui.heading("Controller");
                                ui.heading("Target");
                                ui.heading("Actions");
                                ui.end_row();
                                
                                for (controller, target) in &mapping.mappings {
                                    ui.label(format!("CH:{} CC:{}", controller.channel, controller.cc_number));
                                    ui.label(format!("{:?}", target));
                                    
                                    if ui.button("Delete").clicked() {
                                        if let Ok(midi) = self.midi_system.lock() {
                                            if let Ok(mut mapping) = midi.mapping.lock() {
                                                mapping.remove_mapping(controller);
                                                midi.save_mapping().ok();
                                            }
                                        }
                                    }
                                }
                            });
                        }
                    }
                });
            });
        });
    }
}
