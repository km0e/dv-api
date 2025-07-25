use tokio::io::{AsyncRead, AsyncWrite};

pub trait AsyncStream: AsyncRead + AsyncWrite {}

impl<T: AsyncRead + AsyncWrite> AsyncStream for T {}
