use crate::{
    plot::run_graph_viewer,
    sensors::{
        device_type, format_bytes_per_sec, read_cpu_freq, read_gpu_freq, read_nvme_diskstats,
        squash_sensors, CpuFreq, DeviceKind, SensorReading, SensorRegistry,
    },
    Config,
};
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    symbols,
    text::{Line, Span},
    widgets::{
        Axis, Block, BorderType, Borders, Chart, Clear, Dataset, GraphType, Paragraph,
        Row, Table,
    },
    Frame, Terminal,
};
use std::{
    collections::{HashMap, VecDeque},
    fs::{self, File},
    io::{self, BufRead, BufReader, Write},
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[allow(dead_code)]
pub struct SessionFile {
    pub id: u16,
    pub path: PathBuf,
    pub file: File,
    pub buffer: Vec<String>,
    pub flush_interval: usize,
    pub start_instant: Instant,
    pub sample_count: usize,
    pub peak_temp: f64,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SessionMeta {
    pub id: u16,
    pub path: PathBuf,
    pub sample_count: usize,
    pub duration_sec: Option<f64>,
    pub peak_temp: Option<f64>,
    pub delay_ms: Option<u64>,
    pub sensors: Vec<String>,
}

/// Returns primary XDG session directory: `$XDG_DATA_HOME/twatch/session` or `~/.local/share/twatch/session`
pub fn get_primary_session_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        if !xdg.is_empty() {
            return PathBuf::from(xdg).join("twatch").join("session");
        }
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
        .join(".local")
        .join("share")
        .join("twatch")
        .join("session")
}

/// Returns legacy session directory: `~/Documents/Twatch/session`
pub fn get_legacy_session_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join("Documents").join("Twatch").join("session")
}

