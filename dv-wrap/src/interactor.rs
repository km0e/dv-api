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

// enum Request {
//     Ask(Ask),
//     Log(String),
//     Confirm(Confirm),
// }

// struct Ui {
//     channel: mpsc::Receiver<Request>,
// }
//
// impl Ui {
//     async fn run(mut self) {
//         loop {
//             match self.channel.recv().await {
//                 Some(Request::Ask(a)) => {
//                     if let Err(e) = a.exec().await {
//                         warn!("sync stdin failed: {}", e);
//                     }
//                 }
//                 Some(Request::Log(msg)) => {
//                     println!("{msg}");
//                 }
//                 Some(Request::Confirm(c)) => {
//                     if let Err(e) = c.exec().await {
//                         warn!("confirm failed: {}", e);
//                     }
//                 }
//                 Option::None => {
//                     break;
//                 }
//             }
//         }
//     }
// }
//
// struct Ask(BoxedPty);
//
// impl Ask {
//     async fn exec(self) -> Result<()> {
//         let _guard = RawModeGuard::new()?;
//
//         let mut stdin = tokio_fd::AsyncFd::try_from(0)?;
//         let mut stdout = tokio_fd::AsyncFd::try_from(1)?;
//         let mut buf = vec![0; 1024];
//         let mut stdin_closed = false;
//
//         loop {
//             // Handle one of the possible events:
//             tokio::select! {
//                 // There's terminal input available from the user
//                 r = stdin.read(&mut buf), if !stdin_closed => {
//                     match r {
//                         Ok(0) => {
//                             stdin_closed = true;
//                             writer.eof().await?;
//                         },
//                         // Send it to the server
//                         // Ok(n) => channel.data(&buf[..n]).await?,
//                         Ok(n) => writer.write_all(&buf[..n]).await?,
//                         Err(e) => return Err(e.into()),
//                     };
//                 },
//                 // There's an event available on the session channel
//                 Some(msg) = channel.wait() => {
//                     info!("Received message: {:?}", msg);
//                     match msg {
//                         // Write data to the terminal
//                         ChannelMsg::Data { ref data } => {
//                             stdout.write_all(data).await?;
//                             stdout.flush().await?;
//                         }
//                         // The command has returned an exit code
//                         ChannelMsg::ExitStatus { exit_status } => {
//                             code = exit_status;
//                             if !stdin_closed {
//                                 channel.eof().await?;
//                             }
//                             break;
//                         }
//                         _ => {}
//                     }
//                 },
//             }
//         }
//
//         debug!("start to sync stdin to pty");
//         let h = tokio::spawn(async move {
//             let mut buf = [0; 1024];
//             let mut to = std::io::stdout();
//             loop {
//                 trace!("waiting for data from pty");
//                 let n = reader.read(&mut buf).await?;
//                 if n == 0 {
//                     break;
//                 }
//                 debug!("read {} bytes from pty", n);
//                 to.write_all(&buf[..n])?;
//                 to.flush()?;
//             }
//             Ok::<_, std::io::Error>(())
//         });
//         tokio::time::sleep(Duration::from_millis(100)).await;
//
//         let mut key_buf = [0u8; 4];
//         while exit.try_wait().is_none() {
//             if !event::poll(Duration::from_millis(100))? {
//                 continue;
//             }
//             let ev = event::read()?;
//             if let Event::Key(KeyEvent {
//                 code,
//                 modifiers,
//                 kind: KeyEventKind::Press,
//                 ..
//             }) = ev
//             {
//                 let bytes: &[u8] = match (modifiers, code) {
//                     (KeyModifiers::CONTROL, KeyCode::Char('c')) => "\x03".as_bytes(),
//                     (KeyModifiers::CONTROL, KeyCode::Char('d')) => "\x04".as_bytes(),
//                     (_, KeyCode::Left) => "\x1b[D".as_bytes(),
//                     (_, KeyCode::Right) => "\x1b[C".as_bytes(),
//                     (_, KeyCode::Up) => "\x1b[A".as_bytes(),
//                     (_, KeyCode::Down) => "\x1b[B".as_bytes(),
//                     (_, KeyCode::Char(c)) => {
//                         key_buf[0] = c as u8;
//                         &key_buf[..1]
//                     }
//                     (_, KeyCode::Backspace) => "\x7f".as_bytes(),
//                     (_, KeyCode::Enter) => "\r".as_bytes(),
//                     (_, KeyCode::Esc) => "\x1b".as_bytes(),
//                     _ => continue, //TODO:handle other keys
//                 };
//                 writer.write_all(bytes).await?;
//             } else if let Event::Resize(cols, rows) = ev {
//                 writer.window_change(cols, rows).await?;
//             }
//         }
//         h.abort();
//         info!("ask interactor stopped");
//         Ok(())
//     }
// }
//
// struct Confirm {
//     msg: String,
//     opts: Vec<(char, String)>,
//     sel: Oneshot<usize>,
// }
//
// impl Confirm {
//     async fn exec(self) -> Result<()> {
//         println!("{}", self.msg);
//         print!("opts [");
//         for opt in &self.opts {
//             print!("{}: {}, ", opt.0, opt.1);
//         }
//         print!("]:");
//         let mut stdout = std::io::stdout();
//         stdout.flush()?;
//         let _guard = RawModeGuard::new()?;
//         let mut hash = self
//             .opts
//             .into_iter()
//             .enumerate()
//             .map(|(i, (c, hint))| (c, (i, hint)))
//             .collect::<HashMap<_, _>>();
//         hash.reserve(0);
//         loop {
//             if !event::poll(Duration::from_millis(100))? {
//                 continue;
//             }
//             let ev = event::read()?;
//             if let Event::Key(KeyEvent {
//                 code,
//                 modifiers: KeyModifiers::NONE,
//                 kind: KeyEventKind::Press,
//                 ..
//             }) = ev
//             {
//                 let KeyCode::Char(c) = code else {
//                     continue;
//                 };
//                 debug!("read key {}", c);
//                 if let Some((i, hint)) = hash.remove(&c) {
//                     drop(_guard); //NOTE:MoveToNextLine is not working in raw mode?
//                     println!("{hint}");
//                     self.sel.send(i);
//                     return Ok(());
//                 }
//             }
//         }
//     }
// }

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
