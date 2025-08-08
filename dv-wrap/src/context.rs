use super::dev::*;
use crate::{DeviceInfo, cache::MultiCache, interactor::TermInteractor};
use std::collections::HashMap;

#[derive(Debug)]
pub struct Device {
    pub info: DeviceInfo,
    pub system: Option<String>,
    pub users: Vec<String>,
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
        use crossterm::style::Stylize;
        $ctx.interactor.log(format!(concat!("[{}] {} ",$fmt), if $ctx.dry_run { "n" } else { "a" }, if $suc { "exec".green() } else { "skip".yellow() }, $($arg)*)).await;
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
    pub async fn add_user(&mut self, uid: String, mut user: User) -> Result<()> {
        let hid = user.vars.get("hid").cloned();
        if let Some(hid) = &hid {
            let hid = hid.to_string();
            let dev = match self.devices.get_mut(&hid) {
                Some(dev) => dev,
                None => {
                    let dev = Device::new(DeviceInfo::detect(&user, user.os()).await?);
                    self.devices.insert(hid.clone(), dev);
                    self.devices.get_mut(&hid).unwrap()
                }
            };
            match user.vars.get_mut("os") {
                Some(os) => {
                    let os2: os2::Os = os.as_str().into();
                    if dev.info.os.compatible(&os2) && dev.info.os != os2 {
                        *os = dev.info.os.to_string();
                    }
                }
                None => {
                    user.vars.insert("os".to_string(), dev.info.os.to_string());
                }
            }
            if user.is_system {
                dev.system = Some(uid.clone());
            } else {
                dev.users.push(uid.clone());
            }
        };
        action!(
            self,
            true,
            "add user {}, hid: {}, os: {}",
            uid,
            hid.unwrap_or_default(),
            user.os()
        );
        self.users.insert(uid, user);
        Ok(())
    }
    pub async fn pty(&self, uid: impl AsRef<str>, script: Script<'_, '_>) -> Result<i32> {
        let user = self.get_user(uid)?;
        let pp = user
            .pty(script, self.interactor.window_size().await)
            .await?;
        Ok(self.interactor.ask(pp).await?)
    }
}
