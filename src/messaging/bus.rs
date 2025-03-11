use crossbeam_channel::{unbounded, Receiver, Sender};
use std::sync::{Arc, RwLock};
use crate::core::synth::Synth;
use super::SynthMessage;
use crate::core::oscillator::ModulationTarget;

/// MessageBus manages communication between UI and audio engine
pub struct MessageBus {
    pub(crate) sender: Sender<SynthMessage>,
    pub(crate) receiver: Receiver<SynthMessage>,
    synth_ref: Arc<RwLock<Synth>>,
}

impl MessageBus {
    /// Create a new message bus connected to the synth engine
    pub fn new(synth: Arc<RwLock<Synth>>) -> Self {
        let (sender, receiver) = unbounded();
        
        MessageBus {
            sender,
            receiver,
            synth_ref: synth,
        }
    }
    
    /// Get a sender that can be cloned and passed to UI components
    pub fn sender(&self) -> Sender<SynthMessage> {
        self.sender.clone()
    }
    
    /// Process all pending messages
    pub fn process_messages(&self, max_messages: usize) {
        let mut count = 0;
        
        while let Ok(msg) = self.receiver.try_recv() {
            if count >= max_messages {
                break; // Limit messages per frame
            }
            count += 1;
            
            self.handle_message(msg);
        }
    }
    
    /// Handle an individual message
    fn handle_message(&self, msg: SynthMessage) {
        match msg {
            SynthMessage::NoteOn(note, velocity) => {
                if let Ok(mut synth) = self.synth_ref.write() {
                    synth.note_on(note, velocity);
                }
            },
            SynthMessage::NoteOff(note) => {
                if let Ok(mut synth) = self.synth_ref.write() {
                    synth.note_off(note);
                }
            },
            SynthMessage::ChangeOscillator(index, waveform, volume, detune, octave) => {
                if let Ok(mut synth) = self.synth_ref.write() {
                    if let Some(osc) = synth.oscillators.get_mut(index) {
                        osc.waveform = waveform;
                        osc.volume = volume;
                        osc.detune = detune;
                        osc.octave = octave;
                    }
                }
            },
            SynthMessage::ChangeOscillatorEnvelope(index, attack, decay, sustain, release) => {
                if let Ok(mut synth) = self.synth_ref.write() {
                    if let Some(osc) = synth.oscillators.get_mut(index) {
                        osc.attack = attack;
                        osc.decay = decay;
                        osc.sustain = sustain;
                        osc.release = release;
                    }
                }
            },
            SynthMessage::ChangeOscillatorFilter(index, filter_type, cutoff, resonance) => {
                if let Ok(mut synth) = self.synth_ref.write() {
                    if let Some(osc) = synth.oscillators.get_mut(index) {
                        osc.filter_type = filter_type;
                        osc.filter_cutoff = cutoff;
                        osc.filter_resonance = resonance;
                    }
                }
            },
            SynthMessage::ChangeOscillatorModulation(index, amount, target) => {
                if let Ok(mut synth) = self.synth_ref.write() {
                    if let Some(osc) = synth.oscillators.get_mut(index) {
                        osc.mod_amount = amount;
                        osc.mod_target = target;
                    }
                }
            },
            SynthMessage::ChangeOscillatorCombinationMode(mode) => {
                if let Ok(mut synth) = self.synth_ref.write() {
                    synth.oscillator_combination_mode = mode;
                }
            },
            SynthMessage::ChangeMasterEnvelope(attack, decay, sustain, release) => {
                if let Ok(mut synth) = self.synth_ref.write() {
                    synth.attack = attack;
                    synth.decay = decay;
                    synth.sustain = sustain;
                    synth.release = release;
                }
            },
            SynthMessage::ChangeMasterFilter(filter_type, cutoff, resonance) => {
                if let Ok(mut synth) = self.synth_ref.write() {
                    synth.master_filter_type = filter_type;
                    synth.master_filter_cutoff = cutoff;
                    synth.master_filter_resonance = resonance;
                }
            },
            SynthMessage::SetVolume(volume) => {
                if let Ok(mut synth) = self.synth_ref.write() {
                    synth.volume = volume;
                }
            },
            SynthMessage::SetModulation(amount) => {
                // Broadcast modulation to all oscillators
                if let Ok(mut synth) = self.synth_ref.write() {
                    for osc in &mut synth.oscillators {
                        if osc.mod_target != ModulationTarget::None {
                            osc.mod_amount = amount;
                        }
                    }
                }
            },
            SynthMessage::SetSustainPedal(on) => {
                if let Ok(mut synth) = self.synth_ref.write() {
                    synth.set_sustain_pedal(on);
                }
            },
            SynthMessage::SetPitchBend(value) => {
                if let Ok(mut synth) = self.synth_ref.write() {
                    for osc in &mut synth.oscillators {
                        osc.pitch_bend = value;
                    }
                }
            },
            // Add handlers for other message types
            _ => {
                // Log unhandled message types if desired
            }
        }
    }
    
    /// Public method to try to receive a message
    pub fn try_receive(&self) -> Result<SynthMessage, crossbeam_channel::TryRecvError> {
        self.receiver.try_recv()
    }
    
    /// Public method to send a message
    pub fn send(&self, msg: SynthMessage) -> Result<(), crossbeam_channel::SendError<SynthMessage>> {
        self.sender.send(msg)
    }
}
