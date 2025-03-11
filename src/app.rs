use anyhow::Result;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Sample, SampleFormat, Stream};
use eframe::egui;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use serde::{Serialize, Deserialize};
use std::fs::{self, File};
use std::io::Write;
use serde_json;
use dirs;
use crate::core::synth::Synth;
use crate::core::synth::preset::SynthPreset;
use crate::messaging::{SynthMessage, MessageBus};

// Main app state
pub struct SynthApp {
    synth: Arc<RwLock<Synth>>,
    message_bus: MessageBus,
    _stream: Option<Stream>,
    _midi_connection: Option<midir::MidiInputConnection<()>>,
    midi_ports: Vec<String>,
    selected_midi_port: usize,
    show_sample_dialog: bool,
    available_output_devices: Vec<cpal::Device>,
    selected_output_device_idx: usize,
    available_input_devices: Vec<cpal::Device>,
    selected_input_device_idx: usize,
    current_tab: Tab,
    last_midi_message: Option<String>,
    presets: Vec<SynthPreset>,
    current_preset_name: String,
    app_settings: AppSettings,
    should_exit: bool,
}

impl eframe::App for SynthApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Process any pending messages
        self.process_messages();
        
        // Create the main UI
        egui::CentralPanel::default().show(ctx, |ui| {
            // Add a header with icon and exit button
            ui.horizontal(|ui| {
                // Add the title with larger text
                ui.heading("AudioTheorem 2");
                
                // Add musical keyboard emoji
                ui.label("🎹");
                
                // Add flexible space to push exit button to the right
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("❌ Exit").clicked() {
                        self.should_exit = true;
                    }
                });
            });
            
            ui.add_space(8.0); // Add some space between header and tabs
            
            // Add tabs for different sections
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.current_tab, Tab::Synth, "Synthesizer");
                ui.selectable_value(&mut self.current_tab, Tab::Audio, "Audio Settings");
                ui.selectable_value(&mut self.current_tab, Tab::Midi, "MIDI Settings");
            });
            
            ui.separator(); // Add a separator between tabs and content
            
            // Display the appropriate tab content
            match self.current_tab {
                Tab::Synth => self.render_synth_ui(ui),
                Tab::Audio => self.render_audio_settings(ui),
                Tab::Midi => self.render_midi_settings(ui),
            }
        });
        
        // Check if we should exit
        if self.should_exit {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        
        // Always request a repaint to keep the UI responsive
        ctx.request_repaint();
    }
}

// Add this enum to track the current tab
#[derive(PartialEq)]
enum Tab {
    Synth,
    Audio,
    Midi,
}

