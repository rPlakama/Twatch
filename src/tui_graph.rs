use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    prelude::*,
    symbols,
    widgets::{Block, Borders},
};
use std::{
    collections::HashMap,
    fs,
    io,
    path::PathBuf,
};

#[derive(Clone)]
struct SensorData {
    type_: String,
    temps: Vec<f64>,
}

fn parse_session_data(csv_content: &str) -> HashMap<String, SensorData> {
    let mut series = HashMap::new();
    for line in csv_content.lines() {
        if line.starts_with('#') || line.starts_with("Type,") {
            continue;
        }
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() >= 3 {
            let type_ = parts[0].to_string();
            let label = parts[1].to_string();
            if let Ok(temp) = parts[2].parse::<f64>() {
                let entry = series.entry(label).or_insert_with(|| SensorData {
                    type_,
                    temps: Vec::new(),
                });
                entry.temps.push(temp);
            }
        }
    }
    series
}

fn read_session_csv(session_id: Option<u16>) -> io::Result<(String, u16)> {
    let home = PathBuf::from(std::env::var("HOME").unwrap_or_default());
    let session_dir = home.join("Documents").join("Twatch").join("session");
    let actual_id = match session_id {
        Some(id) => id,
        None => {
            let mut paths: Vec<_> = fs::read_dir(&session_dir)?
                .filter_map(|r| r.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().map_or(false, |ext| ext == "csv"))
                .collect();
            paths.sort();
            let latest = paths
                .last()
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "No sessions found"))?;
            let name = latest.file_stem().unwrap().to_string_lossy();
            name.strip_prefix("session_")
                .and_then(|n| n.parse().ok())
                .unwrap_or(0)
        }
    };
    let path = session_dir.join(format!("session_{}.csv", actual_id));
    let content = fs::read_to_string(&path)?;
    Ok((content, actual_id))
}

