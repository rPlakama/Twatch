use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;
use std::thread;
use std::time::{self};

pub struct SensorLabel {
    pub label: String,
    pub is_cpu: bool,
    pub is_nvme: bool,
    pub temp: u32,
}

fn main() {
    loop {
        println!(
            " -- Twach -- \n 1 ― With Unknown-label sensors \n 2 ― Without Unknown-label sensors"
        );
        let mut input = String::new();
        io::stdin().read_line(&mut input).expect("Failed");

        let choice = input.trim();

        print!("\x1B[2J\x1B[1;1H");
        println!("--- Sensor Monitor ---");

        match choice {
            "1" => {
                let _ = sensor_loop();
            }
            "2" => {
                let _ = sensor_loop();
            }
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
                        temp: temp_value,
                    });
                }
            }
        }
    }

    Ok(collected_data)
}
fn sensor_loop() -> std::io::Result<()> {
    let mut session_id = 0;
    let mut _output_name = String::new();

    loop {
        let candidate = format!("sessio_{}.csv", session_id);
        if !Path::new(&candidate).exists() {
            _output_name = candidate;
            break;
        }
        session_id += 1;
    }
    let mut output = File::create(&_output_name)?;
    writeln!(output, "Type,Label,Temp")?;
    loop {
        let sensors = search_sensors()?;

        for sensor in &sensors {
            let device_type = if sensor.is_cpu {
                "CPU"
            } else if sensor.is_nvme {
                "NVME"
            } else {
                "Unknown"
            };

            println!(
                "\x1B[1m[{}] {}: {}°C\x1B[0m",
                device_type, sensor.label, sensor.temp
            );

            writeln!(output, "{},{},{}", device_type, sensor.label, sensor.temp)?;
            output.flush()?;
            thread::sleep(time::Duration::from_secs(1));
        }
    }
}
