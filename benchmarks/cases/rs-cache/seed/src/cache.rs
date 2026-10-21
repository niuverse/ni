use std::collections::HashMap;
pub struct Cache {values:HashMap<String,String>}
impl Default for Cache {fn default()->Self {Self::new()}}
impl Cache {
    pub fn new()->Self {Self {values:HashMap::new()}}
    pub fn insert(&mut self,key:String,value:String) {self.values.insert(key,value);}
    pub fn get(&self,key:&str)->Option<&str> {self.values.get(key).map(String::as_str)}
}
