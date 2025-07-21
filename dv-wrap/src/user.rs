use dv_api::{
    core::{BoxedUser, Output, VARIABLE_RE},
    multi::{Config, create_local, create_ssh},
    whatever,
};
use os2::Os;
use std::{borrow::Cow, collections::HashMap};

use tracing::debug;

use super::dev::*;

#[derive(Debug)]
pub struct User {
    pub is_system: bool,
    pub vars: HashMap<String, String>,
    inner: BoxedUser,
}

impl User {
    pub async fn new(mut cfg: Config) -> Result<Self> {
        let inner = if let Some(host) = cfg.remove("host") {
            create_ssh(host, &mut cfg).await
        } else {
            create_local(&mut cfg).await
        }?;
        debug!("after create user: {:?}", cfg);
        Ok(Self {
            is_system: cfg.is_system.expect("is_system"),
            vars: cfg.variables,
            inner,
        })
    }
    fn normalize<'a>(&self, path: impl Into<&'a U8Path>) -> Cow<'a, U8Path> {
        let path: &'a U8Path = path.into();
        if path.has_root() {
            Cow::Borrowed(path)
        } else {
            if VARIABLE_RE
                .captures(path.as_str())
                .is_some_and(|c| c.get(0).unwrap().start() == 0)
            {
                //TODO: replace variables
                return Cow::Borrowed(path);
            }
            match (path.starts_with("~"), self.vars.get("mount")) {
                (false, Some(mount)) => {
                    U8PathBuf::from(format!("{}/{}", mount.as_str(), path.as_str())).into()
                }
                _ => path.into(),
            }
        }
    }
    pub fn os(&self) -> Os {
        self.vars["os"].as_str().into()
    }
    pub async fn exist(&self, path: &U8Path) -> Result<bool> {
        let path = self.normalize(path);
        debug!("exist:{}", path);
        Ok(self.inner.exist(&path).await?)
    }
    pub async fn check_file(&self, path: &U8Path) -> (U8PathBuf, dv_api::Result<FileAttributes>) {
        let path = self.normalize(path);
        debug!("check_file:{}", path);
        self.inner.file_attributes(&path).await
    }
    pub async fn get_mtime(&self, path: &U8Path) -> dv_api::Result<Option<i64>> {
        let (path, fa) = self.check_file(path).await;
        match fa {
            Ok(fa) => {
                let ts = match fa.mtime {
                    Some(time) => time as i64,
                    None => whatever!("{path} mtime"),
                };
                Ok(Some(ts))
            }
            Err(e) if e.is_not_found() => Ok(None),
            Err(e) => Err(e),
        }
    }
    pub async fn check_path<'a, 'b: 'a>(&'b self, path: &'a str) -> Result<CheckInfo> {
        let path = self.normalize(path);
        let (path, fa) = self.inner.file_attributes(&path).await;
        debug!("check_path:{}", path);
        let attr = fa?;
        let info = if attr.is_dir() {
            let files = self.inner.glob_file_meta(&path).await?;
            CheckInfo::Dir(DirInfo { path, files })
        } else {
            CheckInfo::File(Metadata { path, attr })
        };
        Ok(info)
    }
    pub async fn check_dir(&self, path: &str) -> Result<DirInfo> {
        let path = self.normalize(path);
        let (path, fa) = self.inner.file_attributes(&path).await;
        let fa = fa?;
        if !fa.is_dir() {
            whatever!("{} not a directory", path);
        }
        let metadata = self.inner.glob_file_meta(&path).await?;
        Ok(DirInfo {
            path,
            files: metadata,
        })
    }
    pub async fn auto(&self, name: &str, action: &str, args: Option<&str>) -> Result<()> {
        Ok(self.inner.auto(name, action, args).await?)
    }
    pub async fn pty(&self, s: Script<'_, '_>, win_size: WindowSize) -> Result<BoxedPty> {
        Ok(self.inner.pty(s, win_size).await?)
    }
    pub async fn exec(&self, s: Script<'_, '_>) -> dv_api::Result<Output> {
        self.inner.exec(s).await
    }
    pub async fn open(&self, path: &U8Path, opt: OpenFlags) -> Result<BoxedFile> {
        self.open_with_attr(path, opt, FileAttributes::default())
            .await
    }
    pub async fn open_with_attr(
        &self,
        path: &U8Path,
        flags: OpenFlags,
        attr: FileAttributes,
    ) -> Result<BoxedFile> {
        let path = self.normalize(path);
        Ok(self.inner.open(path.as_str(), flags, attr).await?)
    }
}
