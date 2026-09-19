use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Style, Stylize},
    symbols,
    text::{Line, Span},
    widgets::{Axis, Block, BorderType, Borders, Chart, Dataset, GraphType, Paragraph, Row, Table},
    Frame, Terminal,
};
use std::{
    collections::HashMap,
    fs::File,
    io::{self, BufRead, BufReader},
    path::PathBuf,
};

use crate::session::{find_latest, list_sessions};



#[derive(Debug, Clone)]
pub struct SeriesData {
    pub key: String,
    pub session_id: u16,
    pub device_type: String,
    pub label: String,
    pub color: Color,
    pub points: Vec<(f64, f64)>, // (x, y) = (sample_idx or elapsed_sec, temp)
    pub spikes: Vec<(f64, f64)>,
    pub visible: bool,
    pub min_temp: f64,
    pub max_temp: f64,
    pub avg_temp: f64,
}

pub fn run_graph_viewer(
    session_ids: &[u16],
    max_plot_temp: u16,
    _temp_steps: u16,
    initial_full_sensors: bool,
) -> io::Result<()> {
    // Resolve session files
    let paths: Vec<(u16, PathBuf)> = if session_ids.is_empty() {
        let latest = find_latest()?;
        let stem = latest
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("session_1");
        let id = stem.strip_prefix("session_").and_then(|s| s.parse().ok()).unwrap_or(1);
        vec![(id, latest)]
    } else {
        let all = list_sessions()?;
        let mut matched = Vec::new();
        for id in session_ids {
            if let Some((_, p)) = all.iter().find(|(sid, _)| sid == id) {
                matched.push((*id, p.clone()));
            } else {
                eprintln!("Session {} not found.", id);
            }
        }
        if matched.is_empty() {
            return Err(io::Error::new(io::ErrorKind::NotFound, "No matching sessions"));
        }
        matched
    };

    let mut full_sensors = initial_full_sensors;
    // Load series from all specified sessions
    let mut all_series = load_sessions(&paths, full_sensors)?;
    if all_series.is_empty() {
        eprintln!("No sensor data found in specified session(s).");
        return Ok(());
    }

    // Determine global X range and Y range
    let max_x = all_series
        .iter()
        .map(|s| s.points.last().map(|p| p.0).unwrap_or(0.0))
        .fold(0.0, f64::max);

    let observed_max_y = all_series
        .iter()
        .map(|s| s.max_temp)
        .fold(0.0, f64::max);

    let initial_y_max = (max_plot_temp as f64).max(observed_max_y + 5.0).max(80.0);

    // Terminal initialization
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.hide_cursor()?;

    let mut view_x_min = 0.0;
    let mut view_x_max = max_x.max(10.0);
    let mut view_y_max = initial_y_max;
    let mut show_spikes = true;
    let mut marker_mode = 0; // 0 = Braille, 1 = Dot, 2 = Block

    let res = (|| -> io::Result<()> {
        loop {
            terminal.draw(|f| {
                draw_graph_frame(
                    f,
                    &all_series,
                    &paths,
                    view_x_min,
                    view_x_max,
                    view_y_max,
                    show_spikes,
                    marker_mode,
                );
            })?;

            if event::poll(std::time::Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    let span = view_x_max - view_x_min;
                    let step = (span * 0.1).max(1.0);

                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => break,
                        // Pan left/right
                        KeyCode::Left | KeyCode::Char('h') => {
                            if view_x_min > 0.0 {
                                let shift = step.min(view_x_min);
                                view_x_min -= shift;
                                view_x_max -= shift;
                            }
                        }
                        KeyCode::Right | KeyCode::Char('l') => {
                            if view_x_max < max_x + step {
                                view_x_min += step;
                                view_x_max += step;
                            }
                        }
                        // Zoom in/out X
                        KeyCode::Char('+') | KeyCode::Char('=') => {
                            if span > 5.0 {
                                view_x_min += step / 2.0;
                                view_x_max -= step / 2.0;
                            }
                        }
                        KeyCode::Char('-') | KeyCode::Char('_') => {
                            view_x_min = (view_x_min - step / 2.0).max(0.0);
                            view_x_max = (view_x_max + step / 2.0).min(max_x * 1.2);
                        }
                        // Zoom Y
                        KeyCode::Up | KeyCode::Char('k') => {
                            view_y_max = (view_y_max - 5.0).max(40.0);
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            view_y_max = (view_y_max + 5.0).min(150.0);
                        }
                        // Home / End
                        KeyCode::Home => {
                            view_x_min = 0.0;
                            view_x_max = span;
                        }
                        KeyCode::End => {
                            view_x_max = max_x;
                            view_x_min = (max_x - span).max(0.0);
                        }
                        // Toggle series 1-9
                        KeyCode::Char(c @ '1'..='9') => {
                            let idx = (c as usize) - ('1' as usize);
                            if idx < all_series.len() {
                                all_series[idx].visible = !all_series[idx].visible;
                            }
                        }
                        // Toggle all
                        KeyCode::Char('a') => {
                            let any_hidden = all_series.iter().any(|s| !s.visible);
                            for s in &mut all_series {
                                s.visible = any_hidden;
                            }
                        }
                        // Toggle spikes
                        KeyCode::Char('s') => {
                            show_spikes = !show_spikes;
                        }
                        // Toggle full sensors vs squashed NVMe
                        KeyCode::Char('f') | KeyCode::Char('F') => {
                            full_sensors = !full_sensors;
                            if let Ok(reloaded) = load_sessions(&paths, full_sensors) {
                                all_series = reloaded;
                            }
                        }
                        // Marker mode
                        KeyCode::Char('m') => {
                            marker_mode = (marker_mode + 1) % 3;
                        }
                        // Reset view
                        KeyCode::Char('r') => {
                            view_x_min = 0.0;
                            view_x_max = max_x.max(10.0);
                            view_y_max = initial_y_max;
                        }
                        _ => {}
                    }
                }
            }
        }
        Ok(())
    })();

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor().ok();

    res
}

