# **DataViz** 🚀  
A modular, customizable, and feature-rich 2D plotting library written in Rust. With **DataViz**, you can create a wide variety of plots tailored to your needs, supporting both raster (PNG) and vector (SVG) outputs. You can save and display(interactive or real-time) your DataViz figures.

---

## **Features**  
### **Supported Plot Types**  
- **Bar Charts**: Create grouped horizontal and vertical bar charts.  
- **Scatter Graphs**: Visualize data points with various shapes (circle, square, triangle, etc.).  
- **Pie Charts**: Represent data proportions as slices of a circle.  
- **Area Charts**: Highlight trends with filled areas under data lines.  
- **Histograms**: Analyze frequency distributions with dynamic bin calculations.  
- **Cartesian Graphs**: Plot mathematical functions or datasets on a coordinate plane.  
- **Quadrant 1 Graphs**: Focused plotting in the first quadrant for non-negative data.

### **Customization Options**  
- **Title**: Add meaningful titles to your plots.  
- **Axes Labels**: Define X-axis and Y-axis labels.  
- **Dynamic Scaling**: Automatically fit data within the plot dimensions.  
- **Colors**: Customize colors for the background, axes, and data elements.  
- **Margins**: Add space around the plot for better visibility.  
- **Line and Dot Styles**: Customize line types (solid, dashed, dotted) and dot shapes (circle, square, cross, triangle).  

### **Output Formats**  
- **Raster (PNG)**: Save high-quality images of your plots.  
- **Vector (SVG)**: Generate scalable vector graphics for precision and scalability.  

### **Interactive Capabilities**  
- Hover effects and real-time updates.

---

## **Installation**  
Add the following dependencies to your `Cargo.toml` file:
```toml
[dependencies]
dataviz = "0.1.5"
```

## **Examples**  
To see more examples you can visit: https://github.com/dataviz-rs/dataviz-examples

---

## **Usage Example**  

