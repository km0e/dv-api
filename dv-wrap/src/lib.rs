mod db;
pub use db::{MultiDB, Sqlite};

mod context;
pub use context::Context;

mod interactor;
pub use interactor::TermInteractor;

mod user;
pub use user::User;

pub mod ops;

mod device;
pub use device::DeviceInfo;

pub mod error {
    #[derive(thiserror::Error, Debug)]
    pub enum Error {
        #[error("dv-api error: {0}")]
        DvApi(#[from] dv_api::error::Error),
        #[error("io error: {0}")]
        IO(#[from] std::io::Error),
        #[error("toml error: {0}")]
        Toml(#[from] toml::de::Error),
        #[error("reqwest error: {0}")]
        Reqwest(#[from] reqwest::Error),
        #[error("unknown error: {0}")]
        Unknown(String),
    }

    impl Error {
        pub fn unknown<T: std::fmt::Display>(msg: T) -> Self {
            Self::Unknown(msg.to_string())
        }
    }

    impl Error {
        pub fn is_not_found(&self) -> bool {
            if let Error::IO(e) = self {
                e.kind() == std::io::ErrorKind::NotFound
            } else if let Error::DvApi(e) = self {
                e.is_not_found()
            } else {
                false
            }
        }
    }
    pub type Result<T, E = Error> = std::result::Result<T, E>;
}

mod dev {
    pub use super::error::Result;
    pub use super::user::User;
    pub use dv_api::fs::*;
    pub use dv_api::process::*;
}

mod utils;
