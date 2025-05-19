use super::dev::*;
mod dev {
    pub use super::super::dev::*;
    pub use super::{BoxedCommandUtil, CommandUtil};
}

use crate::whatever;
use mock::MockCommandUtil;
mod linux;
mod mock;

#[async_trait]
pub trait CommandUtil<U: UserImpl + Send + Sync> {
    //auto
    async fn setup(&self, _user: &U, _name: &str) -> Result<i32> {
        whatever!("setup command unimplemented")
    }
    async fn reload(&self, _user: &U, _name: &str) -> Result<i32> {
        whatever!("reload command unimplemented")
    }
    //file utils
    async fn copy(
        &self,
        _user: &U,
        _src_path: &str,
        _dst_name: &str,
        _dst_path: &str,
    ) -> Result<i32> {
        whatever!("copy command unimplemented")
    }
}

pub type BoxedCommandUtil<U> = Box<dyn CommandUtil<U> + Send + Sync>;

macro_rules! into_boxed_command_util {
    ($t:ty, $($tail:tt)*) => {
        into_boxed_command_util!($t);
        into_boxed_command_util!($($tail)*);
    };
    ($t:ty) => {
        impl<U: UserImpl + Send + Sync> From<$t> for BoxedCommandUtil<U> {
            fn from(value: $t) -> Self {
                Box::new(value)
            }
        }
    };
}
pub(crate) use into_boxed_command_util;

impl<U: UserImpl + Send + Sync> From<&Os> for BoxedCommandUtil<U> {
    fn from(value: &Os) -> Self {
        match &value {
            Os::Linux(os) => linux::try_match(os).unwrap_or_else(|| MockCommandUtil {}.into()),
            _ => MockCommandUtil {}.into(),
        }
    }
}
