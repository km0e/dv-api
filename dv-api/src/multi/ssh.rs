use std::borrow::Cow;

use crate::whatever;

use super::dev::{self, *};
use e4pty::prelude::{BoxedPty, Script, WindowSize};
use russh::client;
use russh_sftp::{
    client::SftpSession,
    protocol::{FileAttributes, StatusCode},
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::{debug, info, warn};
mod config;
pub use config::create;
mod file;

struct Client {}

impl client::Handler for Client {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _: &russh::keys::ssh_key::PublicKey,
    ) -> std::result::Result<bool, Self::Error> {
        Ok(true)
    }
}

pub(crate) struct SSHSession {
    session: client::Handle<Client>,
    sftp: SftpSession,
    home: Option<String>,
}

impl SSHSession {
    fn canonicalize<'a, 'b: 'a>(&'b self, path: &'a str) -> Result<Cow<'a, str>> {
        Ok(if let Some(path) = path.strip_prefix("~") {
            let Some(home) = self.home.as_deref() else {
                whatever!("unknown home")
            };
            if path.starts_with("/") {
                format!("{home}{path}").into()
            } else {
                home.into()
            }
        } else {
            path.into()
        })
    }
    async fn prepare_command(&self, command: Script<'_, '_>) -> Result<String> {
        let cmd = match command {
            Script::Whole(cmd) => cmd.to_string(),
            Script::Split { program, args } => {
                let mut cmd = program.to_string();
                for arg in args {
                    cmd.push(' ');
                    cmd.push_str(arg);
                }
                cmd
            }
            Script::Script { executor, input } => {
                let mut retry = 5;
                let mut name = String::with_capacity(4 + 6);
                loop {
                    //TODO:extract to a function?
                    name.push_str(".tmp");
                    for c in std::iter::repeat_with(fastrand::alphanumeric).take(6) {
                        name.push(c);
                    }
                    use russh_sftp::protocol::OpenFlags;
                    let res = self
                        .sftp
                        .open_with_flags(
                            &name,
                            OpenFlags::CREATE | OpenFlags::WRITE | OpenFlags::EXCLUDE,
                        )
                        .await;
                    if let Ok(mut file) = res {
                        file.write_all(&executor.prepare_clean()).await?;
                        for blk in input {
                            file.write_all(blk.as_bytes()).await?;
                        }
                        break;
                    } else if retry == 0 {
                        res?;
                    }
                    retry -= 1;
                    name.clear();
                }
                let cmd = format!("{executor} {name}");
                cmd
            }
        };
        Ok(cmd)
    }
    async fn create_parent(&self, path: &str) -> Result<()> {
        let Some((parent, _)) = path.rsplit_once("/") else {
            whatever!("invalid path {}", path)
        };
        debug!("try create dir {}", parent);
        match self.sftp.create_dir(parent).await {
            Ok(_) => Ok(()),
            Err(russh_sftp::client::error::Error::Status(s))
                if s.status_code == StatusCode::NoSuchFile
                    || s.status_code == StatusCode::Failure//NOTE:why failure?
            =>
            {
                Box::pin(self.create_parent(parent)).await?;
                Ok(self.sftp.create_dir(parent).await?)
            }
            Err(e) => Err(e)?,
        }
    }
}

#[async_trait]
impl UserImpl for SSHSession {
    async fn exist(&self, path: &U8Path) -> Result<bool> {
        let path2 = self.canonicalize(path.as_str())?;
        let path = path2.as_ref();
        Ok(self.sftp.try_exists(path).await?)
    }
    async fn file_attributes(&self, path: &U8Path) -> Result<(U8PathBuf, Option<FileAttributes>)> {
        let path = self.canonicalize(path.as_str())?.to_string();
        match self.sftp.metadata(&path).await {
            Ok(attr) => Ok((path.into(), Some(attr))),
            Err(russh_sftp::client::error::Error::Status(russh_sftp::protocol::Status {
                status_code: russh_sftp::protocol::StatusCode::NoSuchFile,
                ..
            })) => {
                debug!("{} not found", path);
                Ok((path.into(), None))
            }
            Err(e) => Err(e.into()),
        }
    }
    async fn glob_file_meta(&self, path: &U8Path) -> crate::Result<Vec<Metadata>> {
        let metadata = self.sftp.metadata(path.to_string()).await?;
        if metadata.is_dir() {
            let mut stack = vec![path.to_string()];
            let prefix = format!("{path}/");
            let mut infos = Vec::new();
            while let Some(path) = stack.pop() {
                for entry in self.sftp.read_dir(&path).await? {
                    let sub_path = format!("{}/{}", path, entry.file_name());
                    if entry.file_type().is_dir() {
                        stack.push(sub_path);
                        continue;
                    }
                    if !entry.file_type().is_file() {
                        warn!("find {:?} type file {sub_path}", entry.file_type());
                        continue;
                    }
                    infos.push(Metadata {
                        path: sub_path.strip_prefix(&prefix).unwrap().to_string().into(),
                        attr: entry.metadata(),
                    });
                }
            }
            Ok(infos)
        } else {
            whatever!("{path} is a {:?}", metadata.file_type())
        }
    }
    async fn exec(&self, command: Script<'_, '_>) -> Result<Output> {
        let channel = self.session.channel_open_session().await?;
        let cmd = self.prepare_command(command).await?;
        info!("exec {}", cmd);
        channel.exec(true, cmd).await?;
        let mut pty = channel.into_pty();
        let mut stdout = Vec::new();
        pty.reader.read_to_end(&mut stdout).await?;
        let code = pty.ctl.wait().await?;
        debug!("exec done");
        Ok(Output {
            code,
            stdout,
            stderr: Vec::new(),
        })
    }
    async fn pty(&self, command: Script<'_, '_>, win_size: WindowSize) -> Result<BoxedPty> {
        debug!("open pty with size: {:?}", win_size);
        let channel = self.session.channel_open_session().await?;
        channel
            .request_pty(
                false,
                std::env::var("TERM").as_deref().unwrap_or("xterm"),
                win_size.cols as u32,
                win_size.rows as u32,
                0,
                0,
                &[],
            )
            .await?;
        let cmd = self.prepare_command(command).await?;
        info!("exec {}", cmd);
        channel.exec(true, cmd).await?;
        Ok(channel.into_pty())
    }
    async fn open(&self, path: &str, flags: OpenFlags, attr: FileAttributes) -> Result<BoxedFile> {
        let path2 = self.canonicalize(path)?;
        let path = path2.as_ref();

        let open_flags = flags.into();
        let file = loop {
            match self
                .sftp
                .open_with_flags_and_attributes(path, open_flags, attr.clone())
                .await
            {
                Ok(file) => break Ok(file),
                Err(russh_sftp::client::error::Error::Status(s))
                    if s.status_code == StatusCode::NoSuchFile =>
                {
                    self.create_parent(path).await?;
                }
                Err(e) => break Err(e),
            }
        }?;
        Ok(Box::new(file))
    }
}
