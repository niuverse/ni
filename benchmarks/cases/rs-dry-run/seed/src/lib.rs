use std::path::Path;
pub fn set_value(path:&Path,value:&str)->std::io::Result<String> {
    let old=std::fs::read_to_string(path)?;std::fs::write(path,value)?;
    Ok(format!("{} -> {}",old.trim_end(),value.trim_end()))
}
