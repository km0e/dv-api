use super::dev::*;

pub async fn exec(
    ctx: &Context,
    uid: impl AsRef<str>,
    shell: Option<&str>,
    commands: impl AsRef<str>,
) -> Result<bool> {
    let uid = uid.as_ref();
    let commands = commands.as_ref();
    let script = shell
        .map(|_sh| Script::sh(Box::new([commands].into_iter())))
        .unwrap_or_else(|| Script::Whole(commands));
    let user = ctx.get_user(uid)?;
    if !ctx.dry_run {
        let pp = user.pty(script, ctx.interactor.window_size().await).await?;

        let ec = ctx.interactor.ask(pp).await?;
        if ec != 0 {
            let msg = format!("unexpected exit code: {ec}");
            ctx.interactor.log(msg.clone()).await;
            whatever!("exec error: {}", msg);
        }
    }
    action!(ctx, true, "run {}", commands);
    Ok(true)
}
