pub mod cli;
pub mod plot;
pub mod sensors;
pub mod session;

use crate::{
    cli::{args_processor, ArgumentPassers, SessionType},
    sensors::search_sensors,
    session::session_selector,
};

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
                cli::help();
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
                plot::plot_maker();
                session_type.is_generic = true;
            }
            _ => {
                println!("Argument invalid or not found: {}", arg)
            }
        }
    }

    let _ = session_selector(&mut arg_passers);
    args_processor(&session_type, &arg_passers);
}
