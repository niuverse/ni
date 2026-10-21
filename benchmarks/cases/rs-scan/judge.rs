use std::path::{Path,PathBuf};
use rs_scan::{collect,rules::excluded};
#[test] fn boundaries() {
    let rules=vec![PathBuf::from("build"),PathBuf::from("src/generated"),PathBuf::new()];
    for p in ["build","build/a","src/generated/x"] {assert!(excluded(Path::new(p),&rules));}
    for p in ["builder/x","build.rs","src/generated_extra/x","other/build/x","src/a"] {assert!(!excluded(Path::new(p),&rules));}
}
#[test] fn sorted_tree() {
    let root=std::env::temp_dir().join(format!("inventory-test-{}",std::process::id()));std::fs::create_dir_all(&root).unwrap();
    for file in ["z.txt","builder/a","build/no","src/generated/no","src/generated_extra/yes"] {
        let path=root.join(file);std::fs::create_dir_all(path.parent().unwrap()).unwrap();std::fs::write(path,"x").unwrap();
    }
    #[cfg(unix)] std::os::unix::fs::symlink(root.join("builder"),root.join("link")).unwrap();
    let got=collect(&root,&["build".into(),"src/generated".into()]).unwrap();std::fs::remove_dir_all(root).unwrap();
    assert_eq!(got,vec!["builder/a","src/generated_extra/yes","z.txt"]);
}