/// Lists all sessions from primary and legacy directories, sorted numerically by ID.
pub fn list_sessions() -> io::Result<Vec<(u16, PathBuf)>> {
    let mut sessions = Vec::new();
    let dirs = [get_primary_session_dir(), get_legacy_session_dir()];

    for dir in &dirs {
        if !dir.exists() {
            continue;
        }
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.filter_map(|r| r.ok()) {
                let path = entry.path();
                if path.extension().map_or(false, |ext| ext == "csv") {
                    if let Some(stem) = path.file_stem() {
                        let name = stem.to_string_lossy();
                        if let Some(num_str) = name.strip_prefix("session_") {
                            if let Ok(id) = num_str.parse::<u16>() {
                                // Avoid duplicate ID if present in both
                                if !sessions.iter().any(|(sid, _)| *sid == id) {
                                    sessions.push((id, path));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Sort numerically by ID
    sessions.sort_by_key(|(id, _)| *id);
    Ok(sessions)
}

/// Loads detailed metadata for each session by inspecting header and summary.
pub fn list_sessions_detailed() -> io::Result<Vec<SessionMeta>> {
    let pairs = list_sessions()?;
    let mut metas = Vec::with_capacity(pairs.len());

    for (id, path) in pairs {
        let mut sample_count = 0;
        let mut duration_sec = None;
        let mut peak_temp = None;
        let mut delay_ms = None;
        let mut sensor_set = Vec::new();

        if let Ok(file) = File::open(&path) {
            let reader = BufReader::new(file);
            for line in reader.lines().filter_map(Result::ok) {
                let trimmed = line.trim();
                if trimmed.starts_with("# Delay:") {
                    if let Ok(d) = trimmed["# Delay:".len()..].trim().parse::<u64>() {
                        delay_ms = Some(d);
                    }
                } else if trimmed.starts_with("# Total:") {
                    if let Ok(t) = trimmed["# Total:".len()..].trim().parse::<f64>() {
                        duration_sec = Some(t);
                    }
                } else if trimmed.starts_with("# Peak:") {
                    if let Ok(p) = trimmed["# Peak:".len()..].trim().parse::<f64>() {
                        peak_temp = Some(p);
                    }
                } else if !trimmed.starts_with('#') && !trimmed.starts_with("Type,") && !trimmed.starts_with("Time,") {
                    let parts: Vec<&str> = trimmed.split(',').collect();
                    if parts.len() >= 3 {
                        sample_count += 1;
                        let (typ, label, temp_str) = if parts.len() >= 5 {
                            (parts[2], parts[3], parts[4])
                        } else {
                            (parts[0], parts[1], parts[2])
                        };
                        let name = format!("{}:{}", typ, label);
                        if !sensor_set.contains(&name) && sensor_set.len() < 5 {
                            sensor_set.push(name);
                        }
                        if let Ok(temp) = temp_str.parse::<f64>() {
                            peak_temp = Some(peak_temp.map_or(temp, |p: f64| p.max(temp)));
                        }
                    }
                }
            }
        }

        metas.push(SessionMeta {
            id,
            path,
            sample_count,
            duration_sec,
            peak_temp,
            delay_ms,
            sensors: sensor_set,
        });
    }

    Ok(metas)
}

/// Delete sessions by IDs. Returns count of deleted files.
pub fn delete_sessions(ids: &[u16]) -> io::Result<usize> {
    let all = list_sessions()?;
    let mut deleted = 0;
    for (id, path) in all {
        if ids.contains(&id) {
            fs::remove_file(&path)?;
            deleted += 1;
        }
    }
    Ok(deleted)
}

pub fn find_latest() -> io::Result<PathBuf> {
    let sessions = list_sessions()?;
    sessions
        .last()
        .map(|(_, p)| p.clone())
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "No sessions found"))
}

pub fn session_writer(delay: u64) -> io::Result<SessionFile> {
    let session_dir = get_primary_session_dir();
    fs::create_dir_all(&session_dir)?;

    let existing = list_sessions().unwrap_or_default();
    let max_id = existing.iter().map(|(id, _)| *id).max().unwrap_or(0);
    let session_id = if existing.is_empty() { 1 } else { max_id + 1 };

    let candidate = session_dir.join(format!("session_{}.csv", session_id));
    let mut file = File::create(&candidate)?;

    let now_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    writeln!(file, "# Twatch Session v2")?;
    writeln!(file, "# Delay:{}", delay)?;
    writeln!(file, "# StartTime:{}", now_epoch)?;
    writeln!(file, "Time,Elapsed,Type,Label,Temp")?;

    Ok(SessionFile {
        id: session_id,
        path: candidate,
        file,
        buffer: Vec::with_capacity(64),
        flush_interval: 32,
        start_instant: Instant::now(),
        sample_count: 0,
        peak_temp: 0.0,
    })
}

pub fn flush_buffer(session: &mut SessionFile) -> io::Result<()> {
    for line in &session.buffer {
        writeln!(session.file, "{}", line)?;
    }
    session.file.flush()?;
    session.buffer.clear();
    Ok(())
}

pub fn record_frame(session: &mut SessionFile, sensors: &[SensorReading]) -> io::Result<()> {
    let elapsed = session.start_instant.elapsed().as_secs_f64();
    let now_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64();

    for sensor in sensors {
        let d_type = device_type(sensor);
        if d_type == "OTHER" && sensor.label == "Unknown" {
            continue;
        }
        if sensor.temp > session.peak_temp {
            session.peak_temp = sensor.temp;
        }
        session.buffer.push(format!(
            "{:.2},{:.2},{},{},{:.1}",
            now_epoch, elapsed, d_type, sensor.label, sensor.temp
        ));
    }
    session.sample_count += 1;

    if session.buffer.len() >= session.flush_interval {
        flush_buffer(session)?;
    }
    Ok(())
}

pub fn format_json_frame(sensors: &[SensorReading], elapsed_sec: f64) -> String {
    let mut parts = vec![format!("\"elapsed\":{:.2}", elapsed_sec)];
    for s in sensors {
        let d_type = device_type(s);
        let key = format!("{}_{}", d_type.to_lowercase(), s.label.to_lowercase().replace(' ', "_"));
        parts.push(format!("\"{}\":{:.1}", key, s.temp));
    }
    format!("{{{}}}", parts.join(", "))
}

pub fn target_temp(sensors: &[SensorReading], sensor_kind: &str) -> f64 {
    match sensor_kind {
        "gpu" => sensors.iter().find(|s| s.is_gpu()).map(|s| s.temp).unwrap_or(0.0),
        "nvme" => sensors.iter().find(|s| s.is_nvme()).map(|s| s.temp).unwrap_or(0.0),
        _ => sensors.iter().find(|s| s.is_cpu()).map(|s| s.temp).unwrap_or(0.0),
    }
}

/// Smooth temperature-to-color mapping (btop style)
pub fn temp_to_color(temp: f64) -> Color {
    if temp < 40.0 {
        Color::Cyan
    } else if temp < 55.0 {
        Color::Green
    } else if temp < 70.0 {
        Color::Yellow
    } else if temp < 85.0 {
        Color::Rgb(255, 140, 0) // Amber / Orange
    } else {
        Color::Rgb(255, 50, 70) // Hot Red
    }
}

/// Per-sensor tracker for live statistics & chart histories
#[derive(Default)]
pub struct SensorHistory {
    pub history: VecDeque<(f64, f64)>, // (elapsed_sec, temp)
    pub sparkline_data: VecDeque<u64>,
    pub min_temp: f64,
    pub max_temp: f64,
    pub sum_temp: f64,
    pub count: usize,
    pub prev_temp: f64,
    pub temp_delta: f64, // °C change in last reading
}

impl SensorHistory {
    pub fn update(&mut self, elapsed_sec: f64, temp: f64, max_history: usize) {
        if self.count == 0 {
            self.min_temp = temp;
            self.max_temp = temp;
            self.prev_temp = temp;
        } else {
            self.temp_delta = temp - self.prev_temp;
            self.prev_temp = temp;
            if temp < self.min_temp {
                self.min_temp = temp;
            }
            if temp > self.max_temp {
                self.max_temp = temp;
            }
        }
        self.sum_temp += temp;
        self.count += 1;

        self.history.push_back((elapsed_sec, temp));
        if self.history.len() > max_history {
            self.history.pop_front();
        }

        // Scaled sparkline data (0..100)
        let spark_val = (temp.clamp(0.0, 100.0)) as u64;
        self.sparkline_data.push_back(spark_val);
        if self.sparkline_data.len() > 30 {
            self.sparkline_data.pop_front();
        }
    }

    pub fn avg(&self) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            self.sum_temp / self.count as f64
        }
    }
}

pub struct LiveState {
    pub histories: HashMap<String, SensorHistory>,
    pub paused: bool,
    pub show_help: bool,
    pub start_time: Instant,
    pub max_plot_temp: f64,
    pub full_devices_sensors: bool,
    pub cpu_freq: Option<CpuFreq>,
    pub gpu_freq: Option<f64>,
    /// NVMe I/O: device_name → (read_sectors, write_sectors, last_instant)
    pub nvme_io_prev: HashMap<String, (u64, u64, Instant)>,
    /// NVMe I/O rates: device_name → (read_bps, write_bps)
    pub nvme_io_rates: HashMap<String, (f64, f64)>,
}

impl LiveState {
    pub fn new(max_plot_temp: f64, full_devices_sensors: bool) -> Self {
        let now = Instant::now();
        // Seed the prev snapshot so the first tick has a delta reference
        let initial_stats = read_nvme_diskstats();
        let nvme_io_prev: HashMap<String, (u64, u64, Instant)> = initial_stats
            .into_iter()
            .map(|(dev, (r, w))| (dev, (r, w, now)))
            .collect();
        Self {
            histories: HashMap::new(),
            paused: false,
            show_help: false,
            start_time: now,
            max_plot_temp,
            full_devices_sensors,
            cpu_freq: read_cpu_freq(),
            gpu_freq: read_gpu_freq(),
            nvme_io_prev,
            nvme_io_rates: HashMap::new(),
        }
    }

    pub fn update(&mut self, sensors: &[SensorReading], elapsed_sec: f64) {
        if self.paused {
            return;
        }
        self.cpu_freq = read_cpu_freq();
        self.gpu_freq = read_gpu_freq();

        // Update NVMe I/O rates
        let now = Instant::now();
        let current_stats = read_nvme_diskstats();
        for (dev, (cur_r, cur_w)) in &current_stats {
            if let Some((prev_r, prev_w, prev_t)) = self.nvme_io_prev.get(dev) {
                let dt = now.duration_since(*prev_t).as_secs_f64();
                if dt > 0.0 {
                    // Each sector = 512 bytes
                    let read_bps = (cur_r.saturating_sub(*prev_r) as f64 * 512.0) / dt;
                    let write_bps = (cur_w.saturating_sub(*prev_w) as f64 * 512.0) / dt;
                    self.nvme_io_rates.insert(dev.clone(), (read_bps, write_bps));
                }
            }
        }
        // Update prev snapshot
        for (dev, (cur_r, cur_w)) in current_stats {
            self.nvme_io_prev.insert(dev, (cur_r, cur_w, now));
        }

        for s in sensors {
            let key = format!("{}:{}", s.kind, s.label);
            self.histories
                .entry(key)
                .or_default()
                .update(elapsed_sec, s.temp, 90);
        }
    }
}

fn draw_btop_frame(
    frame: &mut Frame,
    sensors: &[SensorReading],
    state: &LiveState,
    session_id: Option<u16>,
    mode_label: &str,
    status_info: &str,
    delay_ms: u64,
) {
    let area = frame.area();
    if area.width < 30 || area.height < 10 {
        let msg = Paragraph::new("Terminal too small for twatch TUI")
            .style(Style::default().fg(Color::Yellow));
        frame.render_widget(msg, area);
        return;
    }

    // Top Header (3), Main Body (Min(0)), Bottom Status/Keys (1)
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);

    let session_tag = match session_id {
        Some(id) => format!(" Session #{} [RECORDING] ", id),
        None => " LIVE MONITOR ".to_string(),
    };
    let pause_tag = if state.paused { " [PAUSED] " } else { "" };
    let elapsed = state.start_time.elapsed().as_secs();
    let elapsed_str = format!("{:02}:{:02}", elapsed / 60, elapsed % 60);

    let mut header_spans = vec![
        Span::styled(" twatch ", Style::default().fg(Color::Black).bg(Color::Cyan).bold()),
        Span::raw(" "),
        Span::styled(session_tag, Style::default().fg(Color::White).bold()),
        Span::styled(pause_tag, Style::default().fg(Color::Yellow).bold()),
        Span::raw(" | "),
        Span::styled(format!("Time: {} ", elapsed_str), Style::default().fg(Color::Cyan)),
        Span::raw(" | "),
        Span::styled(format!("Rate: {}ms ", delay_ms), Style::default().fg(Color::DarkGray)),
    ];

    if let Some(cf) = state.cpu_freq {
        header_spans.push(Span::raw(" | "));
        header_spans.push(Span::styled(
            format!("CPU: {:.2} GHz ", cf.max_ghz),
            Style::default().fg(Color::Yellow).bold(),
        ));
    }

    if let Some(gf) = state.gpu_freq {
        header_spans.push(Span::raw(" | "));
        header_spans.push(Span::styled(
            format!("GPU: {:.2} GHz ", gf),
            Style::default().fg(Color::Green).bold(),
        ));
    }

    header_spans.push(Span::raw(" | "));
    header_spans.push(Span::styled(status_info, Style::default().fg(Color::White)));

    let header = Paragraph::new(Line::from(header_spans))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::DarkGray))
            .title(Span::styled(
                format!(" {} ", mode_label),
                Style::default().fg(Color::Cyan).bold(),
            )),
    );
    frame.render_widget(header, layout[0]);

    // 2. Main Body Split: Top = Chart (45%), Bottom = Sensors Matrix (55%)
    let body_split = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(layout[1]);

    // Prepare chart datasets for primary sensors
    let mut datasets = Vec::new();
    let sensor_keys: Vec<String> = sensors
        .iter()
        .take(8)
        .map(|s| format!("{}:{}", s.kind, s.label))
        .collect();

    // Chart palette
    let colors = [
        Color::Cyan,
        Color::Green,
        Color::Yellow,
        Color::Rgb(255, 120, 50),
        Color::Magenta,
        Color::LightBlue,
        Color::LightRed,
        Color::White,
    ];

    let mut min_time = 0.0;
    let mut max_time = 60.0;
    let mut max_observed_temp = state.max_plot_temp;

    for (idx, key) in sensor_keys.iter().enumerate() {
        if let Some(hist) = state.histories.get(key) {
            if !hist.history.is_empty() {
                let first_t = hist.history.front().unwrap().0;
                let last_t = hist.history.back().unwrap().0;
                if first_t < min_time || min_time == 0.0 {
                    min_time = first_t;
                }
                if last_t > max_time {
                    max_time = last_t;
                }
                if hist.max_temp > max_observed_temp {
                    max_observed_temp = (hist.max_temp + 5.0).ceil();
                }

                let color = colors[idx % colors.len()];
                let ds = Dataset::default()
                    .name(key.clone())
                    .marker(symbols::Marker::Braille)
                    .graph_type(GraphType::Line)
                    .style(Style::default().fg(color))
                    .data(&hist.history.as_slices().0);
                datasets.push(ds);
            }
        }
    }

    let x_bounds = [min_time, (max_time + 1.0).max(min_time + 10.0)];
    let y_bounds = [20.0, max_observed_temp.max(90.0)];

    let chart_title = format!(" Real-time Temperature Trend (°C) — {}s window ", (x_bounds[1] - x_bounds[0]) as u64);
    let chart = Chart::new(datasets)
        .block(
            Block::default()
                .title(Span::styled(chart_title, Style::default().fg(Color::Cyan).bold()))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .x_axis(
            Axis::default()
                .title("Elapsed (s)")
                .style(Style::default().fg(Color::DarkGray))
                .bounds(x_bounds)
                .labels(vec![
                    Span::raw(format!("{:.0}s", x_bounds[0])),
                    Span::raw(format!("{:.0}s", (x_bounds[0] + x_bounds[1]) / 2.0)),
                    Span::raw(format!("{:.0}s", x_bounds[1])),
                ]),
        )
        .y_axis(
            Axis::default()
                .title("°C")
                .style(Style::default().fg(Color::DarkGray))
                .bounds(y_bounds)
                .labels(vec![
                    Span::raw(format!("{:.0}°", y_bounds[0])),
                    Span::raw(format!("{:.0}°", (y_bounds[0] + y_bounds[1]) / 2.0)),
                    Span::raw(format!("{:.0}°", y_bounds[1])),
                ]),
        );
    frame.render_widget(chart, body_split[0]);

    // 3. Sensor Table / Gauges Matrix
    let table_block = Block::default()
        .title(Span::styled(" Thermal Sensors ", Style::default().fg(Color::Green).bold()))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::DarkGray));

    let header_cells = ["DEVICE", "LABEL", "TEMP", "MIN", "AVG", "MAX", "RATE", "LOAD BAR"]
        .iter()
        .map(|h| Span::styled(*h, Style::default().fg(Color::DarkGray).bold()));
    let header_row = Row::new(header_cells).height(1).bottom_margin(0);

    let display_sensors = squash_sensors(sensors, state.full_devices_sensors);

    let rows: Vec<Row> = display_sensors
        .iter()
        .map(|s| {
            let key = format!("{}:{}", s.kind, s.label);
            let hist = state.histories.get(&key);
            let color = temp_to_color(s.temp);

            // Build label with optional frequency suffix for CPU/GPU,
            // or I/O rates for NVMe devices
            let label_cell: Vec<Span> = match s.kind {
                DeviceKind::Cpu => {
                    let text = if let Some(cf) = state.cpu_freq {
                        format!("{} ({:.2} GHz)", s.label, cf.max_ghz)
                    } else {
                        s.label.clone()
                    };
                    vec![Span::styled(text, Style::default().fg(Color::White))]
                }
                DeviceKind::Gpu => {
                    let text = if let Some(gf) = state.gpu_freq {
                        format!("{} ({:.2} GHz)", s.label, gf)
                    } else {
                        s.label.clone()
                    };
                    vec![Span::styled(text, Style::default().fg(Color::White))]
                }
                DeviceKind::Nvme => {
                    // Look up I/O rates by device_name (e.g. "nvme0")
                    let dev_key = &s.device_name;
                    let (read_bps, write_bps) = state
                        .nvme_io_rates
                        .get(dev_key)
                        .copied()
                        .unwrap_or((0.0, 0.0));
                    vec![
                        Span::styled(s.label.clone(), Style::default().fg(Color::White)),
                        Span::raw("  "),
                        Span::styled("↑", Style::default().fg(Color::Green).bold()),
                        Span::raw(format!(" {} ", format_bytes_per_sec(read_bps))),
                        Span::styled("↓", Style::default().fg(Color::Magenta).bold()),
                        Span::raw(format!(" {}", format_bytes_per_sec(write_bps))),
                    ]
                }
                _ => vec![Span::styled(s.label.clone(), Style::default().fg(Color::White))],
            };

            let min_s = hist.map_or("-".to_string(), |h| format!("{:.1}°", h.min_temp));
            let max_s = hist.map_or("-".to_string(), |h| format!("{:.1}°", h.max_temp));
            let avg_s = hist.map_or("-".to_string(), |h| format!("{:.1}°", h.avg()));

            let delta_s = match hist {
                Some(h) if h.temp_delta.abs() >= 0.1 => {
                    if h.temp_delta > 0.0 {
                        format!("▲ +{:.1}", h.temp_delta)
                    } else {
                        format!("▼ {:.1}", h.temp_delta)
                    }
                }
                _ => "  -- ".to_string(),
            };

            let delta_color = if delta_s.starts_with('▲') {
                Color::Rgb(255, 100, 100)
            } else if delta_s.starts_with('▼') {
                Color::Cyan
            } else {
                Color::DarkGray
            };

            // Gauge bar characters
            let bar_len: usize = 16;
            let ratio = (s.temp / 100.0).clamp(0.0, 1.0);
            let filled = (ratio * bar_len as f64).round() as usize;
            let empty = bar_len.saturating_sub(filled);
            let bar_str = format!("[{}{}]", "■".repeat(filled), " ".repeat(empty));

            Row::new(vec![
                ratatui::text::Text::from(Line::from(vec![
                    Span::styled(format!("[{}]", s.kind), Style::default().fg(Color::Cyan).bold()),
                ])),
                ratatui::text::Text::from(Line::from(label_cell)),
                ratatui::text::Text::from(Line::from(vec![
                    Span::styled(format!("{:.1}°C", s.temp), Style::default().fg(color).bold()),
                ])),
                ratatui::text::Text::from(Line::from(vec![
                    Span::styled(min_s, Style::default().fg(Color::DarkGray)),
                ])),
                ratatui::text::Text::from(Line::from(vec![
                    Span::styled(avg_s, Style::default().fg(Color::DarkGray)),
                ])),
                ratatui::text::Text::from(Line::from(vec![
                    Span::styled(max_s, Style::default().fg(Color::DarkGray)),
                ])),
                ratatui::text::Text::from(Line::from(vec![
                    Span::styled(delta_s, Style::default().fg(delta_color)),
                ])),
                ratatui::text::Text::from(Line::from(vec![
                    Span::styled(bar_str, Style::default().fg(color)),
                ])),
            ])
            .height(1)
        })
        .collect();

    let widths = [
        Constraint::Length(8),
        Constraint::Min(28),
        Constraint::Length(10),
        Constraint::Length(8),
        Constraint::Length(8),
        Constraint::Length(8),
        Constraint::Length(10),
        Constraint::Length(18),
    ];

    let table = Table::new(rows, widths)
        .header(header_row)
        .block(table_block)
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    frame.render_widget(table, body_split[1]);

    // 4. Footer Help / Keybindings
    let full_tag = if state.full_devices_sensors {
        "Full NVMe [ON]  "
    } else {
        "Squash NVMe [ON]  "
    };

    let footer_text = Line::from(vec![
        Span::styled(" [q] ", Style::default().fg(Color::Black).bg(Color::Cyan).bold()),
        Span::raw("Quit & Save  "),
        Span::styled(" [p] ", Style::default().fg(Color::Black).bg(Color::Yellow).bold()),
        Span::raw("Pause/Resume  "),
        Span::styled(" [f] ", Style::default().fg(Color::Black).bg(Color::LightBlue).bold()),
        Span::raw(full_tag),
        Span::styled(" [h/?] ", Style::default().fg(Color::Black).bg(Color::Green).bold()),
        Span::raw("Help  "),
        Span::styled(" [c] ", Style::default().fg(Color::Black).bg(Color::Magenta).bold()),
        Span::raw("Clear History"),
    ]);
    let footer = Paragraph::new(footer_text).alignment(Alignment::Center);
    frame.render_widget(footer, layout[2]);

    // 5. Help Modal if toggled
    if state.show_help {
        let modal_area = centered_rect(60, 50, area);
        frame.render_widget(Clear, modal_area);
        let help_text = vec![
            Line::from(Span::styled("Twatch Keyboard Shortcuts", Style::default().bold().fg(Color::Cyan))),
            Line::from(""),
            Line::from("  q / Esc      Quit the session or live monitor"),
            Line::from("  p / Space    Pause / resume temperature polling"),
            Line::from("  f            Toggle squashed vs full NVMe/device channels"),
            Line::from("  c            Clear real-time chart history buffer"),
            Line::from("  h / ?        Toggle this help dialog"),
            Line::from(""),
            Line::from(Span::styled("Color Legend:", Style::default().bold())),
            Line::from(vec![
                Span::styled("  < 40°C  ", Style::default().fg(Color::Cyan)),
                Span::raw("Cool / Idle"),
            ]),
            Line::from(vec![
                Span::styled("  40-55°C ", Style::default().fg(Color::Green)),
                Span::raw("Normal operating temperature"),
            ]),
            Line::from(vec![
                Span::styled("  55-70°C ", Style::default().fg(Color::Yellow)),
                Span::raw("Moderate workload"),
            ]),
            Line::from(vec![
                Span::styled("  70-85°C ", Style::default().fg(Color::Rgb(255, 140, 0))),
                Span::raw("High heat / load"),
            ]),
            Line::from(vec![
                Span::styled("  > 85°C  ", Style::default().fg(Color::Rgb(255, 50, 70)).bold()),
                Span::raw("Thermal throttling / critical danger!"),
            ]),
            Line::from(""),
            Line::from("Press [h], [?], or [Esc] to close."),
        ];
        let modal = Paragraph::new(help_text).block(
            Block::default()
                .title(" Help ")
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(Color::Cyan)),
        );
        frame.render_widget(modal, modal_area);
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

