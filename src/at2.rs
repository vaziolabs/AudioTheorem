use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Sample, SampleFormat, Stream};
use eframe::egui;
use egui_plot as plot;
use midir::{Ignore, MidiInput};
use std::sync::{Arc, RwLock};
use std::thread;
use std::path::PathBuf;
use std::collections::VecDeque;
use std::time::Instant;
use hound; // For WAV file loading
use rfd::FileDialog; // For file dialogs
use egui_plot::{Plot, PlotPoints, Line};
use std::f32::consts::PI;

// Constants
const SAMPLE_BUFFER_SIZE: usize = 1024;
const WAVEFORM_DISPLAY_POINTS: usize = 200;
const MAX_CUSTOM_WAVETABLES: usize = 8;

// Types of waveforms
#[derive(Debug, Clone, PartialEq)]
enum Waveform {
    Sine,
    Square,
    Saw,
    Triangle,
    WhiteNoise,
    CustomSample(usize), // Index into the custom wavetables
}

// Note state for envelope
#[derive(Debug, Clone, Copy, PartialEq)]
enum NoteState {
    Attack,
    Decay,
    Sustain,
    Release,
}

// A custom wavetable loaded from a sample
struct CustomWavetable {
    name: String,
    samples: Vec<f32>,
    sample_rate: u32,
}

// Structure representing a single note
#[derive(Debug, Clone)]
struct Note {
    midi_note: u8,
    frequency: f32,
    phase: f32,
    phase_increment: f32,
    velocity: f32,
    state: NoteState,
    time_in_state: f32,
}

// Current analyzer state
struct Analyzer {
    current_waveform_samples: VecDeque<f32>,
    fft_buffer: Vec<f32>,
    last_update: Instant,
}

// Main synthesizer state
struct Synth {
    sample_rate: f32,
    volume: f32,
    active_notes: Vec<Note>,
    waveform: Waveform,
    attack: f32,
    decay: f32,
    sustain: f32,
    release: f32,
    custom_wavetables: Vec<CustomWavetable>,
    analyzer: Analyzer,
}

impl Synth {
    fn new(sample_rate: f32) -> Self {
        Synth {
            sample_rate,
            volume: 0.5,
            active_notes: Vec::new(),
            waveform: Waveform::Sine,
            attack: 0.1,
            decay: 0.2,
            sustain: 0.7,
            release: 0.3,
            custom_wavetables: Vec::new(),
            analyzer: Analyzer {
                current_waveform_samples: VecDeque::with_capacity(SAMPLE_BUFFER_SIZE),
                fft_buffer: vec![0.0; SAMPLE_BUFFER_SIZE],
                last_update: Instant::now(),
            },
        }
    }

    fn note_on(&mut self, midi_note: u8, velocity: u8) {
        let freq = midi_note_to_freq(midi_note);
        let vel = velocity as f32 / 127.0;
        
        // Remove any existing instances of this note
        self.active_notes.retain(|n| n.midi_note != midi_note);
        
        // Calculate phase increment based on frequency and sample rate
        let phase_increment = freq / self.sample_rate;
        
        self.active_notes.push(Note {
            midi_note,
            frequency: freq,
            phase: 0.0,
            phase_increment,
            velocity: vel,
            state: NoteState::Attack,
            time_in_state: 0.0,
        });
    }

    fn note_off(&mut self, midi_note: u8) {
        for note in self.active_notes.iter_mut() {
            if note.midi_note == midi_note {
                note.state = NoteState::Release;
                note.time_in_state = 0.0;
            }
        }
    }

