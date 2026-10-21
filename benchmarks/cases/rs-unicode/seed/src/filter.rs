pub fn filter(lines:&str,query:&str,limit:usize)->Vec<String> {
    lines.lines().filter(|line|line.contains(query)).map(|line|crate::excerpt(line,limit)).collect()
}
