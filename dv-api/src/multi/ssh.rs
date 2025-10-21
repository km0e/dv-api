use super::dev::{self, *};
use russh::client;
use russh_sftp::{client::SftpSession, protocol::StatusCode};
use tracing::{info, warn};

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
    session: Vec<client::Handle<Client>>,
    sftp: SftpSession,
    home: Option<String>,
}

impl SSHSession {
    fn session(&self) -> &client::Handle<Client> {
        self.session.last().as_ref().expect("no session")
    }
    fn canonicalize<'a, 'b: 'a>(&'b self, path: &'a U8Path) -> Result<Cow<'a, str>> {
        Ok(if let Some(sub) = path.as_str().strip_prefix("~") {
            let Some(home) = self.home.as_deref() else {
                whatever!("unknown home")
            };
            if sub.starts_with("/") {
                format!("{home}{sub}").into()
            } else if sub.is_empty() {
                home.into()
            } else {
                path.as_str().into()
            }
        } else {
            path.as_str().into()
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
                name.push_str(".tmp");
                loop {
                    //TODO:extract to a function?
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
                        use tokio::io::AsyncWriteExt;
                        file.write_all(&executor.prepare_clean()).await?;
                        file.write_all(input.as_bytes()).await?;
                        break;
                    } else if retry == 0 {
                        res?;
                    }
                    retry -= 1;
                    name.truncate(4);
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
    async fn file_attributes(&self, path: &U8Path) -> Result<(U8PathBuf, Option<FileAttributes>)> {
        let path = self.canonicalize(path)?.to_string();
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
                        path: sub_path.into(),
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
        let channel = self.session().channel_open_session().await?;
        let cmd = self.prepare_command(command).await?;
        info!("exec {}", cmd);
        channel.exec(true, cmd).await?;
        let mut pty = channel.into_pty();
        let mut stdout = Vec::new();
        use tokio::io::AsyncReadExt;
        pty.reader.read_to_end(&mut stdout).await?;
        let code = pty.ctl.wait().await?;
        debug!("exec done");
        Ok(Output {
            code,
            stdout,
            stderr: Vec::new(),
        })
    }
    async fn rm(&self, path: &U8Path) -> Result<()> {
        let path = self.canonicalize(path)?;
        debug!("rm: {}", path);
        match self.sftp.remove_file(path.as_ref()).await {
            Ok(_) => Ok(()),
            Err(russh_sftp::client::error::Error::Status(s))
                if s.status_code == StatusCode::NoSuchFile =>
            {
                debug!("{} not found", path);
                Ok(())
            }
            Err(e) => Err(e.into()),
        }
    }
    async fn pty(&self, command: Script<'_, '_>, win_size: WindowSize) -> Result<BoxedPty> {
        debug!("open pty with size: {:?}", win_size);
        let channel = self.session().channel_open_session().await?;
        channel
            .request_pty(
                true,
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
    async fn open(
        &self,
        path: &U8Path,
        flags: OpenFlags,
        attr: FileAttributes,
    ) -> Result<BoxedFile> {
        let path2 = self.canonicalize(path)?;
        let path = path2.as_ref();
        trace!("open: {}, flags: {:?}", path, flags);
        let open_flags = flags.into();
        let file = loop {
            match self
                .sftp
                .open_with_flags_and_attributes(path, open_flags, attr.clone())
                .await
            {
                Ok(file) => break Ok(file),
                Err(russh_sftp::client::error::Error::Status(s))
                    if s.status_code == StatusCode::NoSuchFile
                        && flags.contains(OpenFlags::CREATE) =>
                {
                    self.create_parent(path).await?;
                }
                Err(e) => break Err(e),
            }
        }?;
        Ok(Box::new(file))
    }
}
