# Log excerpt
Fix excerpt(line,limit) to truncate by Unicode scalar values (Rust chars), not bytes.
Append … only when truncated. Zero limit yields … for nonempty input and "" for empty.
Combining marks count separately. Keep filter(lines,query,limit) ordering/filtering.
CLI: cargo run -- LIMIT QUERY reads stdin. Existing tests: cargo test --offline

Use only the standard library. Do not add dependencies or change build manifests.
