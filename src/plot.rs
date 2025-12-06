use gtk::prelude::*;
use gtk::{Align, Application, ApplicationWindow, DrawingArea};
use gtk4::{self as gtk, AspectFrame, Frame};
use std::fs;

#[derive(Clone)]
pub struct PlotData {
    cpu_temps: Vec<f64>,
    gpu_temps: Vec<f64>,
    nvme_temps: Vec<f64>,
}

pub fn parse_session_data(csv_content: &str) -> PlotData {
    let mut cpu_temps = Vec::new();
    let mut gpu_temps = Vec::new();
    let mut nvme_temps = Vec::new();

    // We will store the "chosen" label for each device type here.
    // Once we pick a label (like "Composite"), we ignore all others (like "Sensor 1").
    let mut cpu_label: Option<String> = None;
    let mut gpu_label: Option<String> = None;
    let mut nvme_label: Option<String> = None;

    for line in csv_content.lines() {
        if line.starts_with('#') || line.starts_with("Type,") {
            continue;
        }

        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() >= 3 {
            let type_ = parts[0];
            let label = parts[1];

            // Only proceed if we can parse the temperature
            if let Ok(temp) = parts[2].parse::<f64>() {

                match type_ {
                    "CPU" => {
                        if cpu_label.is_none() {
                            cpu_label = Some(label.to_string());
                        }
                        if cpu_label.as_deref() == Some(label) {
                            cpu_temps.push(temp);
                        }
                    }
                    "GPU" => {
                        if gpu_label.is_none() {
                            gpu_label = Some(label.to_string());
                        }
                        if gpu_label.as_deref() == Some(label) {
                            gpu_temps.push(temp);
                        }
                    }
                    "NVME" => {
                        if nvme_label.is_none() {
                            nvme_label = Some(label.to_string());
                        }
                        if nvme_label.as_deref() == Some(label) {
                            nvme_temps.push(temp);
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    PlotData {
        cpu_temps,
        gpu_temps,
        nvme_temps,
    }
}

pub fn plot_maker() {
    let dir = fs::read_dir("./session").expect("Unable to read session directory");
    let mut paths: Vec<_> = dir
        .filter_map(|res| res.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map_or(false, |ext| ext == "csv"))
        .collect();

    paths.sort();

    let latest = paths.last().expect("No session files found");
    let csv_content = fs::read_to_string(latest).expect("Unable to read latest session");
    let plot_data = parse_session_data(&csv_content);

    let app = Application::builder()
        .application_id("com.github.twatch")
        .build();

    app.connect_activate(move |app| build_ui(app, plot_data.clone()));
    app.run_with_args(&Vec::<String>::new());
}

pub fn build_ui(app: &Application, plot_data: PlotData) {
    let content = Frame::new(Some("Temperature Monitor Graph"));
    let drawing_area = DrawingArea::new();

    drawing_area.set_draw_func(move |_area, context, width, height| {
        let w = width as f64;
        let h = height as f64;

        // Background
        context.set_source_rgb(1.0, 1.0, 1.0);
        context.paint().expect("Failed to paint");

        let margin_left = 60.0;
        let margin_right = 40.0;
        let margin_top = 40.0;
        let margin_bottom = 50.0;

        let plot_width = w - margin_left - margin_right;
        let plot_height = h - margin_top - margin_bottom;

        let x_inner_pad = 30.0;
        let effect_width = plot_width - (x_inner_pad * 2.0);

        // Draw axes
        context.set_source_rgb(0.0, 0.0, 0.0);
        context.set_line_width(2.0);
        context.move_to(margin_left, margin_top);
        context.line_to(margin_left, h - margin_bottom);
        context.line_to(w - margin_right, h - margin_bottom);
        context.stroke().expect("Failed to stroke axes");

        // Grid lines
        context.set_line_width(0.5);
        context.set_source_rgb(0.8, 0.8, 0.8);

        for i in 0..=11 {
            let temp = i * 10;
            let y = h - margin_bottom - (temp as f64 / 110.0) * plot_height;

            context.move_to(margin_left, y);
            context.line_to(w - margin_right, y);
            context.stroke().expect("Failed to stroke grid");

            context.set_source_rgb(0.0, 0.0, 0.0);
            context.move_to(margin_left - 35.0, y + 5.0);
            context
                .show_text(&format!("{}°C", temp))
                .expect("Failed to show text");
            context.set_source_rgb(0.8, 0.8, 0.8);
        }

        let num_samples = plot_data
            .cpu_temps
            .len()
            .max(plot_data.gpu_temps.len())
            .max(plot_data.nvme_temps.len())
            .max(1);
        let sample_step = if num_samples > 10 {
            num_samples / 10
        } else {
            1
        };

        for i in (0..=num_samples).step_by(sample_step) {
            let pct = i as f64 / num_samples as f64;
            let x = margin_left + x_inner_pad + (pct * effect_width);

            context.move_to(x, margin_top);
            context.line_to(x, h - margin_bottom);
            context.stroke().expect("Failed to stroke grid");

            context.set_source_rgb(0.0, 0.0, 0.0);
            context.move_to(x - 10.0, h - margin_bottom + 20.0);
            context
                .show_text(&format!("{}", i))
                .expect("Failed to show text");
            context.set_source_rgb(0.8, 0.8, 0.8);
        }

        // Plot lines

        if !plot_data.cpu_temps.is_empty() {
            context.set_source_rgb(1.0, 0.0, 0.0);
            context.set_line_width(2.0);
            for (i, &temp) in plot_data.cpu_temps.iter().enumerate() {
                let pct = i as f64 / num_samples as f64;
                let x = margin_left + x_inner_pad + (pct * effect_width);

                let y = h - margin_bottom - (temp / 110.0) * plot_height;

                if i == 0 {
                    context.move_to(x, y);
                } else {
                    context.line_to(x, y);
                }
            }
            context.stroke().expect("Failed to stroke CPU line");
        }
        if !plot_data.gpu_temps.is_empty() {
            context.set_source_rgb(0.0, 0.8, 0.0);
            context.set_line_width(2.0);
            for (i, &temp) in plot_data.gpu_temps.iter().enumerate() {
                let pct = i as f64 / num_samples as f64;
                let x = margin_left + x_inner_pad + (pct * effect_width);

                let y = h - margin_bottom - (temp / 110.0) * plot_height;

                if i == 0 {
                    context.move_to(x, y);
                } else {
                    context.line_to(x, y);
                }
            }
            context.stroke().expect("Failed to stroke GPU line");
        }

        if !plot_data.nvme_temps.is_empty() {
            context.set_source_rgb(0.0, 0.0, 1.0);
            context.set_line_width(2.0);
            for (i, &temp) in plot_data.nvme_temps.iter().enumerate() {
                let pct = i as f64 / num_samples as f64;
                let x = margin_left + x_inner_pad + (pct * effect_width);

                let y = h - margin_bottom - (temp / 110.0) * plot_height;
                if i == 0 {
                    context.move_to(x, y);
                } else {
                    context.line_to(x, y);
                }
            }
            context.stroke().expect("Failed to stroke NVME line");
        }

        // Legend
        let legend_x = w - margin_right - 120.0;
        let legend_y = margin_top + 20.0;

        context.set_source_rgb(1.0, 0.0, 0.0);
        context.rectangle(legend_x, legend_y, 20.0, 10.0);
        context.fill().expect("Failed to fill");
        context.set_source_rgb(0.0, 0.0, 0.0);
        context.move_to(legend_x + 25.0, legend_y + 10.0);
        context.show_text("CPU").expect("Failed to show text");

        context.set_source_rgb(0.0, 0.8, 0.0);
        context.rectangle(legend_x, legend_y + 20.0, 20.0, 10.0);
        context.fill().expect("Failed to fill");
        context.set_source_rgb(0.0, 0.0, 0.0);
        context.move_to(legend_x + 25.0, legend_y + 30.0);
        context.show_text("GPU").expect("Failed to show text");

        context.set_source_rgb(0.0, 0.0, 1.0);
        context.rectangle(legend_x, legend_y + 40.0, 20.0, 10.0);
        context.fill().expect("Failed to fill");
        context.set_source_rgb(0.0, 0.0, 0.0);
        context.move_to(legend_x + 25.0, legend_y + 50.0);
        context.show_text("NVME").expect("Failed to show text");
    });

    content.set_child(Some(&drawing_area));
    let square_container = AspectFrame::builder()
        .ratio(2.5)
        .obey_child(false)
        .margin_top(30)
        .margin_bottom(20)
        .margin_start(20)
        .vexpand(true)
        .hexpand(true)
        .valign(Align::Fill)
        .halign(Align::Fill)
        .margin_end(20)
        .child(&content)
        .build();

    let window = ApplicationWindow::builder()
        .application(app)
        .title("Twatch Temperature Plot")
        .default_width(1000)
        .default_height(400)
        .child(&square_container)
        .build();

    window.present();
}
