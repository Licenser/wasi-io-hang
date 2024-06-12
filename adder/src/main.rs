use std::io::{stdin, stdout, Write};
fn main() {
    let lines = stdin().lines();
    let mut last = None;
    for line in lines {
        let this: u64 = dbg!(line.unwrap().parse()).unwrap();
        if let Some(last) = dbg!(last).take() {
            println!("{:?}", last + this);
            dbg!();
            dbg!(stdout().flush()).unwrap();
        } else {
            dbg!();
            last = Some(this);
        }
    }
}