```rust
use dataviz::figure::{
    canvas::pixelcanvas::PixelCanvas, // For creating a pixel canvas to draw the chart
    configuration::figureconfig::FigureConfig, // For configuring visual properties of the chart
    datasets::bardataset::BarDataset, // For defining datasets for bar charts
    display::winop::Winop,            // For displaying the chart interactively
    drawers::drawer::Drawer,          // For drawing functionality
    figuretypes::groupbarchart::GroupBarChart, // For creating grouped bar charts
    utilities::orientation::Orientation, // For setting the chart orientation (vertical/horizontal)
};

use rand::Rng;

fn main() {
    // Configuration for the figure (e.g., fonts, colors, grid, and axis settings)
    let figure_config = FigureConfig {
        font_size_title: 20.0,       // Font size for the chart title
        font_size_label: 16.0,       // Font size for axis labels
        font_size_legend: 14.0,      // Font size for legend
        font_size_axis: 10.0,        // Font size for axis tick labels
        color_axis: [0, 0, 0],       // Color of the axes (black)
        color_background: [0, 0, 0], // Background color of the chart (black)
        color_grid: [220, 220, 220], // Grid color (light gray)
        color_title: [0, 0, 0],      // Title color (black)
        num_axis_ticks: 20,          // Number of axis tick marks
        num_grid_horizontal: 20,     // Number of horizontal grid lines
        num_grid_vertical: 20,       // Number of vertical grid lines
        font_label: Some("path/to/dataviz/resources/fonts/font.ttf"
            .to_string()), // Path to the font for axis labels
        font_title: Some("path/to/dataviz/resources/fonts/font.ttf"
            .to_string()), // Path to the font for the title
    };

    // Create a grouped bar chart with the given configuration
    let mut barchart = GroupBarChart::new(
        "Grouped Bar Chart",     // Chart title
        "X Axis",                // X-axis label
        "Y Axis",                // Y-axis label
        Orientation::Horizontal, // Orientation of the bar chart (Horizontal)
        figure_config,           // Pass the figure configuration
    );

    // Create a pixel canvas for rendering the chart
    let mut canvas = PixelCanvas::new(800, 600, [255, 255, 255], 80); // 800x600 canvas with white background

    // Define datasets for the bar chart
    let mut dataset1 = BarDataset::new("Company A", [220, 0, 0]); // Dataset for Company A (red bars)
    dataset1.add_data(2020.0, 100.0); // Add data point: Year 2020, value 100
    dataset1.add_data(2021.0, 200.0); // Add data point: Year 2021, value 200
    dataset1.add_data(2022.0, 150.0); // Add data point: Year 2022, value 150

    let mut dataset2 = BarDataset::new("Company B", [0, 220, 0]); // Dataset for Company B (green bars)
    dataset2.add_data(2020.0, 120.0); // Add data point: Year 2020, value 120
    dataset2.add_data(2021.0, 180.0); // Add data point: Year 2021, value 180
    dataset2.add_data(2022.0, 220.0); // Add data point: Year 2022, value 220

    let mut dataset3 = BarDataset::new("Company C", [0, 0, 220]); // Dataset for Company C (blue bars)
    dataset3.add_data(2020.0, 150.0); // Add data point: Year 2020, value 150
    dataset3.add_data(2021.0, 250.0); // Add data point: Year 2021, value 250
    dataset3.add_data(2022.0, 400.0); // Add data point: Year 2022, value 400

    let mut dataset4 = BarDataset::new("Company D", [150, 100, 50]); // Dataset for Company D (brown bars)
    dataset4.add_data(2020.0, 50.0); // Add data point: Year 2020, value 50
    dataset4.add_data(2021.0, 256.0); // Add data point: Year 2021, value 256
    dataset4.add_data(2022.0, 40.0); // Add data point: Year 2022, value 40

    // Add datasets to the bar chart
    barchart.add_dataset(dataset1);
    barchart.add_dataset(dataset2);
    barchart.add_dataset(dataset3);
    barchart.add_dataset(dataset4);

    // Draw the bar chart onto the canvas
    barchart.draw(&mut canvas);

    // Save the canvas as an image file
    canvas.save_as_image("grouped_horizontal_bar_chart.png");

    // Display the bar chart interactively
    Winop::display_interactive(&mut canvas, &mut barchart, "Interactive BarChart");


    // Create a new pixel canvas for real-time display
    let mut rng = rand::thread_rng();
    let update_data = move |chart: &mut GroupBarChart| {
        for i in 0..chart.datasets.len() {
            for point in chart.datasets[i].data.iter_mut() {
                point.1 += rng.gen_range(-20.0..30.0); // Increment y-value
            }
        }
    };

    // Display the bar chart in real-time
    Winop::display_real_time(
        &mut canvas,
        &mut barchart,
        "Real-Time Bar Chart",
        update_data,
        30,
    );
}
```

<img src="https://github.com/dataviz-rs/dataviz-examples/blob/main/pixelgroupedbarchartdisplay/screenshots/interactivedisplay.png?raw=true" alt="" width="300px">

---

```rust
let mut figure = FigureFactory::create_figure(FigureType::GroupBarChartVertical);

    // Attempt to downcast the Drawer trait object to GroupBarChart
    // Dereference the Box and use downcast_mut
    if let Some(group_bar_chart) = figure.as_any().downcast_mut::<GroupBarChart>() {
        group_bar_chart.config.set_font_paths(
            "path/to/dataviz/resources/fonts/font.ttf".to_string(),
            "path/to/dataviz/resources/fonts/font.ttf".to_string(),
        );

        // Create a SVG canvas for rendering the chart
        let mut canvas = PixelCanvas::new(800, 600, [255, 255, 255], 80); // 800x600 canvas with white background

        // Define datasets for the bar chart
        let mut dataset1 = BarDataset::new("Company A", [220, 0, 0]); // Dataset for Company A (red bars)
        dataset1.add_data(2020.0, 100.0); // Add data point: Year 2020, value 100
        dataset1.add_data(2021.0, 200.0); // Add data point: Year 2021, value 200

        let mut dataset2 = BarDataset::new("Company B", [0, 220, 0]); // Dataset for Company B (green bars)
        dataset2.add_data(2020.0, 120.0); // Add data point: Year 2020, value 120
        dataset2.add_data(2021.0, 180.0); // Add data point: Year 2021, value 180

        // Add datasets to the bar chart
        group_bar_chart.add_dataset(dataset1);
        group_bar_chart.add_dataset(dataset2);

        // Draw the bar chart onto the canvas
        group_bar_chart.draw(&mut canvas);

        // Save the canvas as an image file
        canvas.save_as_image("grouped_vertical_bar_chart.png");
    } else {
        println!("Failed to downcast to GroupBarChart.");
    }
```