impl SynthApp {
    pub fn new() -> Result<Self> {
        println!("Creating SynthApp instance");
        
        // Set up audio
        let host = cpal::default_host();
        println!("Using audio host: {}", host.id().name());
        
        let device = host.default_output_device()
            .ok_or_else(|| anyhow::anyhow!("No output device available"))?;
            
        println!("Using output device: {:?}", device.name());
        
        let config = device.default_output_config()?;
        println!("Device config: {:?}", config);
        
        let sample_format = config.sample_format();
        let config = cpal::StreamConfig::from(config);
        let sample_rate = config.sample_rate.0 as f32;
        println!("Using sample rate: {}", sample_rate);
        
        // Create the synth state first
        let synth = Arc::new(RwLock::new(Synth::new(sample_rate)));
        
        // Then create the message bus with a reference to synth
        let message_bus = MessageBus::new(Arc::clone(&synth));
        
        // Set up audio callback
        let stream = match sample_format {
            SampleFormat::F32 => create_stream::<f32>(&device, &config, Arc::clone(&synth)),
            SampleFormat::I16 => create_stream::<i16>(&device, &config, Arc::clone(&synth)),
            SampleFormat::U16 => create_stream::<u16>(&device, &config, Arc::clone(&synth)),
            _ => anyhow::bail!("Unsupported sample format"),
        }?;
        
        stream.play()?;
        println!("Audio stream started successfully");
        
        // Get output devices - collect first, then count
        let mut available_output_devices = Vec::new();
        let output_devices = host.output_devices()?;
        for device in output_devices {
            available_output_devices.push(device);
        }
        println!("Found {} output devices", available_output_devices.len());
        
        // Get input devices - collect first, then count
        let mut available_input_devices = Vec::new();
        let input_devices = host.input_devices()?;
        for device in input_devices {
            available_input_devices.push(device);
        }
        println!("Found {} input devices", available_input_devices.len());
        
        // Load app settings
        let app_settings = Self::load_app_settings().unwrap_or_default();
        
        // Initialize the SynthApp
        let mut app = SynthApp {
            synth,
            message_bus,
            _stream: Some(stream),
            _midi_connection: None,
            midi_ports: Vec::new(),
            selected_midi_port: 0,
            current_tab: Tab::Synth,
            last_midi_message: None,
            available_output_devices,
            available_input_devices,
            selected_output_device_idx: 0,
            selected_input_device_idx: 0,
            show_sample_dialog: false,
            presets: Vec::new(),
            current_preset_name: String::new(),
            app_settings,
            should_exit: false,
        };
        
        // Refresh MIDI ports after initialization
        app.refresh_midi_ports();
        
        // If there was a previously selected MIDI port, try to reconnect
        if let Some(ref port_name) = app.app_settings.selected_midi_port {
            if let Some(port_idx) = app.midi_ports.iter().position(|p| p == port_name) {
                app.selected_midi_port = port_idx;
                app.connect_midi_port(port_idx);
            }
        }
        
        println!("[MAIN] SynthApp created successfully");
        Ok(app)
    }
    
    fn process_messages(&mut self) {
        // Process any pending messages from the message bus
        while let Ok(msg) = self.message_bus.try_receive() {
            match msg {
                SynthMessage::NoteOn(note, velocity) => {
                    // Handle note on message
                    if let Ok(mut synth) = self.synth.write() {
                        synth.note_on(note, velocity);
                    }
                },
                SynthMessage::NoteOff(note) => {
                    // Handle note off message
                    if let Ok(mut synth) = self.synth.write() {
                        synth.note_off(note);
                    }
                },
                SynthMessage::SetVolume(volume) => {
                    if let Ok(mut synth) = self.synth.write() {
                        synth.volume = volume;
                    }
                },
                // Handle other messages...
                _ => {}
            }
        }
    }

    fn render_synth_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Synthesizer");
        
        // Get available width and calculate per-oscillator width
        let available_width = ui.available_width();
        let osc_width = (available_width - 40.0) / 3.0; // 40.0 accounts for spacing
        
