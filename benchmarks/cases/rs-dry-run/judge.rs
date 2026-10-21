use std::process::Command;
#[test] fn cli_and_library() {
    let path=std::env::temp_dir().join(format!("dry-run-{}",std::process::id()));std::fs::write(&path,"old\n").unwrap();
    let modified=std::fs::metadata(&path).unwrap().modified().unwrap();
    let run=|args:&[&str]|Command::new(env!("CARGO_BIN_EXE_rs-dry-run")).args(args).output().unwrap();
    let p=path.to_str().unwrap();let dry=run(&["--dry-run",p,"new"]);assert!(dry.status.success());
    assert_eq!(String::from_utf8(dry.stdout).unwrap(),"old -> new\n");assert_eq!(std::fs::read(&path).unwrap(),b"old\n");
    assert_eq!(std::fs::metadata(&path).unwrap().modified().unwrap(),modified);
    assert!(run(&[p,"new"]).status.success());assert_eq!(std::fs::read(&path).unwrap(),b"new\n");
    assert_eq!(rs_dry_run::set_value(&path,"last").unwrap(),"new -> last");
    assert!(!run(&["--dry-run"]).status.success());std::fs::remove_file(&path).unwrap();
    assert!(!run(&["--dry-run",p,"new"]).status.success());assert!(!path.exists());
}
