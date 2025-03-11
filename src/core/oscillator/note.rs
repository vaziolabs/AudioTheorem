// Note state for envelope
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoteState {
    Attack,
    Decay,
    Sustain,
    Release,
    Off,
    Pressed,
    Released
}

// Structure representing a single note
#[derive(Debug, Clone)]
pub struct Note {
    pub midi_note: u8,
    pub frequency: f32,
    pub phase: f32,
    pub phase_increment: f32,
    pub velocity: f32,
    pub state: NoteState,
    pub time_in_state: f32,
}

impl Default for Note {
    fn default() -> Self {
        Self {
            midi_note: 0,
            velocity: 0.0,
            frequency: 440.0,
            phase: 0.0,
            phase_increment: 0.0,
            state: NoteState::Off,
            time_in_state: 0.0
        }
    }
}