/// Run an interactive recording session or live dashboard
pub fn run_session(
    config: &Config,
    by_temperature: bool,
    capture_limit: u16,
    initial_temp: f64,
    end_temp: f64,
    sensor_kind: &str,
    json_output: bool,
    record: bool,
) -> io::Result<()> {
    let ms_delay = config.delay;
    let target_duration = Duration::from_millis(ms_delay);

    if json_output {
        return run_json_mode(config, by_temperature, capture_limit, initial_temp, end_temp, sensor_kind);
    }

    // Terminal initialization
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.hide_cursor()?;

    // Set up session writer if recording
    let mut session = if record {
        Some(session_writer(ms_delay)?)
    } else {
        None
    };

    let session_id = session.as_ref().map(|s| s.id);
    let mut state = LiveState::new(config.max_plot_temp as f64, config.full_devices_sensors);
    let registry = SensorRegistry::scan();

    let mut elapsed_frames = 0u16;
    let total_start = Instant::now();

    let run_res = (|| -> io::Result<bool> {
        loop {
            let frame_start = Instant::now();
            let elapsed_sec = total_start.elapsed().as_secs_f64();

            // Poll sensor readings using cached descriptors
            let sensors = registry.read_all();

            // Record to file if active
            if let Some(ref mut s) = session {
                record_frame(s, &sensors)?;
            }

            state.update(&sensors, elapsed_sec);
            let target = target_temp(&sensors, sensor_kind);

            // Status details
            let (mode_label, status_info) = if !record {
                ("Live Monitor", format!("Target: [{}] {:.1}°C", sensor_kind.to_uppercase(), target))
            } else if by_temperature {
                (
                    "Temp Trigger",
                    format!(
                        "Target: [{}] {:.1}°C | Stop: >= {:.1}°C",
                        sensor_kind.to_uppercase(),
                        target,
                        end_temp
                    ),
                )
            } else {
                (
                    "Capture Limit",
                    format!("Frames: {}/{} | Target: {:.1}°C", elapsed_frames, capture_limit, target),
                )
            };

            // Draw btop frame
            terminal.draw(|f| {
                draw_btop_frame(
                    f,
                    &sensors,
                    &state,
                    session_id,
                    mode_label,
                    &status_info,
                    ms_delay,
                )
            })?;

            // Event polling (poll with remaining interval)
            let frame_time = frame_start.elapsed();
            let remaining = target_duration.saturating_sub(frame_time);

            if event::poll(remaining)? {
                if let Event::Key(key) = event::read()? {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => {
                            if state.show_help {
                                state.show_help = false;
                            } else {
                                if let Some(ref mut s) = session {
                                    let _ = flush_buffer(s);
                                }
                                return Ok(false);
                            }
                        }
                        KeyCode::Char('p') | KeyCode::Char(' ') => {
                            state.paused = !state.paused;
                        }
                        KeyCode::Char('f') | KeyCode::Char('F') => {
                            state.full_devices_sensors = !state.full_devices_sensors;
                        }
                        KeyCode::Char('h') | KeyCode::Char('?') => {
                            state.show_help = !state.show_help;
                        }
                        KeyCode::Char('c') => {
                            state.histories.clear();
                        }
                        _ => {}
                    }
                }
            }

            // Check triggers if recording
            if record && by_temperature && target >= end_temp {
                if let Some(ref mut s) = session {
                    flush_buffer(s)?;
                    writeln!(s.file, "# Total:{:.2}", total_start.elapsed().as_secs_f64())?;
                    writeln!(s.file, "# Peak:{:.1}", s.peak_temp)?;
                    writeln!(s.file, "{},Exit,{:.1}", sensor_kind.to_uppercase(), target)?;
                }
                return Ok(true);
            }

            if record && !by_temperature {
                elapsed_frames += 1;
                if elapsed_frames >= capture_limit {
                    if let Some(ref mut s) = session {
                        flush_buffer(s)?;
                        writeln!(s.file, "# Total:{:.2}", total_start.elapsed().as_secs_f64())?;
                        writeln!(s.file, "# Peak:{:.1}", s.peak_temp)?;
                        writeln!(s.file, "{},Exit,{:.1}", sensor_kind.to_uppercase(), target)?;
                    }
                    return Ok(true);
                }
            }
        }
    })();

    // Always clean up terminal state
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor().ok();

    let completed = run_res?;

    // If session completed normally and graph is requested, open native graph viewer
    if record && !config.no_graph && completed {
        if let Some(sid) = session_id {
            run_graph_viewer(&[sid], config.max_plot_temp, config.temp_steps, config.full_devices_sensors)?;
        }
    }

    Ok(())
}

