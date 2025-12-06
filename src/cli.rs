use crate::{
    plot::plot_maker,
    session::{by_capture_limit, trigger_by_temperature},
};
use std::{
    fs,
    io::{self},
    process,
};

pub struct ArgumentPassers {
    pub is_by_temperature: bool,
    pub is_by_capture: bool,
    pub plot_latest: bool,
    pub see_sessions: bool,
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

pub fn args_processor(session_type: &SessionType, passers: &ArgumentPassers) {
    // Singular works
    if passers.plot_latest {
        plot_maker();
        println!("Work is done, bye");
        process::exit(1);
    }

    if !passers.session_exists {
        println!("Unable to find session in current path.");
        process::exit(1);
    }
    // Session validators
    // Is measuring temperatures, but the operator is the temperatures itself
    if session_type.is_temperature && passers.is_by_temperature && !passers.is_by_capture {
        trigger_by_temperature(passers).expect("Unable to start session")
    }
    // Is measuring temperatures, but the operator is the captures
    if session_type.is_temperature && passers.is_by_capture && !passers.is_by_temperature {
        by_capture_limit(passers).expect("Unable to start session")
    }
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

pub fn help() {
    println!(
        "
    Current options are: 
    -bt | --by-temperature 
    -bl | --by-capture-limit 
    -it | --initial_temperature 
    -et | --end_temperature 
    -pl | --plot-latest 
    -d  | --delay 
    -c  | --captures 
    -ct | --current-temperature 
    -bw | --by-watts 
    -h  | --help 
    "
    );
}