    fn load_sample(&mut self, path: PathBuf) -> Result<()> {
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

    fn get_sample(&mut self, _sample_time: f32) -> f32 {
        let mut sample = 0.0;
        let mut notes_to_remove = Vec::new();

        for (i, note) in self.active_notes.iter_mut().enumerate() {
            // Update phase for this note
            note.phase = (note.phase + note.phase_increment) % 1.0;
            
            // Calculate envelope
            let envelope = match note.state {
                NoteState::Attack => {
                    note.time_in_state += 1.0 / self.sample_rate;
                    let value = note.time_in_state / self.attack;
                    if value >= 1.0 {
                        note.state = NoteState::Decay;
                        note.time_in_state = 0.0;
                        1.0
                    } else {
                        value
                    }
                },
                NoteState::Decay => {
                    note.time_in_state += 1.0 / self.sample_rate;
                    let value = 1.0 - (1.0 - self.sustain) * (note.time_in_state / self.decay);
                    if value <= self.sustain || note.time_in_state >= self.decay {
                        note.state = NoteState::Sustain;
                        note.time_in_state = 0.0;
                        self.sustain
                    } else {
                        value
                    }
                },
                NoteState::Sustain => self.sustain,
                NoteState::Release => {
                    note.time_in_state += 1.0 / self.sample_rate;
                    let value = self.sustain * (1.0 - note.time_in_state / self.release);
                    if value <= 0.0 || note.time_in_state >= self.release {
                        notes_to_remove.push(i);
                        0.0
                    } else {
                        value
                    }
                },
            };
            
            // Get waveform value based on the current waveform type
            let waveform_value = match &self.waveform {
                Waveform::Sine => (2.0 * std::f32::consts::PI * note.phase).sin(),
                Waveform::Square => if note.phase < 0.5 { 1.0 } else { -1.0 },
                Waveform::Saw => 2.0 * note.phase - 1.0,
                Waveform::Triangle => {
                    if note.phase < 0.25 {
                        4.0 * note.phase
                    } else if note.phase < 0.75 {
                        2.0 - 4.0 * note.phase
                    } else {
                        -4.0 + 4.0 * note.phase
                    }
                },
                Waveform::WhiteNoise => rand::random::<f32>() * 2.0 - 1.0,
                Waveform::CustomSample(index) => {
                    if let Some(wavetable) = self.custom_wavetables.get(*index) {
                        // Sample from the wavetable
                        let position = note.phase * wavetable.samples.len() as f32;
                        let index = position.floor() as usize % wavetable.samples.len();
                        let next_index = (index + 1) % wavetable.samples.len();
                        let fraction = position - position.floor();
                        
                        // Linear interpolation between samples
                        wavetable.samples[index] * (1.0 - fraction) + 
                        wavetable.samples[next_index] * fraction
                    } else {
                        0.0
                    }
                }
            };
            
            sample += waveform_value * envelope * note.velocity;
        }
        
        // Remove finished notes
        for i in notes_to_remove.iter().rev() {
            self.active_notes.remove(*i);
        }
        
        // Apply master volume
        let final_sample = sample * self.volume;
        
        // Update our analyzer with this sample
        if self.analyzer.current_waveform_samples.len() >= SAMPLE_BUFFER_SIZE {
            self.analyzer.current_waveform_samples.pop_front();
        }
        self.analyzer.current_waveform_samples.push_back(final_sample);
        
        final_sample
    }

    // Generate waveform visualization data
    fn generate_waveform_display(&self) -> Vec<[f32; 2]> {
        let mut points = Vec::with_capacity(WAVEFORM_DISPLAY_POINTS);
        let samples = &self.analyzer.current_waveform_samples;
        
        if samples.is_empty() {
            // Generate a flat line if no samples
            for i in 0..WAVEFORM_DISPLAY_POINTS {
                points.push([i as f32 / WAVEFORM_DISPLAY_POINTS as f32, 0.0]);
            }
            return points;
        }
        
        // Sample from our buffer to create the display points
        let step = samples.len() as f32 / WAVEFORM_DISPLAY_POINTS as f32;
        for i in 0..WAVEFORM_DISPLAY_POINTS {
            let pos = (i as f32 * step) as usize;
            if pos < samples.len() {
                points.push([i as f32 / WAVEFORM_DISPLAY_POINTS as f32, samples[pos]]);
            }
        }
        
        points
    }

    // Generate a visualization of the current waveform table
    fn generate_wavetable_display(&self) -> Vec<[f32; 2]> {
        let mut points = Vec::with_capacity(WAVEFORM_DISPLAY_POINTS);
        
        for i in 0..WAVEFORM_DISPLAY_POINTS {
            let phase = i as f32 / WAVEFORM_DISPLAY_POINTS as f32;
            
            let value = match &self.waveform {
                Waveform::Sine => (2.0 * std::f32::consts::PI * phase).sin(),
                Waveform::Square => if phase < 0.5 { 1.0 } else { -1.0 },
                Waveform::Saw => 2.0 * phase - 1.0,
                Waveform::Triangle => {
                    if phase < 0.25 {
                        4.0 * phase
                    } else if phase < 0.75 {
                        2.0 - 4.0 * phase
                    } else {
                        -4.0 + 4.0 * phase
                    }
                },
                Waveform::WhiteNoise => {
                    // For noise, we'll use a pre-calculated random set for visualization
                    let seed = i as f32 / 10.0;
                    (seed.sin() * 12.5).sin()
                },
                Waveform::CustomSample(index) => {
                    if let Some(wavetable) = self.custom_wavetables.get(*index) {
                        let sample_pos = phase * wavetable.samples.len() as f32;
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

    // Generate a 2D representation of the wavetable for different pitches
    fn generate_pitch_wavetable(&self) -> Vec<Vec<[f64; 2]>> {
        const PITCH_CLASSES: usize = 12;
        const SAMPLES_PER_CYCLE: usize = 64;
        
        let mut pitch_lines = Vec::with_capacity(PITCH_CLASSES);
        
        for pitch_class in 0..PITCH_CLASSES {
            let mut points = Vec::with_capacity(SAMPLES_PER_CYCLE);
            
            // Calculate the MIDI note number (middle C = 60)
            let midi_note = 60 + pitch_class;
            
            // Generate one cycle of the waveform for this pitch
            for i in 0..SAMPLES_PER_CYCLE {
                let phase = i as f32 / SAMPLES_PER_CYCLE as f32;
                
                // Get the waveform value based on the current waveform type
                let value = match &self.waveform {
                    Waveform::Sine => (2.0 * PI * phase).sin(),
                    Waveform::Square => if phase < 0.5 { 1.0 } else { -1.0 },
                    Waveform::Saw => 2.0 * phase - 1.0,
                    Waveform::Triangle => {
                        if phase < 0.25 {
                            4.0 * phase
                        } else if phase < 0.75 {
                            2.0 - 4.0 * phase
                        } else {
                            -4.0 + 4.0 * phase
                        }
                    },
                    Waveform::WhiteNoise => {
                        // Use a deterministic "random" function for visualization
                        let seed = (pitch_class * 100 + i) as f32;
                        (seed.sin() * 12.5).sin()
                    },
                    Waveform::CustomSample(index) => {
                        if let Some(wavetable) = self.custom_wavetables.get(*index) {
                            let sample_pos = phase * wavetable.samples.len() as f32;
                            let index = sample_pos.floor() as usize % wavetable.samples.len();
                            wavetable.samples[index]
                        } else {
                            0.0
                        }
                    }
                };
                
                // Apply any active note modulation if this pitch is being played
                let modulated_value = if let Some(note) = self.active_notes.iter()
                    .find(|n| n.midi_note as usize == midi_note) {
                    // Apply envelope modulation
                    let envelope = match note.state {
                        NoteState::Attack => note.time_in_state / self.attack,
                        NoteState::Decay => 1.0 - (1.0 - self.sustain) * (note.time_in_state / self.decay),
                        NoteState::Sustain => self.sustain,
                        NoteState::Release => self.sustain * (1.0 - note.time_in_state / self.release),
                    };
                    value * envelope * note.velocity
                } else {
                    value * 0.3 // Lower amplitude for non-playing notes
                };
                
                // Add the point to our line
                points.push([i as f64 / SAMPLES_PER_CYCLE as f64, modulated_value as f64]);
            }
            
            pitch_lines.push(points);
        }
        
        pitch_lines
    }
}

fn midi_note_to_freq(note: u8) -> f32 {
    const A4_MIDI: f32 = 69.0;
    const A4_FREQ: f32 = 440.0;
    
    A4_FREQ * 2.0f32.powf((note as f32 - A4_MIDI) / 12.0)
}

// Message types for our threaded architecture
enum SynthMessage {
    NoteOn(u8, u8),
    NoteOff(u8),
    ChangeWaveform(Waveform),
    ChangeVolume(f32),
    ChangeAttack(f32),
    ChangeDecay(f32),
    ChangeSustain(f32),
    ChangeRelease(f32),
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
}

impl eframe::App for SynthApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Process any pending messages
        self.process_messages();
        
        // Create the main UI
        egui::CentralPanel::default().show(ctx, |ui| {
            // Add a header with icon
            ui.horizontal(|ui| {
                // Add the title with larger text
                ui.heading("AudioTheorem 2");
                
                // Alternative approach without loading an external image
                ui.label("🎹"); // Use a musical keyboard emoji instead of an image
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
        
        Ok(SynthApp {
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
        })
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
                SynthMessage::ChangeWaveform(waveform) => {
                    if let Ok(mut synth) = self.synth.write() {
                        synth.waveform = waveform;
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
            
            count += 1;
            if count >= MAX_MESSAGES_PER_FRAME {
                break;
            }
        }
    }

    fn render_synth_ui(&mut self, ui: &mut egui::Ui) {
        // Main synthesizer controls
        ui.heading("Synthesizer");
        
        // Waveform selection
        ui.horizontal(|ui| {
            ui.label("Waveform:");
            let mut synth = self.synth.write().unwrap();
            
            if ui.radio_value(&mut synth.waveform, Waveform::Sine, "Sine").clicked() {
                self.sender.send(SynthMessage::ChangeWaveform(Waveform::Sine)).ok();
            }
            if ui.radio_value(&mut synth.waveform, Waveform::Square, "Square").clicked() {
                self.sender.send(SynthMessage::ChangeWaveform(Waveform::Square)).ok();
            }
            if ui.radio_value(&mut synth.waveform, Waveform::Saw, "Saw").clicked() {
                self.sender.send(SynthMessage::ChangeWaveform(Waveform::Saw)).ok();
            }
            if ui.radio_value(&mut synth.waveform, Waveform::Triangle, "Triangle").clicked() {
                self.sender.send(SynthMessage::ChangeWaveform(Waveform::Triangle)).ok();
            }
            if ui.radio_value(&mut synth.waveform, Waveform::WhiteNoise, "Noise").clicked() {
                self.sender.send(SynthMessage::ChangeWaveform(Waveform::WhiteNoise)).ok();
            }
        });
        
        // ADSR controls
        ui.heading("Envelope");
        ui.horizontal(|ui| {
            let mut synth = self.synth.write().unwrap();
            
            ui.label("Attack:");
            if ui.add(egui::Slider::new(&mut synth.attack, 0.01..=2.0).text("s")).changed() {
                self.sender.send(SynthMessage::ChangeAttack(synth.attack)).ok();
            }
            
            ui.label("Decay:");
            if ui.add(egui::Slider::new(&mut synth.decay, 0.01..=2.0).text("s")).changed() {
                self.sender.send(SynthMessage::ChangeDecay(synth.decay)).ok();
            }
            
            ui.label("Sustain:");
            if ui.add(egui::Slider::new(&mut synth.sustain, 0.0..=1.0).text("")).changed() {
                self.sender.send(SynthMessage::ChangeSustain(synth.sustain)).ok();
            }
            
            ui.label("Release:");
            if ui.add(egui::Slider::new(&mut synth.release, 0.01..=5.0).text("s")).changed() {
                self.sender.send(SynthMessage::ChangeRelease(synth.release)).ok();
            }
        });
        
        // Current waveform display
        ui.heading("Current Waveform");
        
        // Create points for the waveform display
        let wavetable_points: Vec<[f64; 2]> = {
            let synth = self.synth.read().unwrap();
            (0..WAVEFORM_DISPLAY_POINTS)
                .map(|i| {
                    let x = i as f64 / WAVEFORM_DISPLAY_POINTS as f64;
                    let phase = x * 2.0 * std::f64::consts::PI;
                    
                    // Get the appropriate sample based on the current waveform
                    let y = match synth.waveform {
                        Waveform::Sine => (phase.sin()) as f64,
                        Waveform::Square => if phase.sin() >= 0.0 { 0.8 } else { -0.8 },
                        Waveform::Saw => (phase / std::f64::consts::PI - 1.0) as f64,
                        Waveform::Triangle => (2.0 * (phase / std::f64::consts::PI).abs() - 1.0) as f64,
                        Waveform::WhiteNoise => 0.0, // We can't show random noise in a static plot
                        Waveform::CustomSample(idx) => {
                            if let Some(wavetable) = synth.custom_wavetables.get(idx) {
                                let sample_idx = (x * wavetable.samples.len() as f64) as usize % wavetable.samples.len();
                                wavetable.samples[sample_idx] as f64
                            } else {
                                0.0
                            }
                        }
                    };
                    
                    [x, y]
                })
                .collect()
        };
        
        // Display the 2D waveform plot
        Plot::new("wavetable_plot")
            .height(150.0)
            .view_aspect(3.0)
            .allow_drag(false)
            .allow_zoom(false)
            .allow_scroll(false)
            .show_axes([false, true])
            .show_grid([false, true])
            .show(ui, |plot_ui| {
                plot_ui.line(Line::new(
                    PlotPoints::from(wavetable_points)
                ).color(egui::Color32::from_rgb(100, 200, 100)));
            });
        
        // Volume control
        ui.heading("Volume");
        ui.horizontal(|ui| {
            let mut synth = self.synth.write().unwrap();
            if ui.add(egui::Slider::new(&mut synth.volume, 0.0..=1.0).text("Volume")).changed() {
                self.sender.send(SynthMessage::SetVolume(synth.volume)).ok();
            }
        });
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
