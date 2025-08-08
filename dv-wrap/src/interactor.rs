use std::{
    collections::HashMap,
    io::{Write, stdout},
    time::Duration,
};

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
        setup_stdin_nonblock()?;
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

        let mut stdin = noblock_stdin();
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
                            stdout().write_all(&buf2[..n])?;
                            stdout().flush()?;
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

#[cfg(windows)]
fn setup_stdin_nonblock() -> std::io::Result<()> {
    Ok(())
}

#[cfg(windows)]
fn noblock_stdin() -> impl tokio::io::AsyncRead {
    use windows::Win32::{
        Storage::FileSystem::ReadFile,
        System::Console::{GetStdHandle, STD_INPUT_HANDLE},
    };

    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = [0; 1024];
        let hin = unsafe { GetStdHandle(STD_INPUT_HANDLE).unwrap() };
        loop {
            let mut bytes = 0;
            unsafe {
                ReadFile(hin, Some(&mut buf), Some(&mut bytes), None).unwrap();
            }
            if bytes == 0 {
                break;
            }
            debug!("read {} bytes from stdin", bytes);
            tx.send(buf[..bytes as usize].to_vec()).unwrap();
        }
    });
    struct AsyncStdin {
        rx: std::sync::mpsc::Receiver<Vec<u8>>,
        buffer: (Vec<u8>, usize),
    }
    impl tokio::io::AsyncRead for AsyncStdin {
        fn poll_read(
            mut self: std::pin::Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
            buf: &mut tokio::io::ReadBuf<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            debug!("poll_read");
            if self.buffer.1 == self.buffer.0.len() {
                debug!("try to read from stdin");
                match self.rx.try_recv() {
                    Ok(data) => {
                        self.buffer.0 = data;
                        self.buffer.1 = 0;
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                        cx.waker().wake_by_ref();
                        return std::task::Poll::Pending;
                    }
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        return std::task::Poll::Ready(Ok(()));
                    }
                }
            }
            let n = std::cmp::min(buf.remaining(), self.buffer.0.len() - self.buffer.1);
            buf.put_slice(&self.buffer.0[self.buffer.1..self.buffer.1 + n]);
            self.buffer.1 += n;
            debug!("sync {} bytes from stdin", n);
            std::task::Poll::Ready(Ok(()))
        }
    }
    AsyncStdin {
        rx,
        buffer: (vec![], 0),
    }
}

#[cfg(not(windows))]
fn setup_stdin_nonblock() -> std::io::Result<()> {
    use rustix::fs;
    use std::os::fd::AsFd;
    let stdin = std::io::stdin();
    let fd = stdin.as_fd();
    fs::fcntl_setfl(fd, fs::fcntl_getfl(fd)? | fs::OFlags::NONBLOCK)?;
    Ok(())
}

#[cfg(not(windows))]
fn noblock_stdin() -> impl tokio::io::AsyncRead {
    use std::io::Read;

    struct AsyncStdin;
    impl tokio::io::AsyncRead for AsyncStdin {
        fn poll_read(
            self: std::pin::Pin<&mut Self>,
            _: &mut std::task::Context<'_>,
            buf: &mut tokio::io::ReadBuf<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            let stdin = std::io::stdin();
            let mut stdin = stdin.lock();
            match stdin.read(buf.initialize_unfilled()) {
                Ok(n) => {
                    buf.advance(n);
                    std::task::Poll::Ready(Ok(()))
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => std::task::Poll::Pending,
                Err(e) => std::task::Poll::Ready(Err(e)),
            }
        }
    }
    AsyncStdin {}
}