        ui.horizontal(|ui| {
            for i in 0..3 {
                let oscillator = if i < self.synth.read().unwrap().oscillators.len() {
                    &self.synth.read().unwrap().oscillators[i]
                } else {
                    continue; // Skip if oscillator doesn't exist
                };
                
                ui.push_id(format!("oscillator_section_{}", i), |ui| {
                    // Use a vertical with constrained width
                    ui.vertical(|ui| {
                        ui.set_max_width(osc_width);
                        ui.set_min_width(osc_width);
                        ui.heading(format!("Oscillator {}", i+1));
                        
                        // Generate oscillator-specific waveform preview
                        let preview_points = crate::utils::audio_visualizer::generate_waveform_preview(
                            &oscillator.waveform,
                            &self.synth.read().unwrap().custom_wavetables,
                            100 // number of samples
                        );
                        
                        // Waveform visualization with height only
                        crate::ui::components::WaveformPlot::new(preview_points)
                            .height(80.0)
                            .show(ui, "waveform_preview");
                        
                        // Oscillator controls
                        ui.horizontal(|ui| {
                            ui.label("Waveform:");
                            let mut waveform = oscillator.waveform.clone();
                            let waveform_changed = egui::ComboBox::new("waveform_selector", "")
                                .selected_text(format!("{:?}", waveform))
                                .show_ui(ui, |ui| {
                                    use crate::core::oscillator::Waveform;
                                    ui.selectable_value(&mut waveform, Waveform::Sine, "Sine");
                                    ui.selectable_value(&mut waveform, Waveform::Square, "Square");
                                    ui.selectable_value(&mut waveform, Waveform::Saw, "Saw");
                                    ui.selectable_value(&mut waveform, Waveform::Triangle, "Triangle");
                                    ui.selectable_value(&mut waveform, Waveform::WhiteNoise, "White Noise");
                                })
                                .response.changed();
                                    
                            if waveform_changed {
                                if let Ok(mut synth) = self.synth.write() {
                                    synth.oscillators[i].waveform = waveform;
                                }
                            }
                        });
                        
                        // Volume, detune, octave
                        ui.horizontal(|ui| {
                            ui.label("Volume:");
                            let mut volume = oscillator.volume;
                            if ui.add(egui::Slider::new(&mut volume, 0.0..=1.0)).changed() {
                                if let Ok(mut synth) = self.synth.write() {
                                    synth.oscillators[i].volume = volume;
                                }
                            }
                        });
                        
                        ui.horizontal(|ui| {
                            ui.label("Detune:");
                            let mut detune = oscillator.detune;
                            if ui.add(egui::Slider::new(&mut detune, -12.0..=12.0)).changed() {
                                if let Ok(mut synth) = self.synth.write() {
                                    synth.oscillators[i].detune = detune;
                                }
                            }
                        });
                        
                        ui.horizontal(|ui| {
                            ui.label("Octave:");
                            let mut octave = oscillator.octave;
                            if ui.add(egui::Slider::new(&mut octave, -4..=4)).changed() {
                                if let Ok(mut synth) = self.synth.write() {
                                    synth.oscillators[i].octave = octave;
                                }
                            }
                        });
                        
                        // ADSR controls
                        ui.collapsing("Envelope", |ui| {
                            let mut attack = oscillator.attack;
                            let mut decay = oscillator.decay;
                            let mut sustain = oscillator.sustain;
                            let mut release = oscillator.release;
                            
                            let attack_changed = ui.add(egui::Slider::new(&mut attack, 0.01..=2.0).text("Attack")).changed();
                            let decay_changed = ui.add(egui::Slider::new(&mut decay, 0.01..=2.0).text("Decay")).changed();
                            let sustain_changed = ui.add(egui::Slider::new(&mut sustain, 0.0..=1.0).text("Sustain")).changed();
                            let release_changed = ui.add(egui::Slider::new(&mut release, 0.01..=3.0).text("Release")).changed();
                            
                            if attack_changed || decay_changed || sustain_changed || release_changed {
                                if let Ok(mut synth) = self.synth.write() {
                                    synth.oscillators[i].attack = attack;
                                    synth.oscillators[i].decay = decay;
                                    synth.oscillators[i].sustain = sustain;
                                    synth.oscillators[i].release = release;
                                }
                            }
                        });
                        
                        // Filter controls
                        ui.collapsing("Filter", |ui| {
                            let mut filter_type = oscillator.filter_type.clone();
                            let mut filter_cutoff = oscillator.filter_cutoff;
                            let mut filter_resonance = oscillator.filter_resonance;
                            
                            let filter_type_changed = egui::ComboBox::new("filter_type_selector", "Type")
                                .selected_text(format!("{:?}", filter_type))
                                .show_ui(ui, |ui| {
                                    use crate::core::oscillator::FilterType;
                                    ui.selectable_value(&mut filter_type, FilterType::LowPass, "Low Pass");
                                    ui.selectable_value(&mut filter_type, FilterType::HighPass, "High Pass");
                                    ui.selectable_value(&mut filter_type, FilterType::BandPass, "Band Pass");
                                    ui.selectable_value(&mut filter_type, FilterType::Notch, "Notch");
                                })
                                .response.changed();
                                    
                            let cutoff_changed = ui.add(egui::Slider::new(&mut filter_cutoff, 0.01..=1.0).text("Cutoff")).changed();
                            let resonance_changed = ui.add(egui::Slider::new(&mut filter_resonance, 0.01..=1.0).text("Resonance")).changed();
                            
                            if filter_type_changed || cutoff_changed || resonance_changed {
                                if let Ok(mut synth) = self.synth.write() {
                                    synth.oscillators[i].filter_type = filter_type;
                                    synth.oscillators[i].filter_cutoff = filter_cutoff;
                                    synth.oscillators[i].filter_resonance = filter_resonance;
                                }
                            }
                        });
                    });
                });
                
                // Add a small visual separator except after the last oscillator
                if i < 2 {
                    ui.add_space(20.0);
                }
            }
        });
        
