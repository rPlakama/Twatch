use crate::{
    plot::{plot_maker, ScalingPlot},
    session::{by_capture_limit, trigger_by_temperature},
};
use std::process;

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
    pub max_plot_temperature: u16,
    pub number_of_steps_for_graph: u16,
}

pub struct SessionType {
    pub is_temperature: bool,
    pub is_generic: bool,
}

pub fn args_processor(session_type: &SessionType, passers: &ArgumentPassers) {
    // Singular works
    if passers.plot_latest {
        plot_maker(ScalingPlot {
            max_plot_temperature: passers.max_plot_temperature,
            number_of_steps_for_graph: passers.number_of_steps_for_graph,
        });
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

pub fn help() {
    println!(
        "
    Current options are: 
    -bt  | --by-temperature 
    -bl  | --by-capture-limit 
    -it  | --initial-temperature 
    -et  | --end-temperature 
    -pl  | --plot-latest 
    -mtg | --max-temperature-on-graph
    -ts | --temperature-steps
    -ss  | --see-session
    -d   | --delay 
    -c   | --captures 
    -ct  | --current-temperature 
    -h   | --help
    "
    );
}
