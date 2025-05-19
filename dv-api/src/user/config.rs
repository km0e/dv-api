use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
};

#[derive(Debug, Default)]
pub struct Config {
    // pub is_system: Option<bool>,
    pub vars: HashMap<String, String>,
}

impl Deref for Config {
    type Target = HashMap<String, String>;

    fn deref(&self) -> &Self::Target {
        &self.vars
    }
}

impl DerefMut for Config {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.vars
    }
}

impl Config {
    pub fn hid(&self) -> Option<&str> {
        self.get("HID").map(|s| s.as_str())
    }
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) -> Option<String> {
        self.vars.insert(key.into(), value.into())
    }
}
