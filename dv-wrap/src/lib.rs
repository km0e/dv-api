mod cache;
pub use cache::SqliteCache;
mod context;
pub use context::Context;
mod interactor;
pub use interactor::TermInteractor;
mod dv;

mod user;
pub use user::User;

pub mod ops;

mod device;
pub use device::DeviceInfo;

mod utils;
mod error {

    #[derive(thiserror::Error, Debug)]
    pub enum Error {
        #[error("dv-api error: {0}")]
        DvApi(#[from] dv_api::error::ErrorChain),
        #[error("sqlite error: {0}")]
        Sqlite(#[from] rusqlite::Error),
        #[error("io error: {0}")]
        IO(#[from] std::io::Error),
        #[error("toml error: {0}")]
        Toml(#[from] toml::de::Error),
        #[error("unknown error: {0}")]
        Unknown(String),
    }

    impl Error {
        pub fn unknown<T: std::fmt::Display>(msg: T) -> Self {
            Self::Unknown(msg.to_string())
        }
    }

    resplus::define!(
        rusqlite::Error,
        dv_api::error::ErrorChain,
        std::io::Error,
        toml::de::Error,
        Error
    );
    impl ErrorChain {
        pub fn is_not_found(&self) -> bool {
            if let Error::IO(ref e) = self.0.source {
                e.kind() == std::io::ErrorKind::NotFound
            } else if let Error::DvApi(ref e) = self.0.source {
                e.is_not_found()
            } else {
                false
            }
        }
    }
    pub type Result<T, E = ErrorChain> = std::result::Result<T, E>;
}

mod dev {
    pub use super::error::Result;
    pub use super::user::User;
    pub use dv_api::fs::*;
    pub use dv_api::process::*;
}
