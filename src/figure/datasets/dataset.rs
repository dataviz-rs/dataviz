use super::{
    areachartdataset::AreaChartDataset, bardataset::BarDataset,
    cartesiangraphdataset::CartesianDataset, scattergraphdataset::ScatterGraphDataset,
};

/// A trait for managing datasets used in different types of charts or graphs.
pub trait Dataset {
    /// Retrieves all points in the dataset as a vector of `(x, y)` tuples.
    ///
    /// # Returns
    /// A vector of `(f64, f64)` representing the data points in the dataset.
    fn get_points(&self) -> Vec<(f64, f64)>;

    /// Adds a single point to the dataset.
    ///
    /// # Parameters
    /// - `point`: A tuple `(f64, f64)` representing the x and y coordinates of the point to add.
    fn add_point(&mut self, point: (f64, f64));
}

impl Dataset for BarDataset {
    /// Implementation of the `Dataset` trait for `BarDataset`.
    ///
    /// - `get_points`: Returns the bar data as `(x, y)` pairs.
    /// - `add_point`: Adds a new `(x, y)` pair to the bar dataset.
    fn get_points(&self) -> Vec<(f64, f64)> {
        self.data.clone()
    }

    fn add_point(&mut self, point: (f64, f64)) {
        self.data.push(point);
    }
}

impl Dataset for CartesianDataset {
    /// Implementation of the `Dataset` trait for `CartesianDataset`.
    ///
    /// - `get_points`: Returns the Cartesian data as `(x, y)` pairs.
    /// - `add_point`: Adds a new `(x, y)` pair to the Cartesian dataset.
    fn get_points(&self) -> Vec<(f64, f64)> {
        self.points.clone()
    }

    fn add_point(&mut self, point: (f64, f64)) {
        self.points.push(point);
    }
}

impl Dataset for ScatterGraphDataset {
    /// Implementation of the `Dataset` trait for `ScatterGraphDataset`.
    ///
    /// - `get_points`: Returns the scatter graph data as `(x, y)` pairs.
    /// - `add_point`: Adds a new `(x, y)` pair to the scatter graph dataset.
    fn get_points(&self) -> Vec<(f64, f64)> {
        self.points.clone()
    }

    fn add_point(&mut self, point: (f64, f64)) {
        self.points.push(point);
    }
}

impl Dataset for AreaChartDataset {
    /// Implementation of the `Dataset` trait for `AreaChartDataset`.
    ///
    /// - `get_points`: Returns the area chart data as `(x, y)` pairs.
    /// - `add_point`: Adds a new `(x, y)` pair to the area chart dataset.
    fn get_points(&self) -> Vec<(f64, f64)> {
        self.points.clone()
    }

    fn add_point(&mut self, point: (f64, f64)) {
        self.points.push(point);
    }
}
