use std::io;
fn main() {
    let mut ncalc = String::new();
    let mut resultado = 0;

    io::stdin().read_line(&mut ncalc).expect("Failed");

    let ncalc_int: i32 = ncalc.trim().parse().expect("Not a valid number");

    for i in 0..ncalc_int {
        let i = i + 2;
        resultado = i;
    }
    if resultado > 0 {
        println!("Funciona.");
    } else {
        println!("Não funciona.");
    }
}
