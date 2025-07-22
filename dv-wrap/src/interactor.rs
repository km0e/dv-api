use std::{collections::HashMap, io::Write, time::Duration};

use super::dev::*;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use dv_api::Result;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::{debug, trace};

#[derive(Debug)]
pub struct TermInteractor {}

impl TermInteractor {
    pub fn new() -> std::io::Result<Self> {
        Ok(Self {})
    }
}

#[async_trait::async_trait]
impl Interactor for TermInteractor {
    async fn window_size(&self) -> WindowSize {
        let (cols, rows) = crossterm::terminal::size().expect("try to get terminal size");
        WindowSize { cols, rows }
    }
    async fn log(&self, msg: String) {
        println!("{msg}");
    }
    async fn ask(&self, mut pty: BoxedPty) -> dv_api::Result<i32> {
        let _guard = RawModeGuard::new()?;

        let mut stdin = tokio_fd::AsyncFd::try_from(0)?;
        let mut stdout = tokio_fd::AsyncFd::try_from(1)?;
        let mut buf = vec![0; 1024];
        let mut buf2 = vec![0; 1024];
        let mut stdin_closed = false;
        let mut pty_stdin_closed = false;

        loop {
            // Handle one of the possible events:
            tokio::select! {
                // There's terminal input available from the user
                r = stdin.read(&mut buf), if !stdin_closed => {
                    match r {
                        Ok(0) => {
                            stdin_closed = true;
                            pty.writer.eof().await?;
                        },
                        // Send it to the server
                        // Ok(n) => channel.data(&buf[..n]).await?,
                        Ok(n) => pty.writer.write_all(&buf[..n]).await?,
                        Err(e) => return Err(e.into()),
                    };
                },
                // There's an event available on the session channel
                w = pty.reader.read(&mut buf2) ,if !pty_stdin_closed => {
                    match w {
                        Ok(0) => {
                            pty_stdin_closed = true;
                        },
                        Ok(n) => {
                            debug!("read {} bytes from pty", n);
                            stdout.write_all(&buf2[..n]).await?;
                            stdout.flush().await?;
                        },
                        Err(e) => return Err(e.into()),
                    };
                },
                ec = pty.ctl.wait() => {
                    if !stdin_closed {
                        pty.writer.eof().await?;
                    }
                    return Ok(ec?);
                },
            }
        }
    }
    async fn confirm(&self, msg: String, opts: &[&str]) -> Result<usize> {
        let opts = opts
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let (c, s) = s
                    .split_once('/')
                    .and_then(|(c, s)| c.chars().next().map(|c| (c, s)))
                    .unwrap_or((char::from_digit(i as u32 + 1, 10).unwrap(), s));
                (c, s.to_string())
            })
            .collect::<Vec<_>>();

        trace!("start to send confirm request");
        println!("{}", msg);
        print!("opts [");
        for opt in &opts {
            print!("{}: {}, ", opt.0, opt.1);
        }
        print!("]:");
        let mut stdout = std::io::stdout();
        stdout.flush()?;
        let _guard = RawModeGuard::new()?;
        let mut hash = opts
            .into_iter()
            .enumerate()
            .map(|(i, (c, hint))| (c, (i, hint)))
            .collect::<HashMap<_, _>>();
        hash.reserve(0);
        loop {
            if !event::poll(Duration::from_millis(100))? {
                continue;
            }
            let ev = event::read()?;
            if let Event::Key(KeyEvent {
                code,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            }) = ev
            {
                let KeyCode::Char(c) = code else {
                    continue;
                };
                debug!("read key {}", c);
                if let Some((i, hint)) = hash.remove(&c) {
                    drop(_guard); //NOTE:MoveToNextLine is not working in raw mode?
                    println!("{hint}");
                    return Ok(i);
                }
            }
        }
    }
}

struct RawModeGuard;
impl RawModeGuard {
    fn new() -> std::io::Result<Self> {
        enable_raw_mode()?;
        Ok(Self)
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        disable_raw_mode().expect("disable raw mode");
    }
}
