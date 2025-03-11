use egui::{Ui, Color32};
use egui_plot::{Plot, Line, PlotPoints};

pub struct WaveformPlot {
    points: Vec<[f32; 2]>,
    height: f32,
    color: Color32,
    fill: bool,
    x_grid: bool,
    y_grid: bool,
}

impl WaveformPlot {
    pub fn new(points: Vec<[f32; 2]>) -> Self {
        Self {
            points,
            height: 100.0,
            color: Color32::from_rgb(0, 188, 212),
            fill: false,
            x_grid: false,
            y_grid: false,
        }
    }
    
    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }
    
    pub fn color(mut self, color: Color32) -> Self {
        self.color = color;
        self
    }
    
    pub fn fill(mut self, fill: bool) -> Self {
        self.fill = fill;
        self
    }
    
    pub fn grid(mut self, x_grid: bool, y_grid: bool) -> Self {
        self.x_grid = x_grid;
        self.y_grid = y_grid;
        self
    }
    
    pub fn show(self, ui: &mut Ui, id_source: impl std::hash::Hash) {
        let plot = Plot::new(id_source)
            .height(self.height)
            .show_x(self.x_grid)
            .show_y(self.y_grid)
            .allow_zoom(false)
            .allow_drag(false);
        
        plot.show(ui, |plot_ui| {
            let plot_points = PlotPoints::from_iter(
                self.points.iter().map(|[x, y]| [*x as f64, *y as f64])
            );
            
            let line = Line::new(plot_points).color(self.color);
            
            let line = if self.fill {
                line.fill(0.2)
            } else {
                line
            };
            
            plot_ui.line(line);
        });
    }
}
