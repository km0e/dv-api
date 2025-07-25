use std::fmt::Debug;

use crate::{Result, fs::*};
use e4pty::prelude::*;

pub struct Output {
    pub code: i32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

#[async_trait::async_trait]
pub trait UserImpl {
    //TODO:better path handling
    async fn file_attributes(&self, path: &U8Path) -> Result<(U8PathBuf, Option<FileAttributes>)>;
    async fn exist(&self, path: &U8Path) -> Result<bool>;
    async fn glob_file_meta(&self, path: &U8Path) -> Result<Vec<Metadata>>;
    async fn open(&self, path: &str, flags: OpenFlags, attr: FileAttributes) -> Result<BoxedFile>;
    async fn exec(&self, command: Script<'_, '_>) -> Result<Output>;
    async fn pty(&self, command: Script<'_, '_>, win_size: WindowSize) -> Result<BoxedPty>;
}

pub type BoxedUser = Box<dyn UserImpl + Send + Sync>;

impl Debug for BoxedUser {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BoxedUser").finish()
    }
}

macro_rules! into_boxed_user {
    ($t:ty) => {
        impl From<$t> for BoxedUser {
            fn from(value: $t) -> Self {
                Box::new(value)
            }
        }
    };
    ($t:ty, $($tail:tt)*) => {
        into_boxed_user!($t);
        into_boxed_user!($($tail)*);
    };
}

pub(crate) use into_boxed_user;
