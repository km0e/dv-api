#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("io error: {0}")]
    IO(#[from] std::io::Error),
    #[cfg(not(windows))]
    #[error("openpty error: {0}")]
    Errno(#[from] rustix_openpty::rustix::io::Errno),
    #[cfg(windows)]
    #[error("openpty error: {0}")]
    Windows(#[from] windows::core::Error),
    #[error("unknown error: {0}")]
    Unknown(String),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
