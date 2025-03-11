use egui::{Ui, Response, Sense, Vec2, Pos2, Color32, Stroke, Align2};

pub struct Knob {
    value: f32,
    range: (f32, f32),
    text: String,
    diameter: f32,
    show_value: bool,
}

impl Knob {
    pub fn new(value: f32, range: (f32, f32), text: impl Into<String>) -> Self {
        Self {
            value,
            range,
            text: text.into(),
            diameter: 40.0,
            show_value: true,
        }
    }
    
    pub fn diameter(mut self, diameter: f32) -> Self {
        self.diameter = diameter;
        self
    }
    
    pub fn show_value(mut self, show: bool) -> Self {
        self.show_value = show;
        self
    }
    
    pub fn show(self, ui: &mut Ui) -> Response {
        let (min, max) = self.range;
        
        // Normalized value between 0 and 1
        let normalized = (self.value - min) / (max - min);
        
        // Calculate the angle (from -140 to 140 degrees)
        let angle = -140.0 + normalized * 280.0;
        let angle_rad = angle.to_radians();
        
        // Allocate space
        let desired_size = Vec2::splat(self.diameter);
        let (rect, response) = ui.allocate_exact_size(desired_size, Sense::click_and_drag());
        
        if ui.is_rect_visible(rect) {
            let center = rect.center();
            let radius = self.diameter / 2.0 - 2.0;
            
            // Draw the background circle
            ui.painter().circle(
                center,
                radius,
                Color32::from_gray(60),
                Stroke::new(1.0, Color32::from_gray(100)),
            );
            
            // Draw the indicator line
            let indicator_end = Pos2::new(
                center.x + angle_rad.cos() * radius * 0.8,
                center.y + angle_rad.sin() * radius * 0.8,
            );
            ui.painter().line_segment(
                [center, indicator_end],
                Stroke::new(2.0, Color32::from_rgb(0, 200, 200)),
            );
            
            // Draw the center dot
            ui.painter().circle(
                center,
                2.0,
                Color32::from_rgb(0, 200, 200),
                Stroke::NONE,
            );
            
            // Draw the text label
            ui.painter().text(
                rect.center_bottom() + Vec2::new(0.0, 14.0),
                Align2::CENTER_CENTER,
                &self.text,
                egui::TextStyle::Body.resolve(ui.style()),
                ui.visuals().text_color(),
            );
            
            // Draw the value
            if self.show_value {
                ui.painter().text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    format!("{:.2}", self.value),
                    egui::TextStyle::Small.resolve(ui.style()),
                    ui.visuals().text_color(),
                );
            }
        }
        
        // Handle interactions
        if response.dragged() {
            if let Some(pointer_pos) = ui.ctx().pointer_latest_pos() {
                let delta = pointer_pos.y - response.rect.center().y;
                let delta_normalized = delta / 100.0; // Sensitivity factor
                let new_normalized = (normalized - delta_normalized).clamp(0.0, 1.0);
                
                if delta != 0.0 {
                    ui.ctx().copy_text(format!("{:.2}", min + new_normalized * (max - min)));
                }
            }
        }
        
        response
    }
}
