mod plot;
mod sensors;
mod session;
mod tui_graph;

use clap::{Parser, Subcommand, ValueEnum};
use sensors::search_sensors;
use session::{list_sessions, run_session};
use std::process;

#[derive(Parser)]
#[command(name = "twatch", about = "Temperature monitoring and graphing tool")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    #[arg(short = 'd', long, default_value = "250", global = true)]
    delay: u64,

    #[arg(long = "no-graph", global = true)]
    no_graph: bool,

    #[arg(long = "max-temp", default_value = "110", global = true)]
    max_plot_temp: u16,

    #[arg(long = "temp-steps", default_value = "5", global = true)]
    temp_steps: u16,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Start a temperature recording session")]
    Run {
        #[arg(short = 't', long, conflicts_with = "count")]
        by_temperature: bool,

        #[arg(short = 'c', long)]
        count: Option<u16>,

        #[arg(short = 'i', long = "initial", default_value = "40")]
        initial_temp: u32,

        #[arg(short = 'e', long = "end", default_value = "70")]
        end_temp: u32,

        #[arg(long = "graph-type", value_enum)]
        graph_type: Option<GraphType>,
    },

    #[command(about = "Plot session data (GTK window)")]
    Graph {
        #[arg(help = "Session ID (latest if omitted)")]
        session_id: Option<u16>,
    },

    #[command(name = "graph-tui", about = "Plot session data (terminal)")]
    GraphTui {
        #[arg(help = "Session ID (latest if omitted)")]
        session_id: Option<u16>,
    },

    #[command(about = "List recorded sessions")]
    List,

    #[command(name = "temp", about = "Show current CPU temperature")]
    Temp,
}

#[derive(Clone, ValueEnum)]
pub enum GraphType {
    Gtk,
    Tui,
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
        Commands::List => {
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

        Commands::Temp => {
            let sensors =
                search_sensors().expect("Unable to receive sensors information");
            let cpu_temp = sensors
                .iter()
                .find(|s| s.is_cpu)
                .map(|s| s.temp)
                .expect("Unable to read CPU temperature");
            println!("CPU TEMP: {}°C", cpu_temp);
        }

        Commands::Run {
            by_temperature,
            count,
            initial_temp,
            end_temp,
            graph_type,
        } => {
            run_session(
                &config,
                by_temperature,
                count,
                initial_temp,
                end_temp,
                graph_type,
            )
            .expect("Session failed");
        }

        Commands::Graph { session_id } => {
            let session_exists = list_sessions()
                .map(|s| !s.is_empty())
                .unwrap_or(false);
            if !session_exists {
                eprintln!("No sessions available to plot.");
                process::exit(1);
            }
            plot::plot_maker(
                session_id,
                plot::ScalingPlot {
                    max_plot_temperature: config.max_plot_temp,
                    number_of_steps_for_graph: config.temp_steps,
                },
            );
        }

        Commands::GraphTui { session_id } => {
            let session_exists = list_sessions()
                .map(|s| !s.is_empty())
                .unwrap_or(false);
            if !session_exists {
                eprintln!("No sessions available to plot.");
                process::exit(1);
            }
            tui_graph::show_graph(session_id).expect("Failed to show TUI graph");
        }
    }
}
