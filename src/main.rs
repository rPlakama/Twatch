use std::io::{self, Write};
use std::time::{Duration, Instant};

fn main() {
    println!("Enter Duration(S):  ");
    let mut _i = 0u64;

    let mut input_duration = String::new();
    io::stdin().read_line(&mut input_duration).expect("Failed");
    let seconds: u64 = input_duration.trim().parse().expect(
        "Enter a valid input, if it was a number, please use ( 0 to 18,446,744,073,709,551,615 )",
    );

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
