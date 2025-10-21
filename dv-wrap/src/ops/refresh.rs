use super::dev::*;

pub async fn refresh(ctx: &Context, id: impl AsRef<str>, key: impl AsRef<str>) -> Result<()> {
    let (id, key) = (id.as_ref(), key.as_ref());
    ctx.db.del(id, key).await?;
    Ok(())
}
