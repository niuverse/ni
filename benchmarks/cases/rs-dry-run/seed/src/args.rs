pub fn run(args:Vec<String>)->Result<(),Box<dyn std::error::Error>> {
    if args.len()!=2 {return Err("expected FILE VALUE".into());}
    println!("{}",rs_dry_run::set_value(std::path::Path::new(&args[0]),&format!("{}\n",args[1]))?);Ok(())
}
