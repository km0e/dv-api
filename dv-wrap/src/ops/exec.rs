use super::dev::*;

pub async fn exec(
    ctx: &Context,
    uid: impl AsRef<str>,
    script: impl AsRef<str>,
    reply: bool,
    executor: Option<ScriptExecutor>,
) -> Result<bool> {
    let uid = uid.as_ref();
    let commands = script.as_ref();
    let user = ctx.get_user(uid)?;
    if !ctx.dry_run {
        let script = executor.map_or_else(
            || Script::Whole(commands),
            |executor| Script::Script {
                executor,
                input: commands,
            },
        );
        if reply {
            let pp = user.pty(script, ctx.interactor.window_size().await).await?;

            let ec = ctx.interactor.ask(pp).await?;
            if ec != 0 {
                let msg = format!("unexpected exit code: {ec}");
                ctx.interactor.log(msg.clone()).await;
                whatever!("exec error: {}", msg);
            }
        } else {
            let output = user.exec(script).output().await?;
            ctx.interactor.log(output).await;
        }
    }
    action!(ctx, true, "run {}", commands);
    Ok(true)
}
