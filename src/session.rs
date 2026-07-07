use crate::{
    plot::{plot_maker, ScalingPlot},
    sensors::{device_type, search_sensors, SensorLabel},
    Config,
};
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Gauge, Paragraph},
};
use std::{
    fs::{self, File},
    io::{self, Write},
    path::PathBuf,
    time::Instant,
};

pub struct SessionFile {
    pub id: u16,
    pub file: File,
    pub buffer: Vec<String>,
    pub flush_interval: usize,
}

pub fn list_sessions() -> io::Result<Vec<(u16, PathBuf)>> {
    let home = PathBuf::from(std::env::var("HOME").unwrap_or_default());
    let session_dir = home.join("Documents").join("Twatch").join("session");

    if !session_dir.exists() {
        return Ok(Vec::new());
    }

    let mut sessions = Vec::new();
    let entries = fs::read_dir(&session_dir)?;

    for entry in entries.filter_map(|r| r.ok()) {
        let path = entry.path();
        if path.extension().map_or(false, |ext| ext == "csv") {
            if let Some(name) = path.file_stem() {
                let name = name.to_string_lossy();
                if let Some(num) = name.strip_prefix("session_") {
                    if let Ok(id) = num.parse::<u16>() {
                        sessions.push((id, path));
                    }
                }
            }
        }
    }

    sessions.sort_by_key(|(id, _)| *id);
    Ok(sessions)
}

fn session_writer(delay: u64) -> io::Result<SessionFile> {
    let home = PathBuf::from(std::env::var("HOME").unwrap_or_default());
    let session_dir = home.join("Documents").join("Twatch").join("session");
    fs::create_dir_all(&session_dir)?;

    let mut session_id: u16 = 0;
    loop {
        let candidate = session_dir.join(format!("session_{}.csv", session_id));
        if !candidate.exists() {
            let mut file = File::create(&candidate)?;
            writeln!(file, "# Delay:{}", delay)?;
            writeln!(file, "Type,Label,Temp")?;
            return Ok(SessionFile {
                id: session_id,
                file,
                buffer: Vec::with_capacity(50),
                flush_interval: 50,
            });
        }
        session_id += 1;
    }
}

fn flush_buffer(session: &mut SessionFile) -> io::Result<()> {
    for line in &session.buffer {
        writeln!(session.file, "{}", line)?;
    }
    session.file.flush()?;
    session.buffer.clear();
    Ok(())
}

fn record_frame(session: &mut SessionFile, sensors: &[SensorLabel]) -> io::Result<()> {
    for sensor in sensors {
        let d_type = device_type(sensor);
        if d_type == "Unknown" {
            continue;
        }
        session
            .buffer
            .push(format!("{},{},{}", d_type, sensor.label, sensor.temp));
    }
    if session.buffer.len() >= session.flush_interval {
        flush_buffer(session)?;
    }
    Ok(())
}

fn format_json_frame(sensors: &[SensorLabel], elapsed: u16) -> String {
    let mut parts = vec![format!("\"elapsed\":{}", elapsed)];
    for s in sensors {
        let d_type = device_type(s);
        if d_type == "Unknown" {
            continue;
        }
        let key = format!("{}_{}", d_type.to_lowercase(), s.label.to_lowercase());
        parts.push(format!("\"{}\":{}", key, s.temp));
    }
    format!("{{{}}}", parts.join(", "))
}

fn target_temp(sensors: &[SensorLabel], sensor_kind: &str) -> u32 {
    match sensor_kind {
        "gpu" => sensors.iter().find(|s| s.is_amd_gpu).map(|s| s.temp).unwrap_or(0),
        "nvme" => sensors.iter().find(|s| s.is_nvme).map(|s| s.temp).unwrap_or(0),
        _ => sensors.iter().find(|s| s.is_cpu).map(|s| s.temp).unwrap_or(0),
    }
}

