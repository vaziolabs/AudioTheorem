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
use crate::oscillator::{CustomWavetable, FilterType, ModulationTarget, OscillatorCombinationMode, Waveform};
use crate::synth::{Synth, SynthPreset, WAVEFORM_DISPLAY_POINTS};
use egui_plot::{Plot, Line, PlotPoints};

// Message types for our threaded architecture
enum SynthMessage {
    NoteOn(u8, u8),
    NoteOff(u8),
    ChangeOscillator(usize, Waveform, f32, f32, i8), // (osc_index, waveform, volume, detune, octave)
    ChangeOscillatorEnvelope(usize, f32, f32, f32, f32), // (osc_index, attack, decay, sustain, release)
    ChangeOscillatorFilter(usize, FilterType, f32, f32), // (osc_index, filter_type, cutoff, resonance)
    ChangeOscillatorModulation(usize, f32, ModulationTarget), // (osc_index, amount, target)
    ChangeOscillatorCombinationMode(OscillatorCombinationMode),
    ChangeMasterEnvelope(f32, f32, f32, f32), // (attack, decay, sustain, release)
    ChangeMasterFilter(FilterType, f32, f32), // (filter_type, cutoff, resonance)
    ChangeVolume(f32),
    LoadSample(PathBuf),
    SetVolume(f32),
    SetModulation(f32),
    SetSustainPedal(bool),
    SetPitchBend(f32),
    SetAftertouch(u8, f32),
    SetChannelPressure(f32),
}

// Main app state
pub struct SynthApp {
    synth: Arc<RwLock<Synth>>,
    sender: crossbeam_channel::Sender<SynthMessage>,
    receiver: crossbeam_channel::Receiver<SynthMessage>,
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
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
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
        
        // Create a channel for message passing
        let (sender, receiver) = crossbeam_channel::unbounded();
        
        // Create the synth state
        let synth = Arc::new(RwLock::new(Synth::new(sample_rate)));
        
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
        
        println!("[MAIN] SynthApp created successfully");
        
        // Load app settings
        let mut app = SynthApp {
            synth,
            sender,
            receiver,
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
            app_settings: AppSettings {
                selected_midi_port: None,
                selected_output_device: None,
                selected_input_device: None,
                volume: 0.5,
                last_preset: None,
            },
            should_exit: false,
        };
        
        if let Ok(settings) = Self::load_app_settings() {
            app.selected_midi_port = settings.selected_midi_port
                .and_then(|name| app.midi_ports.iter().position(|p| p == &name))
                .unwrap_or(0);
            
            app.selected_output_device_idx = settings.selected_output_device
                .and_then(|name| app.available_output_devices.iter().position(|d| d.name().ok() == Some(name.clone())))
                .unwrap_or(0);
            
            app.app_settings.volume = settings.volume;
            
            // Load last preset if available
            if let Some(preset_name) = settings.last_preset {
                app.load_preset(&preset_name).ok();
            }
        }
        
