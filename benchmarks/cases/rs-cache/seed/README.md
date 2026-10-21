# String cache
Add insert_ttl(key:String,value:String,now:u64,ttl:u64) and
get_at(key:&str,now:u64)->Option<&str> to Cache. Expire at now >= insertion+ttl;
addition saturates at u64::MAX. Zero TTL expires immediately. insert replaces
any previous TTL with a permanent value. Preserve insert/get: get ignores expiry,
get_at checks it. Time is caller-supplied, no sleeping or wall clock.
Existing tests: cargo test --offline

Use only the standard library. Do not add dependencies or change build manifests.
