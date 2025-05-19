use resplus::attach;
use std::{borrow::Cow, collections::HashMap, sync::Arc};

use tracing::debug;

mod dev {
    pub use super::super::core::*;
    pub use crate::{Result, fs::*, util::*};
    pub use async_trait::async_trait;
    pub use e4pty::prelude::*;
    pub use utf8path2::*;
}

use dev::*;

mod config;
pub use config::Config;

use crate::{process::DynInteractor, whatever};
mod multi;

// #[derive(Debug)]
// pub struct User {
//     pub variables: HashMap<String, String>,
//     pub is_system: bool,
//     inner: BoxedUser,
//     pub dev: Arc<Dev>,
// }
//
// impl User {
//     pub async fn new(
//         variables: HashMap<String, String>,
//         is_system: bool,
//         inner: BoxedUser,
//         dev: Arc<Dev>,
//     ) -> Result<Self> {
//         Ok(Self {
//             variables,
//             is_system,
//             inner,
//             dev,
//         })
//     }
//     fn normalize<'a>(&self, path: impl Into<&'a U8Path>) -> Cow<'a, U8Path> {
//         let path: &'a U8Path = path.into();
//         let path = if path.has_root() {
//             Cow::Borrowed(path)
//         } else {
//             if crate::core::VARIABLE_RE
//                 .captures(path.as_str())
//                 .is_some_and(|c| c.get(0).unwrap().start() == 0)
//             {
//                 //TODO: replace variables
//                 return Cow::Borrowed(path);
//             }
//             let path = match (path.starts_with("~"), self.variables.get("mount")) {
//                 (false, Some(mount)) => {
//                     U8PathBuf::from(format!("{}/{}", mount.as_str(), path.as_str())).into()
//                 }
//                 _ => path.into(),
//             };
//             path
//         };
//         path
//     }
//     pub async fn exist(&self, path: &U8Path) -> Result<bool> {
//         let path = self.normalize(path);
//         debug!("exist:{}", path);
//         self.inner.exist(&path).await
//     }
//     pub async fn check_file(&self, path: &U8Path) -> (U8PathBuf, Result<FileAttributes>) {
//         let path = self.normalize(path);
//         debug!("check_file:{}", path);
//         self.inner.file_attributes(&path).await
//     }
//     pub async fn get_mtime(&self, path: &U8Path) -> Result<Option<i64>> {
//         let (path, fa) = self.check_file(path).await;
//         match fa {
//             Ok(fa) => {
//                 let ts = match fa.mtime {
//                     Some(time) => time as i64,
//                     None => whatever!("{path} mtime"),
//                 };
//                 Ok(Some(ts))
//             }
//             Err(e) if e.is_not_found() => Ok(None),
//             Err(e) => Err(e),
//         }
//     }
//     pub async fn check_path<'a, 'b: 'a>(&'b self, path: &'a str) -> Result<CheckInfo> {
//         let path = self.normalize(path);
//         let (path, fa) = self.inner.file_attributes(&path).await;
//         debug!("check_path:{}", path);
//         let attr = fa?;
//         let info = if attr.is_dir() {
//             let files = self.inner.glob_file_meta(&path).await?;
//             CheckInfo::Dir(DirInfo { path, files })
//         } else {
//             CheckInfo::File(Metadata { path, attr })
//         };
//         Ok(info)
//     }
//     pub async fn check_dir(&self, path: &str) -> Result<DirInfo> {
//         let path = self.normalize(path);
//         let (path, fa) = self.inner.file_attributes(&path).await;
//         let fa = fa?;
//         if !fa.is_dir() {
//             whatever!("{} not a directory", path);
//         }
//         let metadata = self.inner.glob_file_meta(&path).await?;
//         Ok(DirInfo {
//             path,
//             files: metadata,
//         })
//     }
//     pub async fn auto(&self, name: &str, action: &str, args: Option<&str>) -> Result<()> {
//         self.inner.auto(name, action, args).await
//     }
//     pub async fn app(&self, interactor: &DynInteractor, packages: Package<'_>) -> Result<bool> {
//         packages.install(self, interactor, &self.dev.pm).await
//     }
//     pub async fn pty(&self, s: Script<'_, '_>, win_size: WindowSize) -> Result<BoxedPty> {
//         self.inner.pty(s, win_size).await
//     }
//     pub async fn exec(&self, s: Script<'_, '_>) -> Result<Output> {
//         self.inner.exec(s).await
//     }
//     pub async fn open(&self, path: &U8Path, opt: OpenFlags) -> Result<BoxedFile> {
//         self.open_with_attr(path, opt, FileAttributes::default())
//             .await
//     }
//     pub async fn open_with_attr(
//         &self,
//         path: &U8Path,
//         flags: OpenFlags,
//         attr: FileAttributes,
//     ) -> Result<BoxedFile> {
//         let path = self.normalize(path);
//         attach!(self.inner.open(path.as_str(), flags, attr), 0).await
//     }
// }