pub fn show_graph(session_id: Option<u16>) -> io::Result<()> {
    let (csv_content, id) = read_session_csv(session_id)?;
    let data = parse_session_data(&csv_content);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut offset: usize = 0;
    let mut zoom: f64 = 1.0;

    let colors = [
        Color::Red,
        Color::Green,
        Color::Blue,
        Color::Yellow,
        Color::Magenta,
        Color::Cyan,
    ];

    let num_samples = data
        .values()
        .map(|d| d.temps.len())
        .max()
        .unwrap_or(1)
        .max(1);

    let result = (|| -> io::Result<()> {
        loop {
            terminal
                .draw(|f| {
                    draw_tui_graph(f, &data, &colors, num_samples, offset, zoom, id)
                })
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

            if event::poll(std::time::Duration::from_millis(100))
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?
            {
                if let Event::Key(key) =
                    event::read().map_err(|e| io::Error::new(io::ErrorKind::Other, e))?
                {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => break,
                        KeyCode::Char('+') | KeyCode::Char('=') => zoom /= 1.2,
                        KeyCode::Char('-') => zoom *= 1.2,
                        KeyCode::Left | KeyCode::Char('h') => {
                            offset = offset.saturating_sub((num_samples as f64 * 0.05) as usize)
                        }
                        KeyCode::Right | KeyCode::Char('l') => {
                            offset = (offset + (num_samples as f64 * 0.05) as usize)
                                .min(num_samples)
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
    result
}

fn draw_tui_graph(
    f: &mut Frame,
    data: &HashMap<String, SensorData>,
    colors: &[Color],
    num_samples: usize,
    offset: usize,
    zoom: f64,
    session_id: u16,
) {
    let area = f.area();

    let title_block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" Twatch Graph - Session {} ", session_id));
    let inner = title_block.inner(area);
    f.render_widget(title_block, area);

    if inner.height < 6 || inner.width < 10 {
        return;
    }

    let plot_height = (inner.height as usize).saturating_sub(2);
    let plot_width = (inner.width as usize).saturating_sub(8);

    let visible_samples = ((num_samples as f64) / zoom) as usize;
    let end = (offset + visible_samples).min(num_samples).max(offset + 1);
    let range = end - offset;

    let global_min: f64 = data
        .values()
        .flat_map(|d| d.temps.iter())
        .cloned()
        .fold(f64::INFINITY, f64::min);
    let global_max: f64 = data
        .values()
        .flat_map(|d| d.temps.iter())
        .cloned()
        .fold(f64::NEG_INFINITY, f64::max);

    let y_range = (global_max - global_min).max(5.0);
    let y_min = (global_min - y_range * 0.1).max(0.0);
    let y_max = global_max + y_range * 0.1;

    let buf = f.buffer_mut();

    for i in 0..=4 {
        let y_val = y_max - (i as f64 / 4.0) * (y_max - y_min);
        let y = inner.y + 1 + ((i as u16 * plot_height as u16) / 4);
        if y < inner.y + inner.height {
            for x in inner.x..inner.x + inner.width {
                if let Some(cell) = buf.cell_mut(Position { x, y }) {
                    if x < inner.x + 6 && x >= inner.x {
                        cell.set_symbol("─");
                        cell.set_fg(Color::DarkGray);
                    }
                }
            }
            let label = format!("{:5.0}°C", y_val);
            for (ci, ch) in label.chars().enumerate() {
                let cx = inner.x + ci as u16;
                if cx < inner.x + inner.width {
                    if let Some(cell) = buf.cell_mut(Position { x: cx, y }) {
                        cell.set_symbol(&ch.to_string());
                        cell.set_fg(Color::DarkGray);
                    }
                }
            }
        }
    }

    let x_start = inner.x + 7;
    let mut color_iter = colors.iter().cycle();

    for (_label, sensor) in data {
        if sensor.temps.is_empty() {
            continue;
        }
        let color = *color_iter.next().unwrap_or(&Color::White);
        let slice = &sensor.temps[offset..end];

        for (i, &temp) in slice.iter().enumerate() {
            let col = x_start + if range > 0 {
                ((i as f64 / range as f64) * (plot_width as f64)) as u16
            } else {
                0
            };
            let row = inner.y + inner.height - 2
                - ((temp - y_min) / (y_max - y_min) * (plot_height as f64)) as u16;

            if col < inner.x + inner.width && row < inner.y + inner.height && row >= inner.y + 1 {
                let glyph = if i > 0 {
                    let prev_temp = slice[i - 1];
                    let prev_row = inner.y + inner.height - 2
                        - ((prev_temp - y_min) / (y_max - y_min) * (plot_height as f64))
                            as u16;
                    if row < prev_row {
                        symbols::line::TOP_RIGHT
                    } else if row > prev_row {
                        symbols::line::BOTTOM_RIGHT
                    } else {
                        symbols::line::HORIZONTAL
                    }
                } else {
                    "●"
                };

                if let Some(cell) = buf.cell_mut(Position {
                    x: col,
                    y: row,
                }) {
                    cell.set_symbol(glyph);
                    cell.set_fg(color);
                }

                if i > 0 {
                    let prev_temp = slice[i - 1];
                    let prev_col = x_start
                        + if range > 0 {
                            (((i - 1) as f64 / range as f64) * (plot_width as f64)) as u16
                        } else {
                            0
                        };
                    let prev_row = inner.y + inner.height - 2
                        - ((prev_temp - y_min) / (y_max - y_min) * (plot_height as f64))
                            as u16;
                    fill_line(buf, prev_col, prev_row, col, row, color, inner);
                }
            }
        }
    }

    let legend: Vec<String> = data
        .iter()
        .map(|(l, s)| format!("{}.{}", s.type_, l))
        .collect();
    let legend_text = legend.join("  |  ");

    let legend_y = inner.y + inner.height.saturating_sub(1);
    if legend_y < area.y + area.height {
        for (i, ch) in legend_text.chars().enumerate() {
            let cx = inner.x + 7 + i as u16;
            if cx < inner.x + inner.width {
                if let Some(cell) = buf.cell_mut(Position { x: cx, y: legend_y }) {
                    cell.set_symbol(&ch.to_string());
                    cell.set_fg(Color::DarkGray);
                }
            }
        }
    }

    let nav = format!(
        "←→ pan  +/- zoom  q quit  |  samples {}-{} of {}  zoom {:.1}x",
        offset,
        end.min(num_samples),
        num_samples,
        zoom
    );
    let nav_y = area.y + area.height.saturating_sub(1);
    if nav_y < area.y + area.height {
        for (i, ch) in nav.chars().enumerate() {
            let cx = area.x + 2 + i as u16;
            if cx < area.x + area.width {
                if let Some(cell) = buf.cell_mut(Position { x: cx, y: nav_y }) {
                    cell.set_symbol(&ch.to_string());
                    cell.set_fg(Color::DarkGray);
                }
            }
        }
    }
}

fn fill_line(
    buf: &mut Buffer,
    x1: u16,
    y1: u16,
    x2: u16,
    y2: u16,
    color: Color,
    bounds: Rect,
) {
    let dx = (x2 as i32 - x1 as i32).abs();
    let dy = (y2 as i32 - y1 as i32).abs();
    let sx: i32 = if x1 < x2 { 1 } else { -1 };
    let sy: i32 = if y1 < y2 { 1 } else { -1 };
    let mut err = if dx > dy { dx / 2 } else { -dy / 2 };
    let mut x = x1 as i32;
    let mut y = y1 as i32;

    loop {
        if x >= bounds.x as i32
            && x < (bounds.x + bounds.width) as i32
            && y >= bounds.y as i32 + 1
            && y < (bounds.y + bounds.height) as i32 - 1
        {
            if let Some(cell) = buf.cell_mut(Position {
                x: x as u16,
                y: y as u16,
            }) {
                cell.set_symbol("●");
                cell.set_fg(color);
            }
        }

        if x == x2 as i32 && y == y2 as i32 {
            break;
        }

        let e2 = err;
        if e2 > -dx {
            err -= dy;
            x += sx;
        }
        if e2 < dy {
            err += dx;
            y += sy;
        }
    }
}
