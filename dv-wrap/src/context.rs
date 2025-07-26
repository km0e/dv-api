use super::dev::*;
use crate::{DeviceInfo, cache::MultiCache, interactor::TermInteractor};
use std::collections::HashMap;

#[derive(Debug)]
pub struct Device {
    pub info: DeviceInfo,
    system: Option<String>,
    users: Vec<String>,
}

impl Device {
    pub fn new(info: DeviceInfo) -> Self {
        Self {
            info,
            system: None,
            users: Vec::new(),
        }
    }
}

pub struct Context {
    pub dry_run: bool,
    pub cache: MultiCache,
    pub interactor: TermInteractor,
    pub users: HashMap<String, User>,
    pub devices: HashMap<String, Device>,
}

macro_rules! action {
    ($ctx:expr, $suc:expr, $fmt:expr, $($arg:tt)*) => {
        $ctx.interactor.log(format!(concat!("[{}] {} ",$fmt), if $ctx.dry_run { "n" } else { "a" }, if $suc { "exec" } else { "skip" }, $($arg)*)).await;
    };
}

pub(crate) use action;
use dv_api::whatever;

impl Context {
    pub fn contains_user(&self, uid: impl AsRef<str>) -> bool {
        self.users.contains_key(uid.as_ref())
    }
    pub fn get_user(&self, uid: impl AsRef<str>) -> Result<&User> {
        let uid = uid.as_ref();
        match self.users.get(uid) {
            Some(user) => Ok(user),
            None => {
                whatever!("user {} not found", uid)
            }
        }
    }
    pub async fn add_user(&mut self, uid: String, user: User) -> Result<()> {
        let hid = user.vars.get("hid").cloned();
        if let Some(hid) = hid {
            let hid = hid.to_string();
            let dev = match self.devices.get_mut(&hid) {
                Some(dev) => dev,
                None => {
                    let dev = Device::new(DeviceInfo::detect(&user, user.os()).await?);
                    self.devices.insert(hid.clone(), dev);
                    self.devices.get_mut(&hid).unwrap()
                }
            };
            if user.is_system {
                dev.system = Some(uid.clone());
            } else {
                dev.users.push(uid.clone());
            }
        };
        self.users.insert(uid, user);
        Ok(())
    }
}