fn draw_live_frame(frame: &mut Frame, sensors: &[SensorLabel], status: &str, subtitle: &str) {
    let area = frame.area();

    let header = Paragraph::new(status)
        .block(Block::default().borders(Borders::ALL).title(" Twatch "))
        .style(Style::default().fg(Color::Cyan));

    let header_height = 3;
    let footer_height = 1;

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(header_height),
            Constraint::Min(0),
            Constraint::Length(footer_height),
        ])
        .split(area);

    frame.render_widget(header, layout[0]);

    let sensor_count = sensors
        .iter()
        .filter(|s| device_type(s) != "Unknown")
        .count();
    if sensor_count == 0 {
        let msg =
            Paragraph::new("No sensors found").block(Block::default().borders(Borders::ALL));
        frame.render_widget(msg, layout[1]);
        return;
    }

    let sensor_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            (0..sensor_count)
                .map(|_| Constraint::Length(3))
                .collect::<Vec<_>>(),
        )
        .split(layout[1]);

    let max_temp: u32 = sensors.iter().map(|s| s.temp).max().unwrap_or(100).max(100);

    let mut row = 0;
    for sensor in sensors {
        let d_type = device_type(sensor);
        if d_type == "Unknown" || row >= sensor_layout.len() {
            continue;
        }

        let ratio = (sensor.temp as f64 / max_temp as f64).min(1.0);
        let color = if sensor.temp >= 70 {
            Color::Red
        } else if sensor.temp >= 50 {
            Color::Yellow
        } else {
            Color::Green
        };

        let label = format!("[{}] {}", d_type, sensor.label);
        let gauge = Gauge::default()
            .block(Block::default().borders(Borders::NONE))
            .gauge_style(Style::default().fg(color))
            .ratio(ratio)
            .label(label);

        frame.render_widget(gauge, sensor_layout[row]);

        let value = format!("{}°C", sensor.temp);
        let value_area = Rect {
            x: sensor_layout[row]
                .x
                .saturating_add(sensor_layout[row].width.saturating_sub(10)),
            y: sensor_layout[row].y.saturating_add(1),
            width: 8,
            height: 1,
        };
        frame.render_widget(
            Paragraph::new(value).style(Style::default().fg(color).add_modifier(Modifier::BOLD)),
            value_area,
        );

        row += 1;
    }

    let footer = Paragraph::new(subtitle)
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    frame.render_widget(footer, layout[2]);
}

pub fn run_session(
    config: &Config,
    by_temperature: bool,
    capture_limit: u16,
    initial_temp: u32,
    end_temp: u32,
    sensor_kind: &str,
    json_output: bool,
) -> io::Result<()> {
    let ms_delay = config.delay;

    if !json_output {
        enable_raw_mode()?;
    }
    let mut stdout = io::stdout();
    if !json_output {
        execute!(stdout, EnterAlternateScreen)?;
    }

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut session = session_writer(ms_delay)?;
    let session_id = session.id;
    let mut elapsed = 0u16;
    let total_start = Instant::now();

    let result = (|| -> io::Result<bool> {
        loop {
            let sensors = search_sensors()?;
            record_frame(&mut session, &sensors)?;
            let target = target_temp(&sensors, sensor_kind);

            if json_output {
                println!("{}", format_json_frame(&sensors, elapsed));
            } else {
                let sensor_label = match sensor_kind {
                    "gpu" => "GPU",
                    "nvme" => "NVMe",
                    _ => "CPU",
                };

                let status = if by_temperature {
                    format!(
                        "Temp Trigger [{}]  |  T: {}°C  |  Range: [{}, {}]°C",
                        sensor_label, target, initial_temp, end_temp
                    )
                } else {
                    format!(
                        "Capture Limit  |  {}/{}  |  T: {}°C",
                        elapsed, capture_limit, target
                    )
                };

                let subtitle = format!("Delay: {}ms  |  Session {}  |  q=quit", ms_delay, session_id);

                terminal
                    .draw(|f| draw_live_frame(f, &sensors, &status, &subtitle))
                    .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
            }

            if !json_output {
                if event::poll(std::time::Duration::from_millis(ms_delay / 4))
                    .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?
                {
                    if let Event::Key(key) =
                        event::read().map_err(|e| io::Error::new(io::ErrorKind::Other, e))?
                    {
                        if key.code == KeyCode::Char('q') || key.code == KeyCode::Esc {
                            let _ = flush_buffer(&mut session);
                            return Ok(false);
                        }
                    }
                }
            }

            std::thread::sleep(std::time::Duration::from_millis(ms_delay * 3 / 4));

            if by_temperature && target >= end_temp {
                flush_buffer(&mut session)?;
                writeln!(session.file, "#Total: {:.3}", total_start.elapsed().as_secs())?;
                writeln!(session.file, "CPU,Exit,{}", target)?;
                return Ok(true);
            }

            if !by_temperature {
                elapsed += 1;
                if elapsed >= capture_limit {
                    flush_buffer(&mut session)?;
                    writeln!(session.file, "#Total: {:.3}", total_start.elapsed().as_secs())?;
                    writeln!(session.file, "CPU,Exit,{}", target)?;
                    return Ok(true);
                }
            }
        }
    })();

    if !json_output {
        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        terminal.show_cursor().ok();
    }

    let completed = result?;

    if !config.no_graph && completed {
        let scale = ScalingPlot {
            max_plot_temperature: config.max_plot_temp,
            number_of_steps_for_graph: config.temp_steps,
        };
        plot_maker(&[session_id], scale);
    }

    Ok(())
}
