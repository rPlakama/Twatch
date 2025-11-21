use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;
use std::process::Command;
use std::thread;
use std::time::{self};

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

fn plot_maker() {
    println!("Launching python plotter...");
    let child = Command::new("python").arg("graph.py").spawn();
    match child {
        Ok(_) => println!("Plotter started sucesfully"),
        Err(e) => eprintln!("Failed to start plotter: {}", e),
    }
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

fn main() {
    loop {
        println!("1. -- Raw Session");
        println!("2. -- Plot Latest");
        println!("3. -- Trigger");
        println!("4  -- Quit");

        let mut input = String::new();
        io::stdin().read_line(&mut input).expect("Failed");

        let choice = input.trim();

        match choice {
            "1" => {
                if let Err(e) = sensor_loop() {
                    eprintln!("Error in sensor loop: {}", e);
                }
            }
            "2" => {
                plot_maker();
            }
            "3" => {
                trigger();
            }
            "4" => break,
            _ => {
                println!("Invalid Selection.");
            }
        }
    }
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

fn sensor_loop() -> std::io::Result<()> {
    let mut session = session_writter()?;
    let mut countdown = 0;

    loop {
        countdown += 1;
        let sensors = search_sensors()?;
        let mut display_tui = String::new();

        print!("\x1B[2J\x1B[1;1H");
        println!("--- Sensor Monitor ---");
        println!("  Current Capture: {}", countdown);

        for sensor in &sensors {
            let device_type = device_type(sensor);

            display_tui.push_str(&format!(
                " \x1B[1m [{}] {}: {}°C\x1B[0m \n",
                device_type, sensor.label, sensor.temp
            ));

            writeln!(
                session.file,
                "{},{},{}",
                device_type, sensor.label, sensor.temp
            )?;
            session.file.flush()?;
        }
        print!("{}", display_tui);
        io::stdout().flush()?;
        thread::sleep(time::Duration::from_millis(250));
    }
}
fn trigger() -> std::io::Result<()> {
    print!("\x1B[2J\x1B[1;1H");

    println!("Input start trigger temperature");

    let mut temp = String::new();
    io::stdin().read_line(&mut temp).expect("Failed");
    let temp_int: u32 = temp.trim().parse().unwrap_or(0);

    println!("Input end trigger temperature");

    let mut end_temp = String::new();
    io::stdin().read_line(&mut end_temp).expect("Failed");
    let end_temp_int: u32 = end_temp.trim().parse().unwrap_or(0);

    println!("Digit the scan couldown(ms)");
    let mut couldown = String::new();
    io::stdin().read_line(&mut couldown).expect("Failed");
    let int_couldown: u64 = couldown.trim().parse().unwrap_or(0);

    let mut session = session_writter()?;
    let mut countdown = 0;
    let mut _plot_flag = false;

    loop {
        countdown += 1;

        let cpu_temp = search_sensors()?
            .into_iter()
            .find(|s| s.is_cpu)
            .map(|s| s.temp)
            .unwrap_or(0);
        print!("\x1B[2J\x1B[1;1H");
        println!("--- Trigger Monitor ---");
        println!("  Capture: {}", countdown);
        println!("  CPU Temp: {}°C", cpu_temp);
        println!("  Range: [{}, {}]", temp_int, end_temp_int);

        writeln!(session.file, "CPU,Current,{}", cpu_temp)?;
        session.file.flush()?;

        if cpu_temp >= temp_int {
            println!("Trigger activated: {}°C >= {}°C", cpu_temp, temp_int);
        }
        if end_temp_int < cpu_temp {
            writeln!(session.file, "CPU,Exit, {}", cpu_temp)?;
            _plot_flag = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(int_couldown));
    }
    if _plot_flag {
        plot_maker();
    }
    Ok(())
}
