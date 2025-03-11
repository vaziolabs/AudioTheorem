use egui::{Ui, Rect, Stroke, Vec2, Pos2, Color32};
use crate::core::oscillator::Envelope;

pub struct EnvelopeEditor {
    envelope: Envelope,
    width: f32,
    height: f32,
}

impl EnvelopeEditor {
    pub fn new(envelope: &Envelope) -> Self {
        Self {
            envelope: envelope.clone(),
            width: 200.0,
            height: 80.0,
        }
    }
    
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.width = width;
        self.height = height;
        self
    }
    
    pub fn show(&self, ui: &mut Ui) -> Option<Envelope> {
        let mut result = None;
        let mut env = self.envelope.clone();
        
        // Create sliders for each ADSR parameter
        ui.vertical(|ui| {
            let attack_changed = ui.add(egui::Slider::new(&mut env.attack, 0.01..=2.0).text("Attack")).changed();
            let decay_changed = ui.add(egui::Slider::new(&mut env.decay, 0.01..=2.0).text("Decay")).changed();
            let sustain_changed = ui.add(egui::Slider::new(&mut env.sustain, 0.0..=1.0).text("Sustain")).changed();
            let release_changed = ui.add(egui::Slider::new(&mut env.release, 0.01..=3.0).text("Release")).changed();
            
            if attack_changed || decay_changed || sustain_changed || release_changed {
                result = Some(env.clone());
            }
        });
        
        // Visualize the envelope shape
        let desired_size = Vec2::new(self.width, self.height);
        let (rect, _response) = ui.allocate_exact_size(desired_size, egui::Sense::hover());
        
        if ui.is_rect_visible(rect) {
            // Draw envelope visualization
            let stroke = Stroke::new(1.5, Color32::from_rgb(0, 200, 200));
            let points = self.generate_envelope_points(&env, rect);
            ui.painter().add(egui::Shape::line(points, stroke));
        }
        
        result
    }
    
    fn generate_envelope_points(&self, env: &Envelope, rect: Rect) -> Vec<Pos2> {
        let attack_width = (env.attack / (env.attack + env.decay + env.release)).min(0.33);
        let decay_width = (env.decay / (env.attack + env.decay + env.release)).min(0.33);
        let sustain_width = 0.2; // Fixed width for sustain
        let release_width = (env.release / (env.attack + env.decay + env.release)).min(0.33);
        
        // Normalize to 0-1 range
        let total_width = attack_width + decay_width + sustain_width + release_width;
        let attack_norm = attack_width / total_width;
        let decay_norm = decay_width / total_width;
        let sustain_norm = sustain_width / total_width;
        let release_norm = release_width / total_width;
        
        let left = rect.left();
        let bottom = rect.bottom() - 2.0;
        let width = rect.width();
        let height = rect.height() - 4.0;
        
        let x1 = left;
        let x2 = left + width * attack_norm;
        let x3 = x2 + width * decay_norm;
        let x4 = x3 + width * sustain_norm;
        let x5 = x4 + width * release_norm;
        
        let y1 = bottom; // Start at 0
        let y2 = bottom - height; // Attack peak at 1.0
        let y3 = bottom - height * env.sustain; // Decay to sustain level
        let y4 = y3; // Sustain level
        let y5 = bottom; // Release back to 0
        
        vec![
            Pos2::new(x1, y1),
            Pos2::new(x2, y2),
            Pos2::new(x3, y3),
            Pos2::new(x4, y4),
            Pos2::new(x5, y5),
        ]
    }
}
