mod plot;
mod sensors;
mod session;

use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use plot::run_graph_viewer;
use sensors::{read_cpu_freq, read_gpu_freq, search_sensors, squash_sensors, DeviceKind};
use session::{delete_sessions, list_sessions, list_sessions_detailed, run_session};
use std::{io, process};

#[derive(Parser)]
#[command(
    name = "twatch",
    version,
    about = "btop-grade temperature monitoring and graphing TUI"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    #[arg(
        short = 'd',
        long,
        default_value = "250",
        global = true,
        help = "Milliseconds between sensor captures"
    )]
    delay: u64,

    #[arg(
        long = "max-temp",
        default_value = "110",
        global = true,
        help = "Maximum temperature (°C) on the plot Y-axis"
    )]
    max_plot_temp: u16,

    #[arg(
        long = "temp-steps",
        default_value = "5",
        global = true,
        help = "Grid step interval (°C) on the plot Y-axis"
    )]
    temp_steps: u16,

    #[arg(
        short = 'f',
        long = "full-devices-sensors",
        global = true,
        help = "Expose all individual sub-sensors (e.g. all NVMe channels) without squashing"
    )]
    full_devices_sensors: bool,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Start a temperature recording session")]
    Run {
        #[arg(
            short = 't',
            long,
            conflicts_with = "count",
            help = "Stop recording when temperature reaches the end temperature threshold"
        )]
        by_temperature: bool,

        #[arg(short = 'c', long, help = "Number of captures before stopping")]
        count: Option<u16>,

        #[arg(
            short = 'i',
            long = "initial",
            default_value = "40.0",
            help = "Start temperature (°C) for --by-temperature mode"
        )]
        initial_temp: f64,

        #[arg(
            short = 'e',
            long = "end",
            default_value = "70.0",
            help = "Stop temperature (°C) for --by-temperature mode"
        )]
        end_temp: f64,

        #[arg(
            long,
            value_enum,
            default_value = "cpu",
            help = "Target sensor for --by-temperature trigger mode"
        )]
        sensor: TargetSensorKind,

        #[arg(long, help = "Output JSON records to stdout instead of TUI")]
        json: bool,

        #[arg(long = "no-graph", help = "Skip launching the graph after a session")]
        no_graph: bool,
    },

    #[command(about = "Launch live btop-style thermal monitor (no recording)")]
    Live {
        #[arg(short = 'r', long, help = "Also record session to CSV while monitoring")]
        record: bool,

        #[arg(
            long,
            value_enum,
            default_value = "cpu",
            help = "Target sensor to track in header"
        )]
        sensor: TargetSensorKind,
    },

    #[command(about = "Plot session data in interactive terminal graph viewer")]
    Graph {
        #[arg(help = "Session IDs to plot or compare (latest session if omitted)")]
        session_ids: Vec<u16>,
    },

    #[command(about = "Delete recorded session files")]
    Delete {
        #[arg(required = true, help = "Session IDs to delete")]
        session_ids: Vec<u16>,
    },

    #[command(about = "Generate shell completions")]
    Completions {
        #[arg(value_enum)]
        shell: Shell,
    },

    #[command(about = "List recorded sessions with detailed metrics")]
    List {
        #[arg(short = 's', long, help = "Show simple list without extra metadata")]
        simple: bool,
    },

    #[command(name = "temp", about = "Show current temperatures across all sensors")]
    Temp,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum TargetSensorKind {
    Cpu,
    Gpu,
    Nvme,
}

impl TargetSensorKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            TargetSensorKind::Cpu => "cpu",
            TargetSensorKind::Gpu => "gpu",
            TargetSensorKind::Nvme => "nvme",
        }
    }
}

#[derive(Clone, ValueEnum)]
enum Shell {
    Bash,
    Zsh,
    Fish,
}

pub struct Config {
    pub delay: u64,
    pub no_graph: bool,
    pub max_plot_temp: u16,
    pub temp_steps: u16,
    pub full_devices_sensors: bool,
}

fn main() {
    let cli = Cli::parse();

    let config = Config {
        delay: cli.delay,
        no_graph: false,
        max_plot_temp: cli.max_plot_temp,
        temp_steps: cli.temp_steps,
        full_devices_sensors: cli.full_devices_sensors,
    };

    match cli.command.unwrap_or(Commands::List { simple: false }) {
        Commands::List { simple } => {
            if simple {
                print_sessions_simple();
            } else {
                print_sessions_detailed();
            }
        }

        Commands::Temp => print_current_temperatures(config.full_devices_sensors),

        Commands::Live { record, sensor } => {
            let mut cfg = config;
            cfg.no_graph = true;
            run_session(
                &cfg,
                false,
                u16::MAX,
                40.0,
                100.0,
                sensor.as_str(),
                false,
                record,
            )
            .expect("Live monitor session failed");
        }

        Commands::Run {
            by_temperature,
            count,
            initial_temp,
            end_temp,
            sensor,
            json,
            no_graph,
        } => {
            let capture_limit = count.unwrap_or(250);
            let mut cfg = config;
            cfg.no_graph = no_graph;

            run_session(
                &cfg,
                by_temperature,
                capture_limit,
                initial_temp,
                end_temp,
                sensor.as_str(),
                json,
                true,
            )
            .expect("Session recording failed");
        }

        Commands::Graph { session_ids } => {
            let session_exists = list_sessions().map(|s| !s.is_empty()).unwrap_or(false);
            if !session_exists {
                eprintln!("No sessions available to plot. Run 'twatch run' first.");
                process::exit(1);
            }
            if let Err(e) = run_graph_viewer(
                &session_ids,
                config.max_plot_temp,
                config.temp_steps,
                config.full_devices_sensors,
            ) {
                eprintln!("Graph viewer error: {}", e);
                process::exit(1);
            }
        }

        Commands::Delete { session_ids } => {
            match delete_sessions(&session_ids) {
                Ok(count) => {
                    println!("Successfully deleted {} session file(s).", count);
                }
                Err(e) => {
                    eprintln!("Error deleting sessions: {}", e);
                    process::exit(1);
                }
            }
        }

        Commands::Completions { shell } => {
            use clap_complete::{generate, shells};
            let mut cmd = Cli::command();
            let name = "twatch";
            match shell {
                Shell::Bash => generate(shells::Bash, &mut cmd, name, &mut io::stdout()),
                Shell::Zsh => generate(shells::Zsh, &mut cmd, name, &mut io::stdout()),
                Shell::Fish => generate(shells::Fish, &mut cmd, name, &mut io::stdout()),
            }
        }
    }
}

