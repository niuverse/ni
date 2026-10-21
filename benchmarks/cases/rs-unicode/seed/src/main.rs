use std::io::{self,Read};
fn main() {
    let args:Vec<_>=std::env::args().collect();
    let mut text=String::new();io::stdin().read_to_string(&mut text).unwrap();
    for line in rs_unicode::filter::filter(&text,&args[2],args[1].parse().unwrap()) {println!("{line}");}
}
