pub mod filter;
pub fn excerpt(line:&str,limit:usize)->String {
    if line.len()<=limit {line.to_owned()} else {format!("{}…",&line[..limit])}
}
#[cfg(test)] mod tests {
    #[test] fn ascii() {assert_eq!(super::excerpt("hello",3),"hel…");}
}
