use async_trait::async_trait;
pub use e4pty::prelude::*;

use crate::Result;
use crate::core::Output;

#[async_trait]
pub trait PtyProcessConsumer {
    async fn wait(self) -> Result<i32>;
    async fn output(self) -> Result<String>;
}

#[async_trait]
impl<T: Future<Output = Result<Output>> + Send> PtyProcessConsumer for T {
    async fn wait(self) -> Result<i32> {
        let es = self.await?.code;
        Ok(es)
    }
    async fn output(self) -> Result<String> {
        let stdout = self.await?.stdout;
        Ok(String::from_utf8_lossy(&stdout).to_string())
    }
}
