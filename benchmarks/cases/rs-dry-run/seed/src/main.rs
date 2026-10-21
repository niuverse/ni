mod args;
fn main() {
    if let Err(e)=args::run(std::env::args().skip(1).collect()) {eprintln!("{e}");std::process::exit(2);}
}