<img src="https://github.com/dataviz-rs/dataviz-examples/blob/main/pixelgroupedbarchartfigurefactory/screenshots/grouped_vertical_bar_chart.png?raw=true" alt="" width="300px">

---

```rust
use dataviz::figure::{
    canvas::svgcanvas::SvgCanvas,              // For creating a SVG canvas to draw the chart
    configuration::figureconfig::FigureConfig, // For configuring visual properties of the chart
    datasets::bardataset::BarDataset,          // For defining datasets for bar charts
    drawers::drawer::Drawer,                   // For drawing functionality
    figuretypes::groupbarchart::GroupBarChart, // For creating grouped bar charts
    utilities::orientation::Orientation,       // For setting the chart orientation (vertical/horizontal)
};

fn main() {
    // Configuration for the figure (e.g., fonts, colors, grid, and axis settings)
    let figure_config = FigureConfig {
        font_size_title: 20.0,   // Font size for the chart title
        font_size_label: 16.0,   // Font size for axis labels
        font_size_legend: 14.0,  // Font size for legend
        font_size_axis: 10.0,    // Font size for axis tick labels
        color_axis: [0, 0, 0],   // Color of the axes (black)
        color_background: [0, 0, 0], // Background color of the chart (black)
        color_grid: [220, 220, 220], // Grid color (light gray)
        color_title: [0, 0, 0],      // Title color (black)
        num_axis_ticks: 20,          // Number of axis tick marks
        num_grid_horizontal: 20,     // Number of horizontal grid lines
        num_grid_vertical: 20,       // Number of vertical grid lines
        font_label: Some("path/to/dataviz/resources/fonts/font.ttf"
        .to_string()), // Path to the font for axis labels
        font_title: Some("path/to/dataviz/resources/fonts/font.ttf"
        .to_string()), // Path to the font for the title
    };

    // Create a grouped bar chart with the given configuration
    let mut barchart = GroupBarChart::new(
        "Grouped Bar Chart",    // Chart title
        "X Axis",               // X-axis label
        "Y Axis",               // Y-axis label
        Orientation::Vertical,  // Orientation of the bar chart (vertical)
        figure_config,          // Pass the figure configuration
    );

    // Create a SVG canvas for rendering the chart
    let mut canvas = SvgCanvas::new(800, 600, "white", 80); // 800x600 canvas with white background

    // Define datasets for the bar chart
    let mut dataset1 = BarDataset::new("Company A", [220, 0, 0]); // Dataset for Company A (red bars)
    dataset1.add_data(2020.0, 100.0); // Add data point: Year 2020, value 100
    dataset1.add_data(2021.0, 200.0); // Add data point: Year 2021, value 200
    dataset1.add_data(2022.0, 150.0); // Add data point: Year 2022, value 150

    let mut dataset2 = BarDataset::new("Company B", [0, 220, 0]); // Dataset for Company B (green bars)
    dataset2.add_data(2020.0, 120.0); // Add data point: Year 2020, value 120
    dataset2.add_data(2021.0, 180.0); // Add data point: Year 2021, value 180
    dataset2.add_data(2022.0, 220.0); // Add data point: Year 2022, value 220

    let mut dataset3 = BarDataset::new("Company C", [0, 0, 220]); // Dataset for Company C (blue bars)
    dataset3.add_data(2020.0, 150.0); // Add data point: Year 2020, value 150
    dataset3.add_data(2021.0, 250.0); // Add data point: Year 2021, value 250
    dataset3.add_data(2022.0, 400.0); // Add data point: Year 2022, value 400

    let mut dataset4 = BarDataset::new("Company D", [150, 100, 50]); // Dataset for Company D (brown bars)
    dataset4.add_data(2020.0, 50.0);  // Add data point: Year 2020, value 50
    dataset4.add_data(2021.0, 256.0); // Add data point: Year 2021, value 256
    dataset4.add_data(2022.0, 40.0);  // Add data point: Year 2022, value 40

    // Add datasets to the bar chart
    barchart.add_dataset(dataset1);
    barchart.add_dataset(dataset2);
    barchart.add_dataset(dataset3);
    barchart.add_dataset(dataset4);

    // Draw the bar chart onto the canvas
    barchart.draw_svg(&mut canvas);

    // Save the canvas as an image file
    canvas.save("grouped_vertical_bar_chart.svg").unwrap();
}
```

