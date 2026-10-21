fn main() {
    let args:Vec<_>=std::env::args().collect();let rules:Vec<_>=args[2..].iter().map(std::path::PathBuf::from).collect();
    for path in rs_scan::collect(std::path::Path::new(&args[1]),&rules).unwrap() {println!("{path}");}
}
