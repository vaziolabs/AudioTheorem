use crate::core::oscillator::NoteState;

pub struct Voice {
    pub note: u8,
    pub velocity: u8,
    pub state: NoteState,
    pub envelope_val: f32,
    pub current_phase: f32,
    pub release_time: f32,
}

impl Voice {
    pub fn new(note: u8, velocity: u8) -> Self {
        Self {
            note,
            velocity,
            state: NoteState::Pressed,
            envelope_val: 0.0,
            current_phase: 0.0,
            release_time: 0.0,
        }
    }
    
    pub fn is_active(&self) -> bool {
        match self.state {
            NoteState::Off => false,
            _ => true,
        }
    }
    
    pub fn release(&mut self) {
        if self.state == NoteState::Pressed {
            self.state = NoteState::Released;
            self.release_time = 0.0;
        }
    }
}