        ui.add_space(10.0);
        
        ui.separator();
        
        // Master section below oscillators
        ui.heading("Master");
        
        if let Ok(synth) = self.synth.read() {
            // Generate combined waveform display from all oscillators
            let waveform_points = crate::utils::audio_visualizer::generate_wavetable_display(
                synth.oscillators.as_slice(),
                &synth.oscillator_combination_mode,
                &synth.custom_wavetables
            );
            
            // Display waveform
            crate::ui::components::WaveformPlot::new(waveform_points)
                .height(150.0)
                .color(egui::Color32::from_rgb(0, 188, 212))
                .show(ui, "master_waveform");
            
            // Volume and combination mode controls
            ui.horizontal(|ui| {
                ui.label("Master Volume:");
                let mut volume = synth.volume;
                if ui.add(egui::Slider::new(&mut volume, 0.0..=1.0)).changed() {
                    if let Ok(mut synth) = self.synth.write() {
                        synth.volume = volume;
                    }
                }
                
                ui.label("Combination Mode:");
                let mut current_mode = synth.oscillator_combination_mode.clone();
                egui::ComboBox::new("combination_mode", "")
                    .selected_text(format!("{:?}", current_mode))
                    .show_ui(ui, |ui| {
                        use crate::core::oscillator::OscillatorCombinationMode;
                        ui.selectable_value(&mut current_mode, OscillatorCombinationMode::Parallel, "Parallel");
                        ui.selectable_value(&mut current_mode, OscillatorCombinationMode::FM, "FM");
                        ui.selectable_value(&mut current_mode, OscillatorCombinationMode::AM, "AM");
                        ui.selectable_value(&mut current_mode, OscillatorCombinationMode::RingMod, "Ring Mod");
                        ui.selectable_value(&mut current_mode, OscillatorCombinationMode::Filter, "Filter");
                    });
                    
                if current_mode != synth.oscillator_combination_mode {
                    if let Ok(mut synth) = self.synth.write() {
                        synth.oscillator_combination_mode = current_mode;
                    }
                }
            });
            
            // Master ADSR controls
            ui.collapsing("Master Envelope", |ui| {
                let mut attack = synth.attack;
                let mut decay = synth.decay;
                let mut sustain = synth.sustain;
                let mut release = synth.release;
                
                let attack_changed = ui.add(egui::Slider::new(&mut attack, 0.01..=2.0).text("Attack")).changed();
                let decay_changed = ui.add(egui::Slider::new(&mut decay, 0.01..=2.0).text("Decay")).changed();
                let sustain_changed = ui.add(egui::Slider::new(&mut sustain, 0.0..=1.0).text("Sustain")).changed();
                let release_changed = ui.add(egui::Slider::new(&mut release, 0.01..=3.0).text("Release")).changed();
                
                if attack_changed || decay_changed || sustain_changed || release_changed {
                    if let Ok(mut synth) = self.synth.write() {
                        synth.attack = attack;
                        synth.decay = decay;
                        synth.sustain = sustain;
                        synth.release = release;
                    }
                }
            });
            
            // Master filter controls
            ui.collapsing("Master Filter", |ui| {
                let mut filter_type = synth.master_filter_type.clone();
                let mut filter_cutoff = synth.master_filter_cutoff;
                let mut filter_resonance = synth.master_filter_resonance;
                
                let filter_type_changed = egui::ComboBox::new("master_filter_type", "Type")
                    .selected_text(format!("{:?}", filter_type))
                    .show_ui(ui, |ui| {
                        use crate::core::oscillator::FilterType;
                        ui.selectable_value(&mut filter_type, FilterType::LowPass, "Low Pass");
                        ui.selectable_value(&mut filter_type, FilterType::HighPass, "High Pass");
                        ui.selectable_value(&mut filter_type, FilterType::BandPass, "Band Pass");
                        ui.selectable_value(&mut filter_type, FilterType::Notch, "Notch");
                    })
                    .response.changed();
                    
                let cutoff_changed = ui.add(egui::Slider::new(&mut filter_cutoff, 0.01..=1.0).text("Cutoff")).changed();
                let resonance_changed = ui.add(egui::Slider::new(&mut filter_resonance, 0.01..=1.0).text("Resonance")).changed();
                
                if filter_type_changed || cutoff_changed || resonance_changed {
                    if let Ok(mut synth) = self.synth.write() {
                        synth.master_filter_type = filter_type;
                        synth.master_filter_cutoff = filter_cutoff;
                        synth.master_filter_resonance = filter_resonance;
                    }
                }
            });
        }
        
