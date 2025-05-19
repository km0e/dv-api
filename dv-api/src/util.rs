use tokio::io::{AsyncRead, AsyncWrite};

// mod pm;
// pub use pm::{Package, Pm};
mod command;
mod dev {
    pub use crate::process::PtyProcessConsumer;
    pub use crate::{Result, core::*};
    pub use async_trait::async_trait;
    pub use e4pty::prelude::*;
    pub use os2::*;
}

pub use command::BoxedCommandUtil;

pub trait AsyncStream: AsyncRead + AsyncWrite {}

impl<T: AsyncRead + AsyncWrite> AsyncStream for T {}
