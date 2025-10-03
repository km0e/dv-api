use super::dev::*;

pub struct Once<'b, T: AsRefContext> {
    pub ctx: T,
    pub id: &'b str,
    pub key: &'b str,
}

impl<'b, T: AsRefContext> Once<'b, T> {
    pub fn new(ctx: T, id: &'b str, key: &'b str) -> Self {
        Self { ctx, id, key }
    }
    pub async fn test(&self) -> Result<bool> {
        Ok(self.ctx.as_ref().db.get(self.id, self.key).await?.is_none())
    }
    pub async fn execute(&self) -> Result<()> {
        let ctx = self.ctx.as_ref();
        ctx.db.set(self.id, self.key, "", "").await?;
        Ok(())
    }
}
