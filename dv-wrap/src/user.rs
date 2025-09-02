use dv_api::{
    core::{BoxedUser, Output},
    multi::{Config, create_local, create_ssh},
    whatever,
};
use os2::Os;
use std::{borrow::Cow, collections::HashMap};

use tracing::debug;

use crate::utils::var_replace;

use super::dev::*;

#[derive(Debug)]
pub struct User {
    pub is_system: bool,
    pub vars: HashMap<String, String>,
    inner: BoxedUser,
}

impl User {
    pub async fn local(mut cfg: Config) -> Result<Self> {
        let inner = create_local(&mut cfg).await?;
        Ok(Self {
            is_system: cfg.is_system.expect("is_system"),
            vars: cfg.variables,
            inner,
        })
    }
    pub async fn ssh(mut cfg: Config) -> Result<Self> {
        let Some(host) = cfg.remove("host") else {
            whatever!("ssh user must have host")
        };
        let inner = create_ssh(host, &mut cfg).await?;
        Ok(Self {
            is_system: cfg.is_system.expect("is_system"),
            vars: cfg.variables,
            inner,
        })
    }
    fn normalize<'a>(&self, path: impl Into<&'a U8Path>) -> Result<Cow<'a, U8Path>> {
        let path: &'a U8Path = path.into();
        let Some(path) = var_replace(path.as_str(), &self.vars) else {
            whatever!("var_replace failed: {}", path)
        };
        let path: Cow<U8Path> = match path {
            Cow::Borrowed(path) => U8Path::new(path).into(),
            Cow::Owned(path) => U8PathBuf::from(path).into(),
        };
        Ok(
            if !path.has_root()
                && let (false, Some(mount)) = (path.starts_with("~"), self.vars.get("mount"))
            {
                U8PathBuf::from(format!("{}/{}", mount.as_str(), path.as_ref())).into()
            } else {
                path
            },
        )
    }
    pub fn os(&self) -> Os {
        self.vars["os"].as_str().into()
    }
    pub async fn exist(&self, path: &U8Path) -> Result<bool> {
        let path = self.normalize(path)?;
        debug!("exist:{}", path);
        Ok(self.inner.file_attributes(&path).await?.1.is_some())
    }
    pub async fn file_attributes(
        &self,
        path: &U8Path,
    ) -> Result<(U8PathBuf, Option<FileAttributes>)> {
        let path = self.normalize(path)?;
        debug!("check_file:{}", path);
        Ok(self.inner.file_attributes(&path).await?)
    }
    pub async fn get_mtime(&self, path: &U8Path) -> Result<Option<i64>> {
        let (path, fa) = self.file_attributes(path).await?;
        match fa {
            None => Ok(None),
            Some(FileAttributes {
                mtime: Some(time), ..
            }) => Ok(Some(time as i64)),
            _ => {
                whatever!("{path} mtime")
            }
        }
    }
    pub async fn check_path(&self, path: &str) -> Result<CheckInfo> {
        let path: &U8Path = path.into();
        let (path, fa) = self.file_attributes(path).await?;
        debug!("check_path:{}", path);
        let Some(attr) = fa else {
            whatever!("{} not found", path)
        };
        let info = if attr.is_dir() {
            let files = self.inner.glob_file_meta(&path).await?;
            CheckInfo::Dir(DirInfo { path, files })
        } else {
            CheckInfo::File(Metadata { path, attr })
        };
        Ok(info)
    }
    pub async fn glob(&self, path: &U8Path) -> Result<Vec<Metadata>> {
        Ok(self.inner.glob_file_meta(path).await?)
    }
    pub async fn rm(&self, path: &U8Path) -> Result<()> {
        let path = self.normalize(path)?;
        debug!("rm:{}", path);
        self.inner.rm(&path).await?;
        Ok(())
    }
    pub async fn check_dir(&self, path: &str) -> Result<DirInfo> {
        let path: &U8Path = path.into();
        let (path, fa) = self.file_attributes(path).await?;
        debug!("check_path:{}", path);
        let Some(attr) = fa else {
            whatever!("{} not found", path)
        };
        if !attr.is_dir() {
            whatever!("{} not a directory", path);
        }
        let metadata = self.inner.glob_file_meta(&path).await?;
        Ok(DirInfo {
            path,
            files: metadata,
        })
    }
    pub async fn pty(&self, s: Script<'_, '_>, win_size: WindowSize) -> Result<BoxedPty> {
        Ok(self.inner.pty(s, win_size).await?)
    }
    pub async fn exec(&self, s: Script<'_, '_>) -> dv_api::Result<Output> {
        self.inner.exec(s).await
    }
    pub async fn open<P: AsRef<U8Path>>(&self, path: P, opt: OpenFlags) -> Result<BoxedFile> {
        self.open_with_attr(path, opt, FileAttributes::default())
            .await
    }
    pub async fn open_with_attr<P: AsRef<U8Path>>(
        &self,
        path: P,
        flags: OpenFlags,
        attr: FileAttributes,
    ) -> Result<BoxedFile> {
        let path = self.normalize(path.as_ref())?;
        Ok(self.inner.open(path.as_ref(), flags, attr).await?)
    }
}
