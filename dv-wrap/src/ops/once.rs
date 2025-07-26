use super::dev::*;

pub struct Once<'a, 'b> {
    pub ctx: &'a Context,
    pub id: &'b str,
    pub key: &'b str,
}

impl<'a, 'b> Once<'a, 'b> {
    pub fn new(ctx: &'a Context, id: &'b str, key: &'b str) -> Self {
        Self { ctx, id, key }
    }
    pub async fn test(&self) -> Result<bool> {
        Ok(self.ctx.cache.get(self.id, self.key).await?.is_none())
    }
    pub async fn set(&self) -> Result<()> {
        if !self.ctx.dry_run {
            self.ctx.cache.set(self.id, self.key, 0, 0).await?;
            action!(self.ctx, true, "once {} {}", self.id, self.key);
        }
        Ok(())
    }
}
