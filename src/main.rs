use std::io;
fn main(){

    let mut ncalc = String::new();

    io::stdin()
        .read_line(&mut ncalc)
        .expect("Failed");

    let ncalc_int: i32 = ncalc.trim().parse().expect("Not a valid number");
    println!("{}", ncalc_int );


}
