use super::dev::*;

pub async fn write(
    ctx: &Context,
    uid: impl AsRef<str>,
    path: impl AsRef<U8Path>,
    content: impl AsRef<str>,
) -> Result<bool> {
    let uid = uid.as_ref();
    let content = content.as_ref();
    let user = ctx.get_user(uid)?;
    if !ctx.dry_run {
        use tokio::io::AsyncWriteExt;
        user.open(path, OpenFlags::WRITE | OpenFlags::CREATE)
            .await?
            .write_all(content.as_bytes())
            .await?;
    }
    action!(ctx, true, "write to {}", uid);
    Ok(true)
}

pub async fn read(ctx: &Context, uid: impl AsRef<str>, path: impl AsRef<U8Path>) -> Result<String> {
    let uid = uid.as_ref();
    let path = path.as_ref();
    let user = ctx.get_user(uid)?;
    let mut content = String::new();
    if !ctx.dry_run {
        use tokio::io::AsyncReadExt;
        user.open(path, OpenFlags::READ)
            .await?
            .read_to_string(&mut content)
            .await?;
    }
    action!(ctx, true, "read from {}", uid);
    Ok(content)
}
