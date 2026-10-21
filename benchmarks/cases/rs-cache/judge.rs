use rs_cache::Cache;
#[test] fn expiry() {
    let mut c=Cache::new();c.insert_ttl("x".into(),"one".into(),10,5);
    assert_eq!(c.get_at("x",14),Some("one"));assert_eq!(c.get_at("x",15),None);assert_eq!(c.get("x"),Some("one"));
    c.insert("x".into(),"permanent".into());assert_eq!(c.get_at("x",100),Some("permanent"));
    c.insert_ttl("x".into(),"two".into(),100,2);assert_eq!(c.get_at("x",101),Some("two"));assert_eq!(c.get_at("x",102),None);
    assert_eq!(c.get_at("missing",0),None);
}
#[test] fn zero_overflow() {
    let mut c=Cache::new();c.insert_ttl("z".into(),"v".into(),4,0);assert_eq!(c.get_at("z",4),None);
    c.insert_ttl("s".into(),"v".into(),u64::MAX-2,8);assert_eq!(c.get_at("s",u64::MAX-1),Some("v"));assert_eq!(c.get_at("s",u64::MAX),None);
}
