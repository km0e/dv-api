use super::dev::*;

#[derive(Default)]
pub struct Manjaro {
    systemd: super::Systemd,
}

#[async_trait]
impl<U: UserImpl + Send + Sync> CommandUtil<U> for Manjaro {
    async fn setup(&self, user: &U, name: &str) -> Result<i32> {
        self.systemd.setup(user, name).await
    }
    async fn reload(&self, user: &U, name: &str) -> Result<i32> {
        self.systemd.reload(user, name).await
    }
    //file utils
    async fn copy(&self, dev: &U, src_path: &str, dst_user: &str, dst_path: &str) -> Result<i32> {
        let ec = dev
            .exec(["cp", src_path, dst_path].as_ref().into())
            .wait()
            .await?;
        if dst_user.is_empty() || ec != 0 {
            return Ok(ec);
        }
        dev.exec(["chown", dst_user, dst_path].as_ref().into())
            .wait()
            .await
    }
}
