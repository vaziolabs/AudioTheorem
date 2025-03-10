# AudioTheorem 2 Synthesizer

A modern, feature-rich software synthesizer built with Rust.

## Features

- Multiple waveform types (Sine, Square, Saw, Triangle, Noise)
- ADSR envelope control
- Custom sample loading for wavetable synthesis
- Real-time waveform visualization
- MIDI device support
- Configurable audio output

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