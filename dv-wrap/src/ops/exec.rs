use dv_api::core::Output;

use super::dev::*;

pub async fn exec(
    ctx: &Context,
    uid: impl AsRef<str>,
    script: impl AsRef<str>,
    reply: bool,
    executor: Option<ScriptExecutor>,
) -> Result<Output> {
    let uid = uid.as_ref();
    let commands = script.as_ref();
    let user = ctx.get_user(uid)?;
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
        Ok(Output {
            code: ec,
            stdout: vec![],
            stderr: vec![],
        })
    } else {
        Ok(user.exec(script).await?)
    }
}