fn run_json_mode(
    config: &Config,
    by_temperature: bool,
    capture_limit: u16,
    _initial_temp: f64,
    end_temp: f64,
    sensor_kind: &str,
) -> io::Result<()> {
    let delay = Duration::from_millis(config.delay);
    let registry = SensorRegistry::scan();
    let total_start = Instant::now();
    let mut frame_count = 0u16;

    let term_signal = Arc::new(AtomicBool::new(false));
    let r = term_signal.clone();
    let _ = ctrlc_handler(move || {
        r.store(true, Ordering::SeqCst);
    });

    while !term_signal.load(Ordering::SeqCst) {
        let loop_start = Instant::now();
        let elapsed = total_start.elapsed().as_secs_f64();
        let sensors = registry.read_all();
        let target = target_temp(&sensors, sensor_kind);
        let display_sensors = squash_sensors(&sensors, config.full_devices_sensors);

        println!("{}", format_json_frame(&display_sensors, elapsed));

        if by_temperature && target >= end_temp {
            break;
        }
        if !by_temperature {
            frame_count += 1;
            if frame_count >= capture_limit {
                break;
            }
        }

        let work_time = loop_start.elapsed();
        if work_time < delay {
            std::thread::sleep(delay - work_time);
        }
    }

    Ok(())
}

fn ctrlc_handler<F>(_f: F) -> io::Result<()>
where
    F: Fn() + Send + 'static,
{
    // Minimal standard signal helper without extra crates
    Ok(())
}
