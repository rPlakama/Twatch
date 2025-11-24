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

fn record_frame(
    session: &mut SessionFile,
    countdown: usize,
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

fn main() -> std::io::Result<()> {
    loop {
        let cpu_temp = search_sensors()?
            .into_iter()
            .find(|s| s.is_cpu)
            .map(|s| s.temp)
            .unwrap_or(0);

        println!("Current (CPU) TEMP: {}", cpu_temp);
        println!("1. -- Raw Session");
        println!("2. -- Selection Session");
        println!("3. -- Plot Latest");
        println!("4. -- Trigger");
        println!("5  -- Quit");

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        let choice = input.trim();

        match choice {
            "1" => {
                if let Err(e) = sensor_loop() {
                    eprintln!("Error in sensor loop: {}", e);
                }
            }
            "2" => {
                session_selector();
            }
            "3" => {
                plot_maker();
            }
            "4" => trigger()?,
            "5" => break Ok(()),
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
        record_frame(&mut session, countdown, "--- Sensor Monitor ---")?;
        thread::sleep(time::Duration::from_millis(250));
    }
}

fn trigger() -> std::io::Result<()> {
    println!(" -- 1 By temperature \n -- 2 By captures");

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let input_trim = input.trim();
    if input_trim == "1" {
        print!("\r\x1B[2J\x1B[1;1H");

        println!("Input start trigger temperature:");
        let mut temp = String::new();
        io::stdin().read_line(&mut temp).expect("Failed");
        let start_limit: u32 = temp.trim().parse().unwrap_or(0);

        println!("Input end trigger temperature (Stop if > this):");
        let mut end_temp = String::new();
        io::stdin().read_line(&mut end_temp).expect("Failed");
        let end_limit: u32 = end_temp.trim().parse().unwrap_or(0);

        println!("Digit the scan cooldown(ms):");
        let mut cooldown_str = String::new();
        io::stdin().read_line(&mut cooldown_str).expect("Failed");
        let cooldown_ms: u64 = cooldown_str.trim().parse().unwrap_or(250);

        let mut session = session_writter()?;
        let mut countdown = 0;
        let mut _plot_flag = false;
        print!("\x1B[2J");

        loop {
            print!("\r\x1B[2J\x1B[1;1H");

            countdown += 1;

            let status_header = format!(
                "--- Trigger Monitor ---\nRange: [Start: {}°C, End: {}°C] \n",
                start_limit, end_limit
            );

            let sensors = record_frame(&mut session, countdown, &status_header)?;

            let cpu_temp = sensors
                .iter()
                .find(|s| s.is_cpu)
                .map(|s| s.temp)
                .unwrap_or(0);

            println!("\nStatus:");

            if cpu_temp >= start_limit {
                println!(
                    "\x1B[33m Trigger Active: {}°C >= {}°C\x1B[0m",
                    cpu_temp, start_limit
                );
            } else {
                println!("Trigger to be reached...");
            }

            if cpu_temp > end_limit {
                println!("\x1B[31m Limit reached ({}°C).\x1B[0m", cpu_temp);
                writeln!(session.file, "CPU,Exit,{}", cpu_temp)?;
                _plot_flag = true;
                break;
            }

            std::thread::sleep(std::time::Duration::from_millis(cooldown_ms));
        }

        if _plot_flag {
            plot_maker();
        }

        Ok(())
    } else if input_trim == "2" {
        trigger_by_timeout()
    } else {
        println!("Invalid input");
        Ok(())
    }
}

fn trigger_by_timeout() -> std::io::Result<()> {
    println!("Input delay between captures");
    let mut input = String::new();
    io::stdin().read_line(&mut input).expect("Failed");
    let cooldown_ms: u64 = input.trim().parse().unwrap_or(250);

    println!("input amount of caputures");
    let mut input = String::new();
    io::stdin().read_line(&mut input).expect("Failed");
    let amount: u16 = input.trim().parse().unwrap_or(2500);

    let mut session = session_writter()?;

    for i in 1..amount {
        print!("\x1B[2J");
        let header = format!("\r-- By capture limit -- \n Target: {} frames \n", amount);

        record_frame(&mut session, i as usize, &header)?;

        std::thread::sleep(std::time::Duration::from_millis(cooldown_ms));
    }
    println!(
        "\rCaptures completed, total of {} frames, requesting graph.",
        amount
    );

    plot_maker();

    Ok(())
}

fn session_selector() {
let current = fs::read_dir("./")
    .expect("Could not read the current path");

}