        Ok(app)
    }
    
    fn process_messages(&mut self) {
        // Process a limited number of messages per frame
        const MAX_MESSAGES_PER_FRAME: usize = 10;
        let mut count = 0;
        
        while let Ok(msg) = self.receiver.try_recv() {
            match msg {
                SynthMessage::NoteOn(note, velocity) => {
                    if let Ok(mut synth) = self.synth.write() {
                        synth.note_on(note, velocity);
                    }
                },
                SynthMessage::NoteOff(note) => {
                    if let Ok(mut synth) = self.synth.write() {
                        synth.note_off(note);
                    }
                },
                SynthMessage::ChangeOscillator(index, waveform, volume, detune, octave) => {
                    if let Ok(mut synth) = self.synth.write() {
                        if index < synth.oscillators.len() {
                            synth.oscillators[index].waveform = waveform;
                            synth.oscillators[index].volume = volume;
                            synth.oscillators[index].detune = detune;
                            synth.oscillators[index].octave = octave;
                        }
                    }
                },
                SynthMessage::ChangeOscillatorEnvelope(index, attack, decay, sustain, release) => {
                    if let Ok(mut synth) = self.synth.write() {
                        if index < synth.oscillators.len() {
                            synth.oscillators[index].attack = attack;
                            synth.oscillators[index].decay = decay;
                            synth.oscillators[index].sustain = sustain;
                            synth.oscillators[index].release = release;
                        }
                    }
                },
                SynthMessage::ChangeOscillatorFilter(index, filter_type, cutoff, resonance) => {
                    if let Ok(mut synth) = self.synth.write() {
                        if index < synth.oscillators.len() {
                            synth.oscillators[index].filter_type = filter_type;
                            synth.oscillators[index].filter_cutoff = cutoff;
                            synth.oscillators[index].filter_resonance = resonance;
                        }
                    }
                },
                SynthMessage::ChangeOscillatorModulation(index, amount, target) => {
                    if let Ok(mut synth) = self.synth.write() {
                        if index < synth.oscillators.len() {
                            synth.oscillators[index].mod_amount = amount;
                            synth.oscillators[index].mod_target = target;
                        }
                    }
                },
                SynthMessage::ChangeOscillatorCombinationMode(mode) => {
                    if let Ok(mut synth) = self.synth.write() {
                        synth.oscillator_combination_mode = mode;
                    }
                },
                SynthMessage::ChangeMasterEnvelope(attack, decay, sustain, release) => {
                    if let Ok(mut synth) = self.synth.write() {
                        synth.attack = attack;
                        synth.decay = decay;
                        synth.sustain = sustain;
                        synth.release = release;
                    }
                },
                SynthMessage::ChangeMasterFilter(filter_type, cutoff, resonance) => {
                    if let Ok(mut synth) = self.synth.write() {
                        synth.master_filter_type = filter_type;
                        synth.master_filter_cutoff = cutoff;
                        synth.master_filter_resonance = resonance;
                    }
                },
                SynthMessage::SetVolume(volume) => {
                    if let Ok(mut synth) = self.synth.write() {
                        synth.volume = volume;
                    }
                },
                SynthMessage::SetModulation(modulation) => {
                    if let Ok(mut synth) = self.synth.write() {
                        for oscillator in &mut synth.oscillators {
                            oscillator.mod_amount = modulation;
                        }
                    }
                },
                SynthMessage::SetSustainPedal(on) => {
                    if let Ok(mut synth) = self.synth.write() {
                        for oscillator in &mut synth.oscillators {
                            oscillator.sustain = if on { 1.0 } else { 0.0 };
                        }
                    }
                },
                SynthMessage::SetPitchBend(bend) => {
                    if let Ok(mut synth) = self.synth.write() {
                        for oscillator in &mut synth.oscillators {
                            oscillator.pitch_bend = bend;
                        }
                    }
                },
                SynthMessage::SetAftertouch(note, pressure) => {
                    if let Ok(mut synth) = self.synth.write() {
                        for oscillator in &mut synth.oscillators {
                            if oscillator.note == Some(note) {
                                oscillator.aftertouch = pressure;
                            }
                        }
                    }
                },
                SynthMessage::SetChannelPressure(pressure) => {
                    if let Ok(mut synth) = self.synth.write() {
                        for oscillator in &mut synth.oscillators {
                            oscillator.channel_pressure = pressure;
                        }
                    }
                },
                SynthMessage::LoadSample(path) => {
                    if let Ok(mut synth) = self.synth.write() {
                        if let Err(e) = synth.load_sample(path) {
                            eprintln!("Error loading sample: {}", e);
                        }
                    }
                },
                _ => {}
            }
            
            count += 1;
            if count >= MAX_MESSAGES_PER_FRAME {
                break;
            }
        }
    }

    fn render_synth_ui(&mut self, ui: &mut egui::Ui) {
        // First, get a read-only view of the current state
        let synth_view = self.synth.read().unwrap();
        
        // Create temporary variables to hold UI state
        let mut temp_oscillators: Vec<_> = synth_view.oscillators.iter().map(|osc| {
            (
                osc.waveform.clone(),
                osc.volume,
                osc.detune,
                osc.octave
            )
        }).collect();
        
        ui.columns(3, |columns| {
            for (i, col) in columns.iter_mut().enumerate() {
                col.vertical(|ui| {
                    // Push a unique ID for this oscillator's UI elements
                    ui.push_id(i, |ui| {
                        ui.heading(format!("Oscillator {}", i + 1));
                        
                        // Generate oscillator-specific waveform preview
                        let preview_points = crate::visualizer::generate_waveform_preview(
                            &temp_oscillators[i].0,
                            temp_oscillators[i].2,
                            temp_oscillators[i].3,
                            &synth_view.custom_wavetables
                        );
                        
                        // Waveform visualization
                        let plot = Plot::new(format!("osc_{}", i))
                            .height(100.0)
                            .show_x(false)
                            .show_y(false);
                        
                        plot.show(ui, |plot_ui| {
                            plot_ui.line(Line::new(PlotPoints::from_iter(
                                preview_points.iter().map(|[x, y]| [*x as f64, *y as f64])
                            )).color(egui::Color32::from_rgb(0, 188, 212)));
                        });

                        // Waveform selector
                        let mut current_waveform = temp_oscillators[i].0.clone();
                        egui::ComboBox::from_label("Waveform")
                            .selected_text(format!("{:?}", current_waveform))
                            .show_ui(ui, |ui| {
                                for waveform in &[Waveform::Sine, Waveform::Square, 
                                               Waveform::Saw, Waveform::Triangle, 
                                               Waveform::WhiteNoise] {
                                    if ui.selectable_value(&mut current_waveform, waveform.clone(), 
                                                          format!("{:?}", waveform)).changed() {
                                        // Send message to update the actual synth
                                        self.sender.send(SynthMessage::ChangeOscillator(
                                            i, 
                                            waveform.clone(),
                                            temp_oscillators[i].1,
                                            temp_oscillators[i].2,
                                            temp_oscillators[i].3
                                        )).ok();
                                    }
                                }
                            });
                        temp_oscillators[i].0 = current_waveform;

                        // Volume control
                        let mut volume = temp_oscillators[i].1;
                        if ui.add(egui::Slider::new(&mut volume, 0.0..=1.0).text("Volume")).changed() {
                            temp_oscillators[i].1 = volume;
                            self.sender.send(SynthMessage::ChangeOscillator(
                                i, 
                                temp_oscillators[i].0.clone(),
                                volume,
                                temp_oscillators[i].2,
                                temp_oscillators[i].3
                            )).ok();
                        }
                        
                        // Detune control
                        let mut detune = temp_oscillators[i].2;
                        if ui.add(egui::Slider::new(&mut detune, -12.0..=12.0).text("Detune")).changed() {
                            temp_oscillators[i].2 = detune;
                            self.sender.send(SynthMessage::ChangeOscillator(
                                i, 
                                temp_oscillators[i].0.clone(),
                                temp_oscillators[i].1,
                                detune,
                                temp_oscillators[i].3
                            )).ok();
                        }
                        
                        // Octave control
                        let mut octave = temp_oscillators[i].3;
                        if ui.add(egui::Slider::new(&mut octave, -2..=2).text("Octave")).changed() {
                            temp_oscillators[i].3 = octave;
                            self.sender.send(SynthMessage::ChangeOscillator(
                                i, 
                                temp_oscillators[i].0.clone(),
                                temp_oscillators[i].1,
                                temp_oscillators[i].2,
                                octave
                            )).ok();
                        }
                    });
                });
            }
        });

        // Master visualization
        ui.separator();
        ui.heading("Master Output");
        
        let master_points = synth_view.generate_waveform_display();
        let plot = Plot::new("master_waveform")
            .height(150.0)
            .show_x(false)
            .show_y(false);
        
        plot.show(ui, |plot_ui| {
            plot_ui.line(Line::new(PlotPoints::from_iter(
                master_points.iter().map(|[x, y]| [*x as f64, *y as f64])
            )).color(egui::Color32::from_rgb(0, 188, 212)));
        });
    }
    
    // Add this helper method to generate oscillator preview
    fn generate_oscillator_preview(&self, waveform: &Waveform, detune: f32, octave: i8, 
                                  custom_wavetables: &[CustomWavetable]) -> Vec<[f32; 2]> {
        const PREVIEW_POINTS: usize = 100;
        let mut points = Vec::with_capacity(PREVIEW_POINTS);
        
        // Apply octave shift and detune to the phase
        let octave_factor = 2.0f32.powf(octave as f32);
        let detune_factor = 2.0f32.powf(detune / 12.0);
        let frequency_factor = octave_factor * detune_factor;
        
        for i in 0..PREVIEW_POINTS {
            let phase = i as f32 / PREVIEW_POINTS as f32;
            let mod_phase = (phase * frequency_factor) % 1.0;
            
            // Get waveform value based on the oscillator's waveform type
            let value = match waveform {
                Waveform::Sine => (2.0 * std::f32::consts::PI * mod_phase).sin(),
                Waveform::Square => if mod_phase < 0.5 { 1.0 } else { -1.0 },
                Waveform::Saw => 2.0 * mod_phase - 1.0,
                Waveform::Triangle => {
                    if mod_phase < 0.25 {
                        4.0 * mod_phase
                    } else if mod_phase < 0.75 {
                        2.0 - 4.0 * mod_phase
                    } else {
                        -4.0 + 4.0 * mod_phase
                    }
                },
                Waveform::WhiteNoise => {
                    // For visualization, use a deterministic "random" function
                    let seed = (i * 100) as f32;
                    (seed.sin() * 12.5).sin()
                },
                Waveform::CustomSample(index) => {
                    if let Some(wavetable) = custom_wavetables.get(*index) {
                        let sample_pos = mod_phase * wavetable.samples.len() as f32;
                        let index = sample_pos.floor() as usize % wavetable.samples.len();
                        wavetable.samples[index]
                    } else {
                        0.0
                    }
                }
            };
            
            points.push([phase, value]);
        }
        
        points
    }
    
    fn render_audio_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("Audio Device Settings");
        
        // Output device selection
        ui.label("Output Device:");
        
        if self.available_output_devices.is_empty() {
            ui.label("No output devices available");
        } else {
            egui::ComboBox::new("output_device_selector", "Select Output")
                .selected_text(self.available_output_devices
                    .get(self.selected_output_device_idx)
                    .and_then(|d| d.name().ok())
                    .unwrap_or_else(|| "No device".to_string()))
                .show_ui(ui, |ui| {
                    for (idx, device) in self.available_output_devices.iter().enumerate() {
                        if let Ok(name) = device.name() {
                            ui.selectable_value(&mut self.selected_output_device_idx, idx, name);
                        } else {
                            ui.selectable_value(&mut self.selected_output_device_idx, idx, format!("Device {}", idx));
                        }
                    }
                });
                
            if ui.button("Apply Audio Device Changes").clicked() {
                self.change_audio_devices();
                self.save_app_settings().ok();
            }
        }
    }
    
    fn render_midi_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("MIDI Settings");
        
        // MIDI device selection
        ui.label("MIDI Input Device:");
        
        if self.midi_ports.is_empty() {
            ui.label("No MIDI devices available");
            if ui.button("Refresh MIDI Devices").clicked() {
                self.refresh_midi_devices();
            }
        } else {
            egui::ComboBox::new("midi_port_selector", "Select MIDI Port")
                .selected_text(self.midi_ports.get(self.selected_midi_port)
                .cloned()
                .unwrap_or_else(|| "No port".to_string()))
                .show_ui(ui, |ui| {
                    for (idx, port_name) in self.midi_ports.iter().enumerate() {
                        ui.selectable_value(&mut self.selected_midi_port, idx, port_name);
                    }
                });
                
            if ui.button("Connect MIDI Device").clicked() {
                self.connect_midi(self.selected_midi_port);
                self.save_app_settings().ok();
            }
        }
        
        // Display last MIDI message received
        if let Some(msg) = &self.last_midi_message {
            ui.label(format!("Last MIDI message: {}", msg));
        }
    }

    fn change_audio_devices(&mut self) {
        println!("Changing audio devices");
        
        // Get the selected output device
        if self.selected_output_device_idx < self.available_output_devices.len() {
            let device = &self.available_output_devices[self.selected_output_device_idx];
            
            // Get the device configuration
            match device.default_output_config() {
                Ok(config) => {
                    let sample_format = config.sample_format();
                    let config = cpal::StreamConfig::from(config);
                    let sample_rate = config.sample_rate.0 as f32;
                    
                    // Update the synth's sample rate
                    if let Ok(mut synth) = self.synth.write() {
                        synth.sample_rate = sample_rate;
                    }
                    
                    // Create a new audio stream
                    let synth_clone = Arc::clone(&self.synth);
                    let stream_result = match sample_format {
                        SampleFormat::F32 => create_stream::<f32>(device, &config, synth_clone),
                        SampleFormat::I16 => create_stream::<i16>(device, &config, synth_clone),
                        SampleFormat::U16 => create_stream::<u16>(device, &config, synth_clone),
                        _ => Err(anyhow::anyhow!("Unsupported sample format")),
                    };
                    
                    // Replace the old stream with the new one
                    match stream_result {
                        Ok(stream) => {
                            // Stop the old stream if it exists
                            if let Some(old_stream) = self._stream.take() {
                                drop(old_stream);
                            }
                            
                            // Start the new stream
                            if let Err(err) = stream.play() {
                                println!("Failed to play stream: {}", err);
                            } else {
                                self._stream = Some(stream);
                                println!("Audio device changed successfully");
                            }
                        },
                        Err(err) => {
                            println!("Failed to create stream: {}", err);
                        }
                    }
                },
                Err(err) => {
                    println!("Failed to get device config: {}", err);
                }
            }
        }
    }
    
    fn refresh_midi_devices(&mut self) {
        println!("Refreshing MIDI devices");
        
        // Create a new MIDI input
        let midi_in = match midir::MidiInput::new("rust-synth-midi") {
            Ok(midi_in) => midi_in,
            Err(err) => {
                println!("Failed to create MIDI input: {}", err);
                return;
            }
        };
        
        // Get the available ports
        let ports = midi_in.ports();
        
        // Get the names of the ports
        self.midi_ports.clear();
        for port in ports {
            if let Ok(name) = midi_in.port_name(&port) {
                self.midi_ports.push(name);
            } else {
                self.midi_ports.push(format!("Unknown port {}", self.midi_ports.len()));
            }
        }
        
        // Reset the selected port if needed
        if !self.midi_ports.is_empty() && self.selected_midi_port >= self.midi_ports.len() {
            self.selected_midi_port = 0;
        }
        
        println!("Found {} MIDI devices", self.midi_ports.len());
    }
    
    fn connect_midi(&mut self, port_idx: usize) {
        println!("Connecting to MIDI device {}", port_idx);
        
        // Disconnect any existing connection
        self._midi_connection = None;
        
        // Check if the port index is valid
        if port_idx >= self.midi_ports.len() {
            println!("Invalid MIDI port index");
            return;
        }
        
        // Create a new MIDI input
        let mut midi_in = match midir::MidiInput::new("rust-synth-midi") {
            Ok(midi_in) => midi_in,
            Err(err) => {
                println!("Failed to create MIDI input: {}", err);
                return;
            }
        };
        
        // Configure the MIDI input
        midi_in.ignore(midir::Ignore::None);
        
        // Get the port
        let ports = midi_in.ports();
        if port_idx >= ports.len() {
            println!("MIDI port index out of range");
            return;
        }
        
        let port = &ports[port_idx];
        
        // Clone the sender for the callback
        let sender = self.sender.clone();
        
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
                            sender.send(SynthMessage::NoteOn(data1, data2)).ok();
                        } else {
                            sender.send(SynthMessage::NoteOff(data1)).ok();
                        }
                    },
                    0x80 => {
                        // Note Off
                        sender.send(SynthMessage::NoteOff(data1)).ok();
                    },
                    0xB0 => {
                        // Control Change
                        match data1 {
                            1 => {
                                // Modulation wheel
                                // Map 0-127 to 0.0-1.0
                                let value = data2 as f32 / 127.0;
                                sender.send(SynthMessage::SetModulation(value)).ok();
                            },
                            7 => {
                                // Volume
                                let value = data2 as f32 / 127.0;
                                sender.send(SynthMessage::SetVolume(value)).ok();
                            },
                            64 => {
                                // Sustain pedal
                                let on = data2 >= 64;
                                sender.send(SynthMessage::SetSustainPedal(on)).ok();
                            },
                            // Add more CC handlers as needed
                            _ => {}
                        }
                    },
                    0xE0 => {
                        // Pitch Bend
                        // Combine the two 7-bit values into one 14-bit value
                        let bend_value = ((data2 as u16) << 7) | (data1 as u16);
                        // Map from 0-16383 to -1.0 to 1.0
                        let normalized = (bend_value as f32 / 8192.0) - 1.0;
                        sender.send(SynthMessage::SetPitchBend(normalized)).ok();
                    },
                    0xA0 => {
                        // Aftertouch (Key Pressure)
                        let note = data1;
                        let pressure = data2 as f32 / 127.0;
                        sender.send(SynthMessage::SetAftertouch(note, pressure)).ok();
                    },
                    0xD0 => {
                        // Channel Pressure
                        let pressure = data1 as f32 / 127.0;
                        sender.send(SynthMessage::SetChannelPressure(pressure)).ok();
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

    // Add a method to save a preset
    fn save_preset(&mut self, name: String) -> Result<()> {
        let synth = self.synth.read().unwrap();
        let preset = synth.create_preset(name.clone());
        
        // Check if preset with this name already exists
        if let Some(pos) = self.presets.iter().position(|p| p.name == name) {
            // Replace existing preset
            self.presets[pos] = preset.clone();
        } else {
            // Add new preset
            self.presets.push(preset.clone());
        }
        
        // Save presets to file
        self.save_presets_to_file()?;
        
        // Update current preset name
        self.current_preset_name = name;
        
        // Update app settings
        self.app_settings.last_preset = Some(self.current_preset_name.clone());
        self.save_app_settings()?;
        
        Ok(())
    }
    
    // Add a method to load a preset
    fn load_preset(&mut self, name: &str) -> Result<()> {
        if let Some(preset) = self.presets.iter().find(|p| p.name == name) {
            let mut synth = self.synth.write().unwrap();
            synth.apply_preset(preset);
            
            // Update current preset name
            self.current_preset_name = name.to_string();
            
            // Update app settings
            self.app_settings.last_preset = Some(self.current_preset_name.clone());
            self.save_app_settings()?;
        }
        
        Ok(())
    }
    
    // Add a method to delete a preset
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
    
    // Add a method to save presets to file
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
        
        let settings = AppSettings {
            selected_midi_port: self.midi_ports.get(self.selected_midi_port).cloned(),
            selected_output_device: self.available_output_devices
                .get(self.selected_output_device_idx)
                .and_then(|d| d.name().ok()),
            volume: self.app_settings.volume,
            last_preset: Some(self.current_preset_name.clone()),
            ..AppSettings::default()
        };
        
        serde_json::to_writer_pretty(file, &settings)?;
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
