use std::os::{fd::AsRawFd, unix::process::ExitStatusExt};

use async_trait::async_trait;
use rustix_openpty::rustix::termios::{self, Winsize};
use tokio::fs::File;

use crate::{core::*, error::Result};

struct PtyCtlImpl {
    child: std::process::Child,
}

#[async_trait]
impl PtyCtl for PtyCtlImpl {
    async fn wait(&mut self) -> Result<i32> {
        let ec = self.child.wait().map(|es| {
            es.code()
                .unwrap_or_else(|| es.signal().map_or(1, |v| 128 + v))
        })?;
        Ok(ec)
    }
}

#[async_trait]
impl PtyWriter for File {
    async fn window_change(&self, width: u16, height: u16) -> Result<()> {
        termios::tcsetwinsize(
            self,
            termios::Winsize {
                ws_row: height,
                ws_col: width,
                ws_xpixel: 0, // TODO: ws_xpixel:
                ws_ypixel: 0, // TODO: ws_ypixel:
            },
        )?;
        Ok(())
    }
}

impl PtyReader for File {}

pub fn openpty(window_size: WindowSize, script: Script<'_, '_>) -> std::io::Result<BoxedPty> {
    let pair = rustix_openpty::openpty(
        None,
        Some(&Winsize {
            ws_row: window_size.rows,
            ws_col: window_size.cols,
            ws_xpixel: 0,
            ws_ypixel: 0,
        }),
    )?;

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    if let Ok(mut termios) = termios::tcgetattr(&pair.controller) {
        // Set character encoding to UTF-8.
        termios.input_modes.set(termios::InputModes::IUTF8, true);
        let _ = termios::tcsetattr(&pair.controller, termios::OptionalActions::Now, &termios);
    }
    let mut builder = script.into_command()?;
    // Setup child stdin/stdout/stderr.
    builder.stdin(pair.user.try_clone()?);
    builder.stderr(pair.user.try_clone()?);
    builder.stdout(pair.user.try_clone()?);
    let stdio = pair.controller.try_clone()?;
    unsafe {
        use std::os::unix::process::CommandExt;
        builder.pre_exec(move || {
            // Create a new process group.
            #[cfg(target_os = "macos")]
            use rustix::{io, process};
            #[cfg(not(target_os = "macos"))]
            use rustix_openpty::rustix::{io, process};
            process::setsid()?;
            process::ioctl_tiocsctty(&pair.user)?;

            io::close(pair.user.as_raw_fd());
            io::close(pair.controller.as_raw_fd());
            // libc::signal(libc::SIGCHLD, libc::SIG_DFL);
            // libc::signal(libc::SIGHUP, libc::SIG_DFL);
            // libc::signal(libc::SIGINT, libc::SIG_DFL);
            // libc::signal(libc::SIGQUIT, libc::SIG_DFL);
            // libc::signal(libc::SIGTERM, libc::SIG_DFL);
            // libc::signal(libc::SIGALRM, libc::SIG_DFL);
            //
            Ok(())
        });
    }
    // TODO:set working directory
    // set signal handler

    let child = builder.spawn()?;
    use rustix_openpty::rustix::io;
    let pw = io::dup(&stdio)?;
    io::fcntl_setfd(&pw, io::fcntl_getfd(&pw)? | io::FdFlags::CLOEXEC)?;
    let pr = std::fs::File::from(stdio);
    Ok(BoxedPty::new(
        PtyCtlImpl { child },
        File::from_std(std::fs::File::from(pw)),
        File::from_std(pr),
    ))
}