<img src="https://github.com/dataviz-rs/dataviz-examples/blob/main/svggroupedbarchartvertical/screenshots/grouped_vertical_bar_chart.svg?raw=true" alt="" width="300px">

---

## **Examples**  
### **PNG Outputs**  
<img src="https://github.com/dataviz-rs/dataviz-screenshots/blob/main/cartesian_graph.png?raw=true" alt="" width="300px">
<img src="https://github.com/dataviz-rs/dataviz-screenshots/blob/main/grouped_horizontal_bar_chart.png?raw=true" alt="" width="300px">
<img src="https://github.com/dataviz-rs/dataviz-screenshots/blob/main/grouped_vertical_bar_chart.png?raw=true" alt="" width="300px">
<img src="https://github.com/dataviz-rs/dataviz-screenshots/blob/main/pie_chart.png?raw=true" alt="" width="300px">
<img src="https://github.com/dataviz-rs/dataviz-screenshots/blob/main/quadrant1_graph.png?raw=true" alt="" width="300px">
<img src="https://github.com/dataviz-rs/dataviz-screenshots/blob/main/scatter_graph.png?raw=true" alt="" width="300px">
<img src="https://github.com/dataviz-rs/dataviz-screenshots/blob/main/area_chart.png?raw=true" alt="" width="300px">
<img src="https://github.com/dataviz-rs/dataviz-screenshots/blob/main/histogram.png?raw=true" alt="" width="300px">
<img src="https://github.com/dataviz-rs/dataviz-screenshots/blob/main/real_time.gif?raw=true" alt="" width="300px">


### **SVG Outputs**
<img src="https://github.com/dataviz-rs/dataviz-screenshots/blob/main/cartesian_graph.svg?raw=true" alt="" width="300px">
<img src="https://github.com/dataviz-rs/dataviz-screenshots/blob/main/grouped_horizontal_bar_chart.svg?raw=true" alt="" width="300px">
<img src="https://github.com/dataviz-rs/dataviz-screenshots/blob/main/grouped_vertical_bar_chart.svg?raw=true" alt="" width="300px">
<img src="https://github.com/dataviz-rs/dataviz-screenshots/blob/main/pie_chart.svg?raw=true" alt="" width="300px">
<img src="https://github.com/dataviz-rs/dataviz-screenshots/blob/main/quadrant1_graph.svg?raw=true" alt="" width="300px">
<img src="https://github.com/dataviz-rs/dataviz-screenshots/blob/main/scatter_graph.svg?raw=true" alt="" width="300px">
<img src="https://github.com/dataviz-rs/dataviz-screenshots/blob/main/area_chart.svg?raw=true" alt="" width="300px">
<img src="https://github.com/dataviz-rs/dataviz-screenshots/blob/main/histogram.svg?raw=true" alt="" width="300px">


## **License**  
This project is licensed under the **MIT**. See the `LICENSE` file for details.

---

## **Contributing**  
We welcome contributions to make **DataViz** even better!
- Visit DataViz repository: https://github.com/dataviz-rs/dataviz
- Report bugs and suggest features through GitHub Issues.  
- Submit pull requests for enhancements or fixes.

Let’s make data visualization in Rust easy and accessible!

---