fn print_sessions_simple() {
    match list_sessions() {
        Ok(sessions) => {
            if sessions.is_empty() {
                println!("No sessions found. Run 'twatch run' to create one.");
            } else {
                println!("Sessions:");
                for (id, path) in &sessions {
                    println!("  [{}] {}", id, path.display());
                }
            }
        }
        Err(e) => eprintln!("Error listing sessions: {}", e),
    }
}

fn print_sessions_detailed() {
    match list_sessions_detailed() {
        Ok(metas) => {
            if metas.is_empty() {
                println!("No sessions found. Run 'twatch run' to create one.");
                return;
            }

            println!("Twatch Recorded Sessions (Total: {}):", metas.len());
            println!("{:-<80}", "");
            println!(
                "{:<6} {:<10} {:<10} {:<12} {:<36}",
                "ID", "SAMPLES", "DURATION", "PEAK TEMP", "PATH"
            );
            println!("{:-<80}", "");

            for m in &metas {
                let dur_str = m
                    .duration_sec
                    .map(|d| format!("{:.1}s", d))
                    .unwrap_or_else(|| {
                        if let Some(delay) = m.delay_ms {
                            format!("{:.1}s", (m.sample_count as f64 * delay as f64) / 1000.0)
                        } else {
                            "-".to_string()
                        }
                    });

                let peak_str = m
                    .peak_temp
                    .map(|p| format!("{:.1}°C", p))
                    .unwrap_or_else(|| "-".to_string());

                println!(
                    "#{:<5} {:<10} {:<10} {:<12} {:<36}",
                    m.id,
                    m.sample_count,
                    dur_str,
                    peak_str,
                    m.path.display()
                );
            }
            println!("{:-<80}", "");
            println!("Tip: Run 'twatch graph <ID>' to view interactive charts.");
        }
        Err(e) => eprintln!("Error reading sessions: {}", e),
    }
}

fn print_current_temperatures(full_sensors: bool) {
    match search_sensors() {
        Ok(raw_sensors) => {
            if raw_sensors.is_empty() {
                println!("No hardware thermal sensors detected.");
                return;
            }

            let sensors = squash_sensors(&raw_sensors, full_sensors);
            let cpu_freq = read_cpu_freq();
            let gpu_freq = read_gpu_freq();

            println!("Hardware Thermal Sensors:");
            println!("{:-<55}", "");

            let mut current_group: Option<(DeviceKind, String)> = None;
            for s in &sensors {
                let group = (s.kind, s.display_name());
                if current_group.as_ref() != Some(&group) {
                    let freq_str = match s.kind {
                        DeviceKind::Cpu => {
                            cpu_freq.map_or(String::new(), |f| {
                                format!(" (boost: {:.2} GHz | avg: {:.2} GHz)", f.max_ghz, f.avg_ghz)
                            })
                        }
                        DeviceKind::Gpu => {
                            gpu_freq.map_or(String::new(), |f| format!(" ({:.2} GHz)", f))
                        }
                        _ => String::new(),
                    };
                    // Full-sensor labels are already prefixed with the device
                    // name, so only show it in the header for the default view.
                    let name = if full_sensors {
                        String::new()
                    } else {
                        format!(" {}", group.1)
                    };
                    println!("\n  [{}]{}{}", s.kind, name, freq_str);
                    current_group = Some(group);
                }

                let color_prefix = if s.temp >= 85.0 {
                    "\x1b[1;31m" // Bold Red
                } else if s.temp >= 70.0 {
                    "\x1b[33m" // Yellow
                } else if s.temp >= 55.0 {
                    "\x1b[32m" // Green
                } else {
                    "\x1b[36m" // Cyan
                };
                let reset = "\x1b[0m";

                let crit_info = match (s.max_temp, s.crit_temp) {
                    (_, Some(crit)) => format!(" (crit: {:.0}°C)", crit),
                    (Some(max), None) => format!(" (max: {:.0}°C)", max),
                    (None, None) => String::new(),
                };

                println!(
                    "    {:<24} {}{:>6.1}°C{}{}",
                    s.label, color_prefix, s.temp, reset, crit_info
                );
            }
            if !full_sensors {
                println!("\nTip: Use -f / --full-devices-sensors to expose all device channels.");
            }
            println!("{:-<55}", "");
        }
        Err(e) => eprintln!("Unable to read sensors: {}", e),
    }
}
