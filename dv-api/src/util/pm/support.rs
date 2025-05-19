use super::dev::{self, *};
use tracing::{info, trace};

pub mod apk;
pub mod apt;
pub mod pacman;
pub mod paru;
pub mod winget;
pub mod yay;

async fn install(
    u: &User,
    int: &DynInteractor,
    query_args: impl AsRef<str>,
    query_s: &str,
    pm: &str,
    args: &[&str],
) -> Result<bool> {
    use std::iter::once;
    trace!(
        "install script {} with args {}",
        query_s,
        query_args.as_ref()
    );
    let input = once(query_args.as_ref()).chain(once(query_s));
    let cmd = Script::sh(Box::new(input));
    let pkgs = u.exec(cmd).output().await?;
    if pkgs.is_empty() {
        return Ok(false);
    }
    info!("pkgs[{}] need to be installed", pkgs);
    let args = args.iter().copied();
    let pkgs = pkgs.split_whitespace();
    let s = Script::Split {
        program: pm,
        args: Box::new(args.chain(pkgs)),
    };
    let pp = u.pty(s, int.window_size().await).await?;
    let ec = int.ask(pp).await?;
    if ec != 0 {
        whatever!("unexpected exit status {}", ec);
    }
    Ok(true)
}
