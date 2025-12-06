use crate::{
    cli::ArgumentPassers,
    plot::{plot_maker, ScalingPlot},
    sensors::{device_type, search_sensors, SensorLabel},
};
use std::{
    fs::{self, File},
    io::{self, Write},
    path::Path,
    time::Instant,
};

pub struct SessionFile {
    pub id: u32,
    pub file: File,
    pub buffer: Vec<String>,
    pub flush_interval: usize,
}

pub fn record_frame(
    session: &mut SessionFile,
    _countdown: u64,
    header_msg: &str,
) -> std::io::Result<Vec<SensorLabel>> {
    let sensors = search_sensors()?;
    let mut display_tui: String = Default::default();

    display_tui.push_str(header_msg);

    for sensor in &sensors {
        let d_type = device_type(sensor);

        if d_type == "Unknown" {
            continue;
        }

        println!();
        display_tui.push_str(&format!(
            "\n [{}] {}: {}°C",
            d_type, sensor.label, sensor.temp
        ));

        session
            .buffer
            .push(format!("{},{},{}", d_type, sensor.label, sensor.temp));
    }
    if session.buffer.len() >= session.flush_interval {
        for line in &session.buffer {
            writeln!(session.file, "{}", line)?;
        }
        session.file.flush()?;
        session.buffer.clear();
    }

    display_tui.push_str("\x1B[J");
    println!("{}", display_tui);

    Ok(sensors)
}

pub fn session_writter(passers: &ArgumentPassers) -> std::io::Result<SessionFile> {
    let mut session_id = 0;

    loop {
        let condidate = format!("session/session_{}.csv", session_id);
        if !Path::new(&condidate).exists() {
            if let Some(parent) = Path::new(&condidate).parent() {
                std::fs::create_dir_all(parent)?;
            }

            let mut file = File::create(&condidate)?;
            writeln!(file, "# Delay:{}", passers.ms_delay)?;
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

pub fn trigger_by_temperature(passers: &ArgumentPassers) -> std::io::Result<()> {
    print!("\r\x1B[2J\x1B[1;1H");
    let mut session = session_writter(passers)?;
    let mut countdown = 0;

    let total_start = Instant::now();
    loop {
        print!("\r\x1B[2J\x1B[1;1H");

        countdown += 1;

        let status_header = format!(
            "--- Trigger Monitor ---\nRange: [Start: {}°C, End: {}°C]",
            passers.initial_temperature, passers.end_temperature
        );

        let sensors = record_frame(&mut session, countdown, &status_header)?;

        let cpu_temp = sensors
            .iter()
            .find(|s| s.is_cpu)
            .map(|s| s.temp)
            .expect("Unable to read CPU temperature");

        println!("\nStatus:");

        if cpu_temp >= passers.initial_temperature {
            println!(
                "\x1B[33m Trigger Active: {}°C >= {}°C\x1B[0m",
                cpu_temp, passers.initial_temperature
            );
        } else {
            println!("Below Target");
        }

        if cpu_temp > passers.end_temperature {
            println!("\x1B[31m Limit reached ({}°C).\x1B[0m", cpu_temp);
            writeln!(
                session.file,
                "#Total: {:.3}",
                total_start.elapsed().as_secs()
            )?;
            writeln!(session.file, "CPU,Exit,{}", cpu_temp)?;
            plot_maker(ScalingPlot {
                max_plot_temperature: 110,
                number_of_steps_for_graph: 5,
            });
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(passers.ms_delay));
    }
    Ok(())
}

pub fn session_selector(arg_passers: &mut ArgumentPassers) -> io::Result<()> {
    let entries = fs::read_dir(".")?
        .filter_map(|res| res.ok())
        .map(|e| e.path())
        .collect::<Vec<_>>();

    let found_session = entries
        .iter()
        .any(|p| p.file_name() == Some("session".as_ref()) && p.is_dir());

    if found_session {
        arg_passers.session_exists = found_session;
    } else {
        println!("Session folder not found in current PATH");
    }

    if arg_passers.see_sessions && found_session {
        let session_files = fs::read_dir("./session")?
            .filter_map(|res| res.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().map_or(false, |ext| ext == "csv"));
        for path in session_files {
            println!("{}", path.display());
        }
    }

    Ok(())
}

pub fn by_capture_limit(passers: &ArgumentPassers) -> std::io::Result<()> {
    let mut session = session_writter(passers)?;
    let mut countdown = 0;

    let total_start = Instant::now();
    loop {
        print!("\r\x1B[2J\x1B[1;1H");

        countdown += 1;

        let status_header = format!(
            "--- Trigger Monitor ---\nCurrent: [{}] Target: [{}]\n",
            countdown, passers.amount_captures
        );

        let sensors = record_frame(&mut session, countdown, &status_header)?;

        let cpu_temp = sensors
            .iter()
            .find(|s| s.is_cpu)
            .map(|s| s.temp)
            .expect("Unable to read CPU Temperatures");

        if countdown >= passers.amount_captures {
            println!(
                "\n\x1B[31mTarget reached: [{}]\x1B[0m",
                passers.amount_captures
            );
            writeln!(
                session.file,
                "#Total: {:.3}",
                total_start.elapsed().as_secs()
            )?;
            writeln!(session.file, "CPU,Exit,{}", cpu_temp)?;
            plot_maker(ScalingPlot {
                max_plot_temperature: 110,
                number_of_steps_for_graph: 5,
            });
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(passers.ms_delay));
    }
    Ok(())
}
