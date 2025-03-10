# AudioTheorem 2 Synthesizer

A modern, feature-rich software synthesizer built with Rust, with the aim of integrating the original AudioTheorem to have a complete and comprehensive music theory tool.

## Todo
- [ ] Add Equalizer to each oscillator and master output
- [ ] Add effects to each oscillator and effects chain to master output
- [ ] Add ability to save and load custom wavetables
- [ ] Add ability to load previous system configuration
- [ ] Show 3d wavetable to show octaves and pitchclasses

## Features

- Multiple waveform types (Sine, Square, Saw, Triangle, Noise)
- Per-oscillator and master ADSR envelope control
- Custom sample loading for wavetable synthesis
- Real-time waveform visualization
- MIDI device support
- Configurable audio output
- Multiple oscillator combination modes (Parallel, FM, AM, Ring Mod, Filter)
- Per-oscillator filters with modulation options
- Preset system for saving and loading synth configurations

## Technical Details

- Built with Rust for high performance and memory safety
- Uses CPAL for cross-platform audio
- EGUI for responsive GUI
- MIDIR for MIDI device integration
- Multi-threaded architecture with message passing

## Getting Started

1. Clone the repository
2. Run with `cargo run --release`
3. Connect a MIDI keyboard or use computer keyboard controls
4. Experiment with different waveforms and envelope settings

## Requirements

- Rust 1.70+
- Audio output device
- Optional: MIDI controller

## License

Ancillary