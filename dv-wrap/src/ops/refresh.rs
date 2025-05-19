use super::dev::*;

pub async fn refresh(ctx: Context<'_>, id: impl AsRef<str>, key: impl AsRef<str>) -> Result<()> {
    let (id, key) = (id.as_ref(), key.as_ref());
    ctx.cache.del(id, key).await?;
    action!(ctx, true, "refresh {} {}", id, key);
    Ok(())
}
