use std::io;
use std::time::Instant;
fn main() {
    let mut ncalc = String::new();
    let mut resultado = 0;

    io::stdin().read_line(&mut ncalc).expect("Failed");

    let ncalc_int: i32 = ncalc.trim().parse().expect("Not a valid number");

    let start = Instant::now();
    for i in 0..ncalc_int {
        let i = i + 2000;
        resultado = i;
    }
    if resultado > 0 {
        println!("Tempo Total: {:?}", start.elapsed());
    };
}
