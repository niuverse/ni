pub mod rules;
use std::path::{Path,PathBuf};
pub fn collect(root:&Path,rules:&[PathBuf])->std::io::Result<Vec<String>> {
    fn visit(root:&Path,dir:&Path,rules:&[PathBuf],out:&mut Vec<String>)->std::io::Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry=entry?;let path=entry.path();let relative=path.strip_prefix(root).unwrap();
            if crate::rules::excluded(relative,rules) {continue;}
            let kind=entry.file_type()?;
            if kind.is_dir() {visit(root,&path,rules,out)?;} else if kind.is_file() {out.push(relative.to_string_lossy().replace('\\',"/"));}
        }
        Ok(())
    }
    let mut out=vec![];visit(root,root,rules,&mut out)?;out.sort();Ok(out)
}
#[cfg(test)] mod tests {
    #[test] fn exact() {assert!(crate::rules::excluded(std::path::Path::new("build/x"),&["build".into()]));}
}