fn load_sessions(paths: &[(u16, PathBuf)], full_devices_sensors: bool) -> io::Result<Vec<SeriesData>> {
    let mut series_map: HashMap<(u16, String, String), Vec<(f64, f64)>> = HashMap::new();

    let palette = [
        Color::Cyan,
        Color::Green,
        Color::Yellow,
        Color::Rgb(255, 140, 0),
        Color::Magenta,
        Color::LightBlue,
        Color::LightRed,
        Color::LightCyan,
        Color::Rgb(180, 100, 255),
        Color::White,
    ];

    for (session_id, path) in paths {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let mut sample_idx = 0.0;

        for line in reader.lines().filter_map(Result::ok) {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with("Type,") || line.starts_with("Time,") {
                continue;
            }

            let parts: Vec<&str> = line.split(',').collect();
            if parts.len() >= 5 {
                // New format: Time,Elapsed,Type,Label,Temp
                let elapsed: f64 = parts[1].parse().unwrap_or(sample_idx);
                let typ = parts[2].to_string();
                let label = parts[3].to_string();
                let temp: f64 = parts[4].parse().unwrap_or(0.0);
                series_map
                    .entry((*session_id, typ, label))
                    .or_default()
                    .push((elapsed, temp));
            } else if parts.len() >= 3 {
                // Legacy format: Type,Label,Temp
                let typ = parts[0].to_string();
                let label = parts[1].to_string();
                let temp: f64 = parts[2].parse().unwrap_or(0.0);
                series_map
                    .entry((*session_id, typ, label))
                    .or_default()
                    .push((sample_idx, temp));
                sample_idx += 1.0;
            }
        }
    }

    if !full_devices_sensors {
        let mut keep_keys = std::collections::HashSet::new();
        let mut nvme_groups: HashMap<(u16, String), Vec<((u16, String, String), f64)>> = HashMap::new();

        for ((sid, dev_type, label), pts) in &series_map {
            if dev_type == "NVME" {
                let max_t = pts.iter().map(|p| p.1).fold(0.0, f64::max);
                let dev_key = if let Some((prefix, _)) = label.split_once(':') {
                    prefix.to_string()
                } else {
                    "nvme".to_string()
                };
                nvme_groups
                    .entry((*sid, dev_key))
                    .or_default()
                    .push(((*sid, dev_type.clone(), label.clone()), max_t));
            }
        }

        for (_, list) in nvme_groups {
            let chosen = list
                .iter()
                .find(|((_, _, lbl), _)| lbl.to_lowercase().contains("composite"))
                .or_else(|| list.iter().max_by(|a, b| a.1.partial_cmp(&b.1).unwrap()));
            if let Some((k, _)) = chosen {
                keep_keys.insert(k.clone());
            }
        }

        series_map.retain(|k, _| k.1 != "NVME" || keep_keys.contains(k));
    }

    let mut result = Vec::new();
    let multi = paths.len() > 1;
    let mut color_idx = 0;

    for ((session_id, dev_type, label), mut points) in series_map {
        if points.is_empty() {
            continue;
        }

        // Sort by X coordinate
        points.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

        let min_temp = points.iter().map(|p| p.1).fold(f64::INFINITY, f64::min);
        let max_temp = points.iter().map(|p| p.1).fold(f64::NEG_INFINITY, f64::max);
        let sum_temp: f64 = points.iter().map(|p| p.1).sum();
        let avg_temp = sum_temp / points.len() as f64;

        // Detect heat spikes (top 10% highest positive jumps)
        let spikes = detect_heat_spikes(&points);

        let key = if multi {
            format!("S{} {}:{}", session_id, dev_type, label)
        } else {
            format!("{}:{}", dev_type, label)
        };

        let color = palette[color_idx % palette.len()];
        color_idx += 1;

        result.push(SeriesData {
            key,
            session_id,
            device_type: dev_type,
            label,
            color,
            points,
            spikes,
            visible: true,
            min_temp,
            max_temp,
            avg_temp,
        });
    }

    // Sort series: CPUs first, GPUs second, etc.
    result.sort_by(|a, b| {
        a.session_id
            .cmp(&b.session_id)
            .then_with(|| a.device_type.cmp(&b.device_type))
            .then_with(|| a.label.cmp(&b.label))
    });

    Ok(result)
}

