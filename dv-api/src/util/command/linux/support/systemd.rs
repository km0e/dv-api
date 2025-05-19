use super::dev::*;

#[derive(Default)]
pub struct Systemd {}

impl Systemd {
    pub async fn setup<U: UserImpl>(&self, user: &U, name: &str) -> crate::Result<i32> {
        user.exec(["systemctl", "enable", name].as_ref().into())
            .wait()
            .await
    }
    pub async fn reload<U: UserImpl>(&self, user: &U, name: &str) -> crate::Result<i32> {
        user.exec(["systemctl", "reload-or-restart", name].as_ref().into())
            .wait()
            .await
    }
}
