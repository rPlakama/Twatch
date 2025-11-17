use std::fs;
use std::io::{self, Write};
use std::time::{Duration, Instant};

fn countdown() {
    println!("Enter Duration(S):  ");
    let mut _i = 0u64;

    let mut input_duration = String::new();
    io::stdin().read_line(&mut input_duration).expect("Failed");
    let seconds: u64 = input_duration.trim().parse().expect(".");

    let duration = Duration::from_secs(seconds);
    let start = Instant::now();

    while Instant::now() - start <= duration {
        let elapse = Instant::now() - start;
        let remaining = duration
            .checked_sub(elapse)
            .unwrap_or_else(|| Duration::from_secs(0));
        print!("\rTime to end: {}s ", remaining.as_secs());
        std::io::stdout().flush().unwrap();
        while Instant::now() - start < duration {}
    }
}
fn search() {
    let hmowns = fs::read_dir("/sys/class/hwmon/").unwrap();
    println!("{:?}", hmowns);
}

fn main() {
    println!("Select funtion: \n 1 ― Sensors. \n 2 ― Countdown");

    let mut choice = String::new();
    io::stdin().read_line(&mut choice).expect("Failed");

    if choice.trim() == "1" {
        search();
    } else if choice.trim() == "2" {
        countdown();
    };
}
