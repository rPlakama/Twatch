mod plot;
mod sensors;
mod session;

use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use sensors::search_sensors;
use session::{list_sessions, run_session};
use std::{io, process};

#[derive(Parser)]
#[command(name = "twatch", about = "Temperature monitoring and graphing tool")]
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
        long = "no-graph",
        global = true,
        help = "Skip launching the matplotlib graph after a session"
    )]
    no_graph: bool,

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
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Start a temperature recording session")]
    Run {
        #[arg(short = 't', long, conflicts_with = "count", help = "Stop recording when temperature falls outside the initial-end range")]
        by_temperature: bool,

        #[arg(
            short = 'c',
            long,
            help = "Number of captures before stopping"
        )]
        count: Option<u16>,

        #[arg(
            short = 'i',
            long = "initial",
            default_value = "40",
            help = "Start temperature (°C) for --by-temperature mode"
        )]
        initial_temp: u32,

        #[arg(
            short = 'e',
            long = "end",
            default_value = "70",
            help = "Stop temperature (°C) for --by-temperature mode"
        )]
        end_temp: u32,

        #[arg(
            long,
            default_value = "cpu",
            help = "Target sensor for --by-temperature mode: cpu, gpu, or nvme"
        )]
        sensor: String,

        #[arg(long, help = "Output JSON records to stdout instead of TUI")]
        json: bool,
    },

    #[command(about = "Plot session data (matplotlib window)")]
    Graph {
        #[arg(help = "Session IDs (latest if omitted)")]
        session_ids: Vec<u16>,
    },

    #[command(about = "Generate shell completions")]
    Completions {
        #[arg(value_enum)]
        shell: Shell,
    },

    #[command(about = "List recorded sessions")]
    List,

    #[command(name = "temp", about = "Show current CPU temperature")]
    Temp,
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
}

fn main() {
    let cli = Cli::parse();

    let config = Config {
        delay: cli.delay,
        no_graph: cli.no_graph,
        max_plot_temp: cli.max_plot_temp,
        temp_steps: cli.temp_steps,
    };

    match cli.command.unwrap_or(Commands::List) {
        Commands::List => print_sessions(),

        Commands::Temp => {
            let sensors = search_sensors().expect("Unable to receive sensors information");
            let cpu_temp = sensors
                .iter()
                .find(|s| s.is_cpu)
                .map(|s| s.temp)
                .unwrap_or(0);
            println!("CPU TEMP: {}°C", cpu_temp);
        }

        Commands::Run {
            by_temperature,
            count,
            initial_temp,
            end_temp,
            sensor,
            json,
        } => {
            let capture_limit = count.unwrap_or(250);

            run_session(
                &config,
                by_temperature,
                capture_limit,
                initial_temp,
                end_temp,
                &sensor,
                json,
            )
            .expect("Session failed");
        }

        Commands::Graph { session_ids } => {
            let session_exists = list_sessions().map(|s| !s.is_empty()).unwrap_or(false);
            if !session_exists {
                eprintln!("No sessions available to plot.");
                process::exit(1);
            }
            plot::plot_maker(
                &session_ids,
                plot::ScalingPlot {
                    max_plot_temperature: config.max_plot_temp,
                    number_of_steps_for_graph: config.temp_steps,
                },
            );
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

fn print_sessions() {
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
