use std::io;
use std::time::Instant;
fn main() {
    let mut duration = String::new();

    io::stdin().read_line(&mut duration).expect("Failed");

    let start = Instant::now();
    println!("Tempo Total: {:?}", start.elapsed());
}