        // Preset controls at the bottom
        ui.separator();
        ui.collapsing("Presets", |ui| {
            ui.horizontal(|ui| {
                ui.label("Preset Name:");
                ui.text_edit_singleline(&mut self.current_preset_name);
            });
            
            ui.horizontal(|ui| {
                if ui.button("Save Preset").clicked() && !self.current_preset_name.is_empty() {
                    self.save_preset(self.current_preset_name.clone()).ok();
                }
                
                if ui.button("Load Selected").clicked() && !self.current_preset_name.is_empty() {
                    let preset_name = self.current_preset_name.clone();
                    self.load_preset(&preset_name).ok();
                }
            });
            
            ui.separator();
            for preset in &self.presets {
                if ui.selectable_label(self.current_preset_name == preset.name, &preset.name).clicked() {
                    self.current_preset_name = preset.name.clone();
                }
            }
        });
    }
    
    fn render_audio_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("Audio Settings");
        
        // Output device selector
        ui.group(|ui| {
            ui.label("Output Device:");
            
            for (i, device) in self.available_output_devices.iter().enumerate() {
                if let Ok(name) = device.name() {
                    let name_str = name.clone();
                    if ui.radio_value(&mut self.selected_output_device_idx, i, &name_str).clicked() {
                        self.app_settings.selected_output_device = Some(name_str);
                        self.save_app_settings().ok();
                    }
                }
            }
            
            if ui.button("Refresh Devices").clicked() {
                // Refresh the device list
                let host = cpal::default_host();
                
                self.available_output_devices.clear();
                if let Ok(devices) = host.output_devices() {
                    for device in devices {
                        self.available_output_devices.push(device);
                    }
                }
            }
        });
        
        // Master volume control
        ui.horizontal(|ui| {
            ui.label("Master Volume:");
            let mut volume = self.app_settings.volume;
            if ui.add(egui::Slider::new(&mut volume, 0.0..=1.0)).changed() {
                self.app_settings.volume = volume;
                self.message_bus.send(SynthMessage::SetVolume(volume));
                self.save_app_settings().ok();
            }
        });
    }
    
    fn render_midi_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("MIDI Settings");
        
        // Basic MIDI port selection
        ui.horizontal(|ui| {
            ui.label("MIDI Input Device:");
            
            if ui.button("Refresh MIDI Ports").clicked() {
                self.refresh_midi_ports();
            }
        });
        
        // MIDI port list as buttons to avoid borrowing issues
        for i in 0..self.midi_ports.len() {
            let port_name = self.midi_ports[i].clone();
            let is_selected = i == self.selected_midi_port;
            
            if ui.radio(is_selected, &port_name).clicked() && !is_selected {
                self.selected_midi_port = i;
                self.connect_midi_port(i);
                self.app_settings.selected_midi_port = Some(port_name);
                self.save_app_settings().ok();
            }
        }
        
        // Show last MIDI message if available
        if let Some(msg) = &self.last_midi_message {
            ui.label(format!("Last MIDI message: {}", msg));
        }
    }
    
    fn refresh_midi_ports(&mut self) {
        self.midi_ports.clear();
        
        if let Ok(midi_in) = midir::MidiInput::new("midi-input") {
            for port in midi_in.ports() {
                if let Ok(port_name) = midi_in.port_name(&port) {
                    self.midi_ports.push(port_name);
                }
            }
        }
    }
    
    fn connect_midi_port(&mut self, port_idx: usize) {
        // Disconnect existing connection if any
        self._midi_connection = None;
        
        // Create a new MIDI input connection
        let midi_in = match midir::MidiInput::new("midi-input") {
            Ok(midi_in) => midi_in,
            Err(e) => {
                println!("Error creating MIDI input: {}", e);
                return;
            }
        };
        
        // Get ports
        let ports = midi_in.ports();
        if port_idx >= ports.len() {
            println!("Invalid MIDI port index");
            return;
        }
        
        let port = &ports[port_idx];
        
        // Clone the sender for the callback
        let message_sender = self.message_bus.sender();
        
        // Connect to the port
        match midi_in.connect(port, "midi-connection", move |_stamp, message, _| {
            // Process MIDI messages
            if message.len() >= 3 {
                let status = message[0];
                let data1 = message[1];
                let data2 = message[2];
                
                match status & 0xF0 {
                    0x90 => {
                        // Note On
                        if data2 > 0 {
                            message_sender.send(SynthMessage::NoteOn(data1, data2)).ok();
                        } else {
                            message_sender.send(SynthMessage::NoteOff(data1)).ok();
                        }
                    },
                    0x80 => {
                        // Note Off
                        message_sender.send(SynthMessage::NoteOff(data1)).ok();
                    },
                    0xB0 => {
                        // Control Change
                        match data1 {
                            1 => {
                                // Modulation wheel
                                let value = data2 as f32 / 127.0;
                                message_sender.send(SynthMessage::SetModulation(value)).ok();
                            },
                            7 => {
                                // Volume
                                let value = data2 as f32 / 127.0;
                                message_sender.send(SynthMessage::SetVolume(value)).ok();
                            },
                            64 => {
                                // Sustain pedal
                                let on = data2 >= 64;
                                message_sender.send(SynthMessage::SetSustainPedal(on)).ok();
                            },
                            // Add more CC handlers as needed
                            _ => {}
                        }
                    },
                    // Add more MIDI message handling as needed
                    _ => {}
                }
            }
        }, ()) {
            Ok(conn) => {
                println!("Connected to MIDI device");
                self._midi_connection = Some(conn);
                self.last_midi_message = Some("Connected".to_string());
            },
            Err(err) => {
                println!("Failed to connect to MIDI device: {}", err);
            }
        }
    }

    fn save_preset(&mut self, name: String) -> Result<()> {
        if let Ok(synth) = self.synth.read() {
            let preset = synth.create_preset(&name, "User", "Custom preset");
            
            // Check if preset with this name already exists
            if let Some(pos) = self.presets.iter().position(|p| p.name == name) {
                // Replace existing preset
                self.presets[pos] = preset.clone();
            } else {
                // Add new preset
                self.presets.push(preset);
            }
            
            // Save presets to file
            self.save_presets_to_file()?;
            
            // Update current preset name
            self.current_preset_name = name;
            
            // Update app settings
            self.app_settings.last_preset = Some(self.current_preset_name.clone());
            self.save_app_settings()?;
        }
        
        Ok(())
    }
    
    fn load_preset(&mut self, name: &str) -> Result<()> {
        if let Some(preset) = self.presets.iter().find(|p| p.name == name) {
            if let Ok(mut synth) = self.synth.write() {
                synth.apply_preset(preset);
                
                // Update current preset name
                self.current_preset_name = name.to_string();
                
                // Update app settings
                self.app_settings.last_preset = Some(self.current_preset_name.clone());
                self.save_app_settings()?;
            }
        }
        
        Ok(())
    }
    
    fn delete_preset(&mut self, name: &str) -> Result<()> {
        if let Some(pos) = self.presets.iter().position(|p| p.name == name) {
            self.presets.remove(pos);
            
            // Save presets to file
            self.save_presets_to_file()?;
            
            // If we deleted the current preset, clear the current preset name
            if self.current_preset_name == name {
                self.current_preset_name = String::new();
                self.app_settings.last_preset = None;
                self.save_app_settings()?;
            }
        }
        
        Ok(())
    }
    
    fn save_presets_to_file(&self) -> Result<()> {
        // Create presets directory if it doesn't exist
        let presets_dir = Self::get_presets_dir()?;
        fs::create_dir_all(&presets_dir)?;
        
        // Save each preset to a separate file
        for preset in &self.presets {
            let preset_path = presets_dir.join(format!("{}.json", preset.name));
            let preset_json = serde_json::to_string_pretty(preset)?;
            let mut file = File::create(preset_path)?;
            file.write_all(preset_json.as_bytes())?;
        }
        
        Ok(())
    }
    
    fn save_app_settings(&self) -> Result<()> {
        let settings_dir = Self::get_settings_dir()?;
        fs::create_dir_all(&settings_dir)?;
        
        let path = settings_dir.join("settings.json");
        let file = File::create(path)?;
        
        serde_json::to_writer_pretty(file, &self.app_settings)?;
        Ok(())
    }
    
    fn get_settings_dir() -> Result<PathBuf> {
        let mut path = dirs::config_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not find config directory"))?;
        path.push("audiotheorem2");
        Ok(path)
    }
    
    fn get_presets_dir() -> Result<PathBuf> {
        let mut path = Self::get_settings_dir()?;
        path.push("presets");
        Ok(path)
    }

    fn load_app_settings() -> Result<AppSettings> {
        let path = Self::get_settings_dir()?.join("settings.json");
        if path.exists() {
            let file = File::open(path)?;
            Ok(serde_json::from_reader(file)?)
        } else {
            Ok(AppSettings::default())
        }
    }
}

fn create_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    synth: Arc<RwLock<Synth>>,
) -> Result<Stream>
where
    T: Sample + Send + 'static + cpal::SizedSample + cpal::FromSample<f32>,
{
    let config = config.clone();
    let channels = config.channels as usize;
    let err_fn = |err| eprintln!("an error occurred on the audio stream: {}", err);
    
    let stream = device.build_output_stream(
        &config,
        move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
            // Simple audio callback that won't deadlock
            for frame in data.chunks_mut(channels) {
                let value = match synth.write() {
                    Ok(mut guard) => guard.get_sample(1.0 / 44100.0),
                    Err(_) => 0.0,
                };
                
                let value_t = T::from_sample(value);
                
                for sample in frame.iter_mut() {
                    *sample = value_t;
                }
            }
        },
        err_fn,
        None,
    )?;
    
    Ok(stream)
}


// App settings structure
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct AppSettings {
    selected_midi_port: Option<String>,
    selected_output_device: Option<String>,
    selected_input_device: Option<String>,
    volume: f32,
    last_preset: Option<String>,
}
