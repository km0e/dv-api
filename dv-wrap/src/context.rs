use super::dev::*;
use crate::{
    DeviceInfo,
    db::MultiDB,
    interactor::{DynInteractor, Interactor},
};
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
    pub db: MultiDB,
    pub interactor: Box<DynInteractor>,
    pub users: HashMap<String, User>,
    pub devices: HashMap<String, Device>,
    pub cache_dir: Option<std::path::PathBuf>,
}

impl Context {
    pub fn new(
        db: MultiDB,
        cache_dir: Option<std::path::PathBuf>,
        interactor: impl Interactor + Sync + 'static,
    ) -> Self {
        Self {
            db,
            interactor: Box::new(interactor),
            users: HashMap::new(),
            devices: HashMap::new(),
            cache_dir,
        }
    }
    pub fn contains_user<Q>(&self, uid: &Q) -> bool
    where
        String: std::borrow::Borrow<Q>,
        Q: ?Sized + std::hash::Hash + Eq,
    {
        self.users.contains_key(uid)
    }
    pub fn get_user(&self, uid: impl AsRef<str>) -> Result<&User> {
        let uid = uid.as_ref();
        match self.users.get(uid) {
            Some(user) => Ok(user),
            None => {
                anyhow::bail!("user {} not found", uid)
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
        self.users.insert(uid, user);
        Ok(())
    }
    pub async fn pty(&self, uid: impl AsRef<str>, script: Script<'_, '_>) -> Result<i32> {
        let user = self.get_user(uid)?;
        let pp = user
            .pty(script, self.interactor.window_size().await)
            .await?;
        self.interactor.ask(pp).await
    }
}

pub trait AsRefContext {
    fn as_ref(&self) -> impl std::ops::Deref<Target = Context> + '_;
}

impl AsRefContext for Context {
    fn as_ref(&self) -> impl std::ops::Deref<Target = Context> + '_ {
        self
    }
}

impl AsRefContext for &Context {
    fn as_ref(&self) -> impl std::ops::Deref<Target = Context> + '_ {
        *self
    }
}
