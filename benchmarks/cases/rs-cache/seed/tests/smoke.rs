#[test] fn permanent() {
    let mut c=rs_cache::Cache::new();c.insert("x".into(),"v".into());assert_eq!(c.get("x"),Some("v"));
}
