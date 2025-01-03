use crate::figure::{
    configuration::figureconfig::FigureConfig, datasets::cartesiangraphdataset::CartesianDataset,
};

pub struct CartesianGraph {
    pub datasets: Vec<CartesianDataset>,
    pub title: String,
    pub x_label: String,
    pub y_label: String,
    pub x_min: f64, // Minimum x-value
    pub x_max: f64, // Maximum x-value
    pub y_min: f64, // Minimum y-value
    pub y_max: f64, // Maximum y-value
    pub config: FigureConfig,
}

impl CartesianGraph {
    pub fn new(title: &str, x_label: &str, y_label: &str, config: &FigureConfig) -> Self {
        Self {
            datasets: Vec::new(),
            title: title.to_string(),
            x_label: x_label.to_string(),
            y_label: y_label.to_string(),
            x_min: f64::INFINITY,     // Initialize to max range
            x_max: f64::NEG_INFINITY, // Initialize to min range
            y_min: f64::INFINITY,     // Initialize to max range
            y_max: f64::NEG_INFINITY, // Initialize to min range
            config: config.clone(),
        }
    }

    pub fn add_dataset(&mut self, dataset: CartesianDataset) {
        self.datasets.push(dataset);
        self.update_range();
    }

    pub fn update_range(&mut self) {
        for dataset in &self.datasets {
            for &(x, y) in &dataset.points {
                if x < self.x_min {
                    self.x_min = x;
                }
                if x > self.x_max {
                    self.x_max = x;
                }
                if y < self.y_min {
                    self.y_min = y;
                }
                if y > self.y_max {
                    self.y_max = y;
                }
            }
        }
        let mut is_empty = self.datasets.is_empty();

        for dataset in &self.datasets {
            if dataset.points.is_empty() {
                is_empty = true;
                break;
            }
        }

        if !is_empty {
            let abs_x_min = self.x_min.abs();
            let abs_x_max = self.x_max.abs();

            if abs_x_min > abs_x_max {
                self.x_max = abs_x_min;
            } else {
                self.x_min = -abs_x_max;
            }

            let abs_y_min = self.y_min.abs();
            let abs_y_max = self.y_max.abs();

            if abs_y_min > abs_y_max {
                self.y_max = abs_y_min;
            } else {
                self.y_min = -abs_y_max;
            }
        }
    }
}
