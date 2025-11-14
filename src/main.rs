use std::io;
use std::time::Duration;
fn main() {
    println!("Enter Duration(S):  ");

    let mut input_duration = String::new();
    io::stdin().read_line(&mut input_duration).expect("Failed");
    let seconds: u64 = input_duration.trim().parse().expect(
        "Enter a valid input, if it was a number, please use ( 0 to 18,446,744,073,709,551,615 )",
    );

    let duration = Duration::from_secs(seconds);
    while duration > Duration::from_secs(0) {
        // Agora fazer a estrutura do loop 

    }
}
