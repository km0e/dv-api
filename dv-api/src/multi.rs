use std::collections::HashMap;
mod dev {
    pub use super::Config;
    pub use crate::Result;
    pub use crate::core::*;
    pub use crate::fs::*;
    pub use crate::process::*;
    pub use crate::whatever;
    pub use async_trait::async_trait;
    pub use std::borrow::Cow;
    pub use tracing::{debug, trace};
}

#[derive(Debug, Clone, Default)]
pub struct Config {
    pub is_system: Option<bool>,
    pub variables: HashMap<String, String>,
}

impl Config {
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.variables.insert(key.into(), value.into());
    }

    pub fn get(&self, key: impl AsRef<str>) -> Option<&String> {
        self.variables.get(key.as_ref())
    }

    pub fn remove(&mut self, key: impl AsRef<str>) -> Option<String> {
        self.variables.remove(key.as_ref())
    }

    pub fn session(&self) -> Option<&String> {
        self.get("XDG_SESSION_TYPE")
    }
}

mod local;
pub use local::create as create_local;

mod ssh;
pub use ssh::create as create_ssh;

use crate::core::into_boxed_user;
use dev::BoxedUser;

into_boxed_user!(local::This, ssh::SSHSession);
