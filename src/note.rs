// Note state for envelope
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NoteState {
    Attack,
    Decay,
    Sustain,
    Release,
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