fn detect_heat_spikes(points: &[(f64, f64)]) -> Vec<(f64, f64)> {
    if points.len() < 4 {
        return Vec::new();
    }

    let mut diffs: Vec<(usize, f64)> = Vec::new();
    for i in 1..points.len() {
        let dt = (points[i].0 - points[i - 1].0).max(0.1);
        let dy = points[i].1 - points[i - 1].1;
        if dy > 0.0 {
            diffs.push((i, dy / dt));
        }
    }

    if diffs.is_empty() {
        return Vec::new();
    }

    diffs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let top_n = (points.len() / 10).clamp(1, 10);

    let mut spikes = Vec::new();
    for (idx, _) in diffs.into_iter().take(top_n) {
        spikes.push(points[idx]);
    }
    spikes.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    spikes
}

fn draw_graph_frame(
    frame: &mut Frame,
    series: &[SeriesData],
    paths: &[(u16, PathBuf)],
    x_min: f64,
    x_max: f64,
    y_max: f64,
    show_spikes: bool,
    marker_mode: usize,
) {
    let area = frame.area();
    if area.width < 40 || area.height < 12 {
        let msg = Paragraph::new("Terminal too small for graph viewer")
            .style(Style::default().fg(Color::Yellow));
        frame.render_widget(msg, area);
        return;
    }

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(0),    // Chart
            Constraint::Length(7), // Legend / Stats table
            Constraint::Length(1), // Footer controls
        ])
        .split(area);

    // 1. Header
    let title = if paths.len() == 1 {
        format!(" Session #{} — {} ", paths[0].0, paths[0].1.display())
    } else {
        let ids: Vec<String> = paths.iter().map(|(id, _)| format!("#{}", id)).collect();
        format!(" Comparative Session Graph [{}] ", ids.join(" vs "))
    };

    let marker_name = match marker_mode {
        0 => "Braille",
        1 => "Dot",
        _ => "Block",
    };

    let header = Paragraph::new(Line::from(vec![
        Span::styled(" twatch graph ", Style::default().fg(Color::Black).bg(Color::Cyan).bold()),
        Span::raw(" "),
        Span::styled(title, Style::default().fg(Color::White).bold()),
        Span::raw(" | "),
        Span::styled(format!("Window: {:.1}s..{:.1}s ", x_min, x_max), Style::default().fg(Color::Yellow)),
        Span::raw(" | "),
        Span::styled(format!("Marker: {} ", marker_name), Style::default().fg(Color::DarkGray)),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    frame.render_widget(header, layout[0]);

    // 2. Build Datasets
    let marker = match marker_mode {
        0 => symbols::Marker::Braille,
        1 => symbols::Marker::Dot,
        _ => symbols::Marker::Block,
    };

    let mut datasets = Vec::new();
    let mut spike_points = Vec::new();

    for s in series {
        if !s.visible {
            continue;
        }

        let ds = Dataset::default()
            .name(s.key.clone())
            .marker(marker)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(s.color))
            .data(&s.points);
        datasets.push(ds);

        if show_spikes {
            for sp in &s.spikes {
                if sp.0 >= x_min && sp.0 <= x_max {
                    spike_points.push(*sp);
                }
            }
        }
    }

    // Heat spikes dataset
    let spike_ds;
    if show_spikes && !spike_points.is_empty() {
        spike_ds = Dataset::default()
            .name("▼ Heat Spikes".to_string())
            .marker(symbols::Marker::Dot)
            .graph_type(GraphType::Scatter)
            .style(Style::default().fg(Color::Red).bold())
            .data(&spike_points);
        datasets.push(spike_ds);
    }

    let x_mid = (x_min + x_max) / 2.0;
    let y_bounds = [0.0, y_max];

    let chart = Chart::new(datasets)
        .block(
            Block::default()
                .title(Span::styled(" Temperature Curves (°C) ", Style::default().fg(Color::Cyan).bold()))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .x_axis(
            Axis::default()
                .title("Time / Sample")
                .style(Style::default().fg(Color::DarkGray))
                .bounds([x_min, x_max])
                .labels(vec![
                    Span::raw(format!("{:.1}s", x_min)),
                    Span::raw(format!("{:.1}s", x_mid)),
                    Span::raw(format!("{:.1}s", x_max)),
                ]),
        )
        .y_axis(
            Axis::default()
                .title("°C")
                .style(Style::default().fg(Color::DarkGray))
                .bounds(y_bounds)
                .labels(vec![
                    Span::raw("0°C"),
                    Span::raw(format!("{:.0}°C", y_max / 4.0)),
                    Span::raw(format!("{:.0}°C", y_max / 2.0)),
                    Span::raw(format!("{:.0}°C", y_max * 3.0 / 4.0)),
                    Span::raw(format!("{:.0}°C", y_max)),
                ]),
        );
    frame.render_widget(chart, layout[1]);

    // 3. Sensor Legend Table
    let table_block = Block::default()
        .title(Span::styled(" Series & Statistics ", Style::default().fg(Color::Green).bold()))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::DarkGray));

    let header_row = Row::new(vec!["KEY", "SERIES", "POINTS", "MIN", "AVG", "MAX", "PEAK SPIKE", "STATUS"])
        .style(Style::default().fg(Color::DarkGray).bold());

    let rows: Vec<Row> = series
        .iter()
        .enumerate()
        .map(|(idx, s)| {
            let key_shortcut = if idx < 9 {
                format!("[{}]", idx + 1)
            } else {
                "   ".to_string()
            };
            let status = if s.visible { "[ON]" } else { "[OFF]" };
            let status_color = if s.visible { Color::Green } else { Color::DarkGray };

            let peak_spike = s
                .spikes
                .iter()
                .map(|p| p.1)
                .fold(0.0, f64::max);
            let spike_str = if peak_spike > 0.0 {
                format!("▼ {:.1}°C", peak_spike)
            } else {
                "-".to_string()
            };

            Row::new(vec![
                Span::styled(key_shortcut, Style::default().fg(Color::Yellow).bold()),
                Span::styled(s.key.clone(), Style::default().fg(s.color).bold()),
                Span::raw(format!("{}", s.points.len())),
                Span::raw(format!("{:.1}°C", s.min_temp)),
                Span::raw(format!("{:.1}°C", s.avg_temp)),
                Span::styled(format!("{:.1}°C", s.max_temp), Style::default().fg(Color::White).bold()),
                Span::styled(spike_str, Style::default().fg(Color::LightRed)),
                Span::styled(status, Style::default().fg(status_color).bold()),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(5),
        Constraint::Length(24),
        Constraint::Length(8),
        Constraint::Length(10),
        Constraint::Length(10),
        Constraint::Length(10),
        Constraint::Length(14),
        Constraint::Min(8),
    ];

    let table = Table::new(rows, widths).header(header_row).block(table_block);
    frame.render_widget(table, layout[2]);

    // 4. Footer controls
    let footer = Paragraph::new(Line::from(vec![
        Span::styled(" [←/→] ", Style::default().bold().fg(Color::Cyan)),
        Span::raw("Pan  "),
        Span::styled(" [+/-] ", Style::default().bold().fg(Color::Cyan)),
        Span::raw("Zoom X  "),
        Span::styled(" [↑/↓] ", Style::default().bold().fg(Color::Cyan)),
        Span::raw("Zoom Y  "),
        Span::styled(" [1-9] ", Style::default().bold().fg(Color::Yellow)),
        Span::raw("Toggle Series  "),
        Span::styled(" [a] ", Style::default().bold().fg(Color::Yellow)),
        Span::raw("All  "),
        Span::styled(" [f] ", Style::default().bold().fg(Color::LightBlue)),
        Span::raw("NVMe Squash/Full  "),
        Span::styled(" [s] ", Style::default().bold().fg(Color::LightRed)),
        Span::raw("Spikes  "),
        Span::styled(" [m] ", Style::default().bold().fg(Color::Green)),
        Span::raw("Marker  "),
        Span::styled(" [r] ", Style::default().bold().fg(Color::Magenta)),
        Span::raw("Reset  "),
        Span::styled(" [q] ", Style::default().bold().fg(Color::White)),
        Span::raw("Quit"),
    ]))
    .alignment(Alignment::Center);
    frame.render_widget(footer, layout[3]);
}
