use std::{fmt::Display, io::Write, process::Command};

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};

use crate::Result;

#[derive(Debug, Clone)]
pub struct WindowSize {
    pub rows: u16,
    pub cols: u16,
}

#[async_trait]
pub trait PtyWriter: AsyncWrite {
    async fn window_change(&self, width: u16, height: u16) -> Result<()>;
    async fn eof(&self) -> Result<()>;
}

pub type BoxedPtyWriter = Box<dyn PtyWriter + Send + Sync + Unpin>;

pub trait PtyReader: AsyncRead {}

pub type BoxedPtyReader = Box<dyn PtyReader + Send + Sync + Unpin>;

#[async_trait]
pub trait PtyCtl {
    async fn wait(&mut self) -> Result<i32>;
}

pub type BoxedPtyCtl = Box<dyn PtyCtl + Send + Sync + Unpin>;

pub struct BoxedPty {
    pub ctl: BoxedPtyCtl,
    pub writer: BoxedPtyWriter,
    pub reader: BoxedPtyReader,
}

impl BoxedPty {
    pub fn new(
        ctl: impl PtyCtl + Send + Sync + Unpin + 'static,
        writer: impl PtyWriter + Send + Sync + Unpin + 'static,
        reader: impl PtyReader + Send + Sync + Unpin + 'static,
    ) -> Self {
        Self {
            ctl: Box::new(ctl),
            writer: Box::new(writer),
            reader: Box::new(reader),
        }
    }
    pub fn destruct(self) -> (BoxedPtyCtl, BoxedPtyWriter, BoxedPtyReader) {
        (self.ctl, self.writer, self.reader)
    }
}

pub enum ScriptExecutor {
    Sh,
    Powershell,
}

impl Display for ScriptExecutor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScriptExecutor::Sh => write!(f, "sh"),
            ScriptExecutor::Powershell => write!(f, "powershell"),
        }
    }
}

impl ScriptExecutor {
    pub fn prepare_clean(&self) -> Vec<u8> {
        match self {
            ScriptExecutor::Sh => b"\ntrap 'rm -f -- \"$0\"' EXIT;".to_vec(),
            ScriptExecutor::Powershell => b"\r\nRemove-Item $MyInvocation.MyCommand.Path".to_vec(),
        }
    }
}

impl AsRef<str> for ScriptExecutor {
    fn as_ref(&self) -> &str {
        match self {
            ScriptExecutor::Sh => "sh",
            ScriptExecutor::Powershell => "powershell",
        }
    }
}

pub enum Script<'a, 'b> {
    Whole(&'a str),
    Split {
        program: &'a str,
        args: Box<dyn 'b + Iterator<Item = &'a str> + Send>,
    },
    Script {
        executor: ScriptExecutor,
        input: Box<dyn 'b + Iterator<Item = &'a str> + Send>,
    },
}

impl<'a, 'b> Script<'a, 'b> {
    pub fn sh(input: Box<dyn 'b + Iterator<Item = &'a str> + Send>) -> Self {
        Script::Script {
            executor: ScriptExecutor::Sh,
            input,
        }
    }
    pub fn powershell(input: Box<dyn 'b + Iterator<Item = &'a str> + Send>) -> Self {
        Script::Script {
            executor: ScriptExecutor::Powershell,
            input,
        }
    }
    pub fn into_command(self) -> std::io::Result<Command> {
        let cmd = match self {
            Script::Whole(cmd) => {
                let mut iter = cmd.split_whitespace();
                let mut cmd = Command::new(iter.next().unwrap());
                cmd.args(iter);
                cmd
            }
            Script::Split { program, args } => {
                let mut cmd = Command::new(program);
                cmd.args(args);
                cmd
            }
            Script::Script { executor, input } => {
                let mut temp = match executor {
                    ScriptExecutor::Sh => tempfile::NamedTempFile::new(),
                    ScriptExecutor::Powershell => tempfile::NamedTempFile::with_suffix(".ps1"),
                }?;
                for line in input {
                    temp.write_all(line.as_bytes())?;
                }
                temp.write_all(executor.prepare_clean().as_slice())?;
                let path = temp.into_temp_path().keep()?;
                let mut cmd = Command::new(executor.as_ref());
                cmd.arg(path);
                cmd
            }
        };
        Ok(cmd)
    }
}

impl<'a, 'b> From<&'b [&'a str]> for Script<'a, 'b> {
    fn from(args: &'b [&'a str]) -> Self {
        Script::Split {
            program: args[0],
            args: Box::new(args.iter().skip(1).copied()),
        }
    }
}

impl<'a> From<&'a str> for Script<'a, 'a> {
    fn from(program: &'a str) -> Self {
        Script::Whole(program)
    }
}

impl<'a, 'b> Script<'a, 'b> {
    pub fn new<I>(program: &'a str, args: I) -> Self
    where
        I: IntoIterator<Item = &'a str> + 'b,
        <I as std::iter::IntoIterator>::IntoIter: Send,
    {
        Self::Split {
            program,
            args: Box::new(args.into_iter()),
        }
    }
}
