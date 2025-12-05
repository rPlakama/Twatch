use gtk::prelude::*;
use gtk::{Application, ApplicationWindow, DrawingArea};
use gtk4 as gtk;
use std::f64::consts::PI;

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
pub struct SensorLabel {
    pub label: String,
    pub is_cpu: bool,
    pub is_amd_gpu: bool,
    pub is_nvme: bool,
    pub temp: u32,
}

pub struct ArgumentPassers {
    pub is_by_temperature: bool,
    pub is_by_capture: bool,
    pub plot_latest: bool,
    pub ms_delay: u64,
    pub amount_captures: u64,
    pub initial_temperature: u32,
    pub end_temperature: u32,
    pub session_exists: bool,
}

pub struct SessionType {
    pub is_power: bool,
    pub is_temperature: bool,
    pub is_generic: bool,
}

fn record_frame(
    session: &mut SessionFile,
    _countdown: u64,
    header_msg: &str,
) -> std::io::Result<Vec<SensorLabel>> {
    let sensors = search_sensors()?;
    let mut display_tui: String = Default::default();

    display_tui.push_str(&format!("{}", header_msg));

    for sensor in &sensors {
        let d_type = device_type(sensor);

        if d_type == "Unknown" {
            continue;
        }
        // Can be implemented a posix flag

        println!("");
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

fn device_type(sensor: &SensorLabel) -> &'static str {
    if sensor.is_cpu {
        "CPU"
    } else if sensor.is_nvme {
        "NVME"
    } else if sensor.is_amd_gpu {
        "GPU"
    } else {
        "Unknown"
    }
}

fn args_processor(session_type: &SessionType, passers: &ArgumentPassers) {
    if session_type.is_power && !session_type.is_temperature {
        if let Err(e) = power_usage() {
            eprintln!("Error during power monitoring: {}", e);
        }
    } else if session_type.is_temperature
        && passers.is_by_temperature
        && !session_type.is_power
        && !passers.is_by_capture
    {
        if let Err(e) = trigger_by_temperature(passers) {
            eprintln!("Error during temperature monitoring: {}", e);
        }
    } else if !passers.is_by_temperature && passers.is_by_capture {
        if let Err(e) = by_capture_limit(passers) {
            eprintln!("Error during the monitoring: {}", e);
        }
    } else if passers.plot_latest {
        plot_maker();
    } else if !passers.session_exists {
        println!("Unable to find session file, do a capture first.");
    }
}
fn main() {
    let mut args = std::env::args().skip(1);

    let mut session_type = SessionType {
        is_power: false,
        is_temperature: false,
        is_generic: false,
    };
    let mut arg_passers = ArgumentPassers {
        is_by_temperature: false,
        is_by_capture: false,
        ms_delay: 250,
        amount_captures: 250,
        plot_latest: false,
        end_temperature: 70,
        initial_temperature: 40,
        session_exists: false,
    };

    let mut _help_called = false;
    let sensors = search_sensors().expect("Unable to receive sensors information");

    let cpu_temp = sensors
        .iter()
        .find(|s| s.is_cpu)
        .map(|s| s.temp)
        .expect("Unable to read cpu temperature");
    while let Some(arg) = args.next() {
        match &arg[..] {
            "-bt" | "--by-temperature" => {
                session_type.is_temperature = true;
                arg_passers.is_by_temperature = true;
            }
            "-h" | "--help" => {
                _help_called = true;
                help();
            }
            "-pl" | "--plot-latest" => {
                arg_passers.plot_latest = true;
            }
            "-d" | "--delay" => {
                if let Some(val_str) = args.next() {
                    arg_passers.ms_delay = val_str.parse().unwrap_or(250);
                }
            }
            "-c" | "--captures" => {
                if let Some(val_str) = args.next() {
                    arg_passers.amount_captures = val_str.parse().unwrap_or(500);
                }
            }
            "-it" | "--initial-temperature" => {
                if let Some(val_str) = args.next() {
                    arg_passers.initial_temperature = val_str.parse().unwrap_or(45)
                }
            }
            "-et" | "--end-temperature" => {
                if let Some(val_str) = args.next() {
                    arg_passers.end_temperature = val_str.parse().unwrap_or(70)
                }
            }
            "-ct" | "--current-temperature" => {
                println!("CPU TEMP: {}C", cpu_temp);
            }
            "-bl" | "--by-capture-limit" => {
                arg_passers.is_by_capture = true;
            }
            "-bw" | "--by-watts" => {
                session_type.is_power = true;
            }
            "-wl" | "--window-test" => {
                plot_maker();
                session_type.is_generic = true;
            }
            _ => {
                println!("Argument invalid or not found {}", arg)
            }
        }

        // if !session_type.is_power
        //     && !session_type.is_generic
        //     && !arg_passers.plot_latest
        //     && !arg_passers.is_by_temperature
        //     && !arg_passers.is_by_capture
        //     && !help_called
        // {
        //     eprintln!(
        //         "You must provide one of the key arguments: \n
        //         --plot_latest\n
        //         --by-watts \n
        //         --by-capture-limit\n
        //         --by-temperature\n"
        //     );
        //     std::process::exit(1);
        // }
    }
    let _ = session_selector(&mut arg_passers);
    args_processor(&session_type, &arg_passers);
}

fn search_sensors() -> std::io::Result<Vec<SensorLabel>> {
    let mut collected_data: Vec<SensorLabel> = Vec::new();

    let hwmon_paths = fs::read_dir("/sys/class/hwmon/")
        .expect("Could not read the sys/class/hwmon directory")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_dir()
                && path
                    .file_name()
                    .map_or(false, |name| name.to_string_lossy().starts_with("hwmon"))
        });

    for path in hwmon_paths {
        let device_name = fs::read_to_string(path.join("name"))
            .expect("Unable to read_to_string in hwmon path")
            .trim()
            .to_string();
        let is_nvme = device_name.contains("nvme");
        let is_cpu = device_name.contains("coretemp") || device_name.contains("k10temp");
        let is_amd_gpu = device_name.contains("amdgpu");

        if let Ok(entries) = fs::read_dir(&path) {
            for entry in entries.filter_map(Result::ok) {
                let file_name = entry.file_name().to_string_lossy().to_string();
                if file_name.starts_with("temp") && file_name.ends_with("_input") {
                    let temp_string =
                        fs::read_to_string(entry.path()).expect("Unable to entry.path");
                    let temp_value: u32 = temp_string.trim().parse().unwrap_or(0) / 1000;
                    let label_path = entry
                        .path()
                        .with_file_name(file_name.replace("_input", "_label"));
                    let label_string = fs::read_to_string(label_path)
                        .unwrap_or("Unknown".to_string())
                        .trim()
                        .to_string();
                    collected_data.push(SensorLabel {
                        label: label_string,
                        is_cpu: is_cpu,
                        is_nvme: is_nvme,
                        is_amd_gpu: is_amd_gpu,
                        temp: temp_value,
                    });
                }
            }
        }
    }

    Ok(collected_data)
}
fn session_writter(passers: &ArgumentPassers) -> std::io::Result<SessionFile> {
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
                file: file,
                buffer: Vec::with_capacity(50),
                flush_interval: 50,
            });
        }
        session_id += 1;
    }
}

