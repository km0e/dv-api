use super::dev::*;

#[derive(Default)]
pub struct Alpine {
    openrc: super::Openrc,
}

#[async_trait]
impl<U: UserImpl + Send + Sync> CommandUtil<U> for Alpine {
    async fn setup(&self, user: &U, name: &str) -> Result<i32> {
        self.openrc.setup(user, name).await
    }
    async fn reload(&self, user: &U, name: &str) -> Result<i32> {
        self.openrc.reload(user, name).await
    }
}
