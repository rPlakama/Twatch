use std::{
    fs::{self, File},
    io::{self, Write},
    path::Path,
    process::Command,
};

pub struct SessionFile {
    pub id: u32,
    pub file: File,
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

fn record_frame(
    session: &mut SessionFile,
    countdown: u64,
    header_msg: &str,
) -> std::io::Result<Vec<SensorLabel>> {
    let sensors = search_sensors()?;

    let mut display_tui: String = Default::default();

    display_tui.push_str(&format!("{}", header_msg));
    display_tui.push_str(&format!("Current Capture: {}", countdown));

    for sensor in &sensors {
        let d_type = device_type(sensor);

        println!("");
        display_tui.push_str(&format!(
            "\n [{}] {}: {}°C",
            d_type, sensor.label, sensor.temp
        ));

        writeln!(session.file, "{},{},{}", d_type, sensor.label, sensor.temp)?;
    }

    display_tui.push_str("\x1B[J");

    println!("{}", display_tui);

    session.file.flush()?;

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

fn args_processor(passers: &ArgumentPassers) {
    match (
        passers.is_by_temperature,
        passers.is_by_capture,
        passers.plot_latest,
        passers.session_exists,
    ) {
        (true, false, _, false) => {
            if let Err(e) = trigger_by_temperature(passers) {
                eprintln!("Error during temperature monitoring: {}", e);
            }
        }
        (false, true, _, _) => {
            if let Err(e) = by_capture_limit(passers) {
                eprintln!("Error during the monitoring: {}", e);
            }
        }
        (_, true, _, _) => {
            plot_maker();
        }
        (_, _, true, _) => {
            println!("Unable to find session file, do a capture first.");
        }
        _ => {}
    }
}
fn main() {
    let mut args = std::env::args().skip(1);

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

    let sensors = search_sensors().unwrap_or_default();

    let cpu_temp = sensors
        .iter()
        .find(|s| s.is_cpu)
        .map(|s| s.temp)
        .unwrap_or(0);
    while let Some(arg) = args.next() {
        match &arg[..] {
            "-bt" | "--by-temperature" => {
                arg_passers.is_by_temperature = true;
            }
            "-h" | "--help" => {
                help();
            }
            "-pl" | "--plot-latest" => {
                arg_passers.plot_latest = true;
            }
            "-d" | "--delay" => {
                if let Some(val_str) = args.next() {
                    arg_passers.ms_delay = val_str.parse().unwrap_or(250);
                    println!("You are missing arguments");
                } else {
                    eprintln!("Error: -d | --d requires (value) in (ms)");
                    return;
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
            _ => {
                println!("Argument invalid or not found {}", arg)
            }
        }
    }
    let _ = session_selector(&mut arg_passers);
    args_processor(&arg_passers);
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
            .unwrap_or_default()
            .trim()
            .to_string();
        let is_nvme = device_name.contains("nvme");
        let is_cpu = device_name.contains("coretemp") || device_name.contains("k10temp");
        let is_amd_gpu = device_name.contains("amdgpu");

        if let Ok(entries) = fs::read_dir(&path) {
            for entry in entries.filter_map(Result::ok) {
                let file_name = entry.file_name().to_string_lossy().to_string();
                if file_name.starts_with("temp") && file_name.ends_with("_input") {
                    let temp_string = fs::read_to_string(entry.path()).unwrap_or_default();
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
fn session_writter() -> std::io::Result<SessionFile> {
    let mut session_id = 0;

    loop {
        let condidate = format!("session/session_{}.csv", session_id);
        if !Path::new(&condidate).exists() {
            if let Some(parent) = Path::new(&condidate).parent() {
                std::fs::create_dir_all(parent)?;
            }

            let mut file = File::create(&condidate)?;
            writeln!(file, "Type,Label,Temp")?;

            return Ok(SessionFile {
                id: session_id,
                file: file,
            });
        }
        session_id += 1;
    }
}

fn trigger_by_temperature(passers: &ArgumentPassers) -> std::io::Result<()> {
    print!("\r\x1B[2J\x1B[1;1H");
    let mut session = session_writter()?;
    let mut countdown = 0;
    let mut _plot_flag = false;
    print!("\x1B[2J");

    loop {
        print!("\r\x1B[2J\x1B[1;1H");

        countdown += 1;

        let status_header = format!(
            "--- Trigger Monitor ---\nRange: [Start: {}°C, End: {}°C] \n",
            passers.initial_temperature, passers.end_temperature
        );

        let sensors = record_frame(&mut session, countdown, &status_header)?;

        let cpu_temp = sensors
            .iter()
            .find(|s| s.is_cpu)
            .map(|s| s.temp)
            .unwrap_or(0);

        println!("\nStatus:");

        if cpu_temp >= passers.initial_temperature {
            println!(
                "\x1B[33m Trigger Active: {}°C >= {}°C\x1B[0m",
                cpu_temp, passers.initial_temperature
            );
        } else {
            println!("Trigger to be reached...");
        }

        if cpu_temp > passers.end_temperature {
            println!("\x1B[31m Limit reached ({}°C).\x1B[0m", cpu_temp);
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

fn plot_maker() {
    let exe_path = match std::env::current_exe() {
        Ok(path) => path,
        Err(e) => {
            eprintln!("Failed to get current executable path: {}", e);
            return;
        }
    };

    let script_path = if let Some(dir) = exe_path.parent() {
        dir.join("graph.py")
    } else {
        eprintln!("Failed to get parent directory of executable");
        return;
    };

    match Command::new("python").arg(&script_path).status() {
        Ok(status) => match status.code() {
            Some(0) => println!("Success!"),
            Some(1) => println!(
                "Python script failed with an error. Is '{}' the correct path?",
                script_path.display()
            ),
            Some(code) => println!("Exited with code: {}", code),
            None => println!("Process terminated by signal"),
        },
        Err(e) => println!("Failed to execute python: {}. Is python in your PATH?", e),
    }
}

fn by_capture_limit(passers: &ArgumentPassers) -> std::io::Result<()> {
    print!("\r\x1B[2J\x1B[1;1H");
    let mut session = session_writter()?;
    let mut countdown = 0;
    let mut _plot_flag = false;
    print!("\x1B[2J");

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
            .unwrap_or(0);

        if countdown > passers.amount_captures {
            println!(
                "\x1B[31m Target reached ([{}]).\x1B[0m",
                passers.amount_captures
            );
            writeln!(session.file, "CPU,Exit,{}", cpu_temp)?;
            plot_maker();
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(passers.ms_delay));
    }
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
    -h  | --help \n 
    "
    );
}