fn trigger_by_temperature(passers: &ArgumentPassers) -> std::io::Result<()> {
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
            println!("Bellow Target");
        }

        if cpu_temp > passers.end_temperature {
            println!("\x1B[31m Limit reached ({}°C).\x1B[0m", cpu_temp);
            writeln!(
                session.file,
                "#Total: {:.3}",
                total_start.elapsed().as_secs()
            )?;
            writeln!(session.file, "CPU,Exit,{}", cpu_temp)?;
            plot_maker();
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(passers.ms_delay));
    }
    Ok(())
}
fn session_selector(arg_passers: &mut ArgumentPassers) -> io::Result<()> {
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

    Ok(())
}

fn by_capture_limit(passers: &ArgumentPassers) -> std::io::Result<()> {
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
            plot_maker();
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(passers.ms_delay));
    }
    Ok(())
}
fn power_usage() -> std::io::Result<()> {
    let power_input = fs::read_to_string("/sys/class/power_supply/BAT0/power_now")
        .map_err(|e| io::Error::new(io::ErrorKind::NotFound, format!("BAT0 not found: {}", e)))?;
    let power_int: f32 = power_input.trim().parse().map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Failed to parse power: {}", e),
        )
    })?;
    println!("Current watts: {:.2}W", power_int / 1_000_000.0);
    Ok(())
}
fn help() {
    println!(
        "\n
    Current options are: \n 
    -bt | --by-temperature \n
    -bl | --by-capture-limit \n
    -it | --initial_temperature \n
    -et | --end_temperature \n
    -pl | --plot-latest \n
    -d  | --delay \n
    -c  | --captures \n
    -ct | --current-temperature \n
    -bw | --by-watts \n
    -h  | --help \n 
    "
    );
}

fn plot_maker() {
    let app = Application::builder().application_id("twatch").build();

    app.connect_activate(|app| {
        let window = ApplicationWindow::builder()
            .application(app)
            .default_width(320)
            .default_height(200)
            .title("hello")
            .build();
        window.present();
    });

    app.run_with_args(&Vec::<String>::new());
}
