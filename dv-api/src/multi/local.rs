use crate::{Error, fs::Metadata, whatever};

use super::dev::{self, *};
use autox::AutoX;
use resplus::attach;
use russh_sftp::protocol::FileAttributes;

use std::{
    borrow::Cow,
    path::{Path, PathBuf},
};
use tracing::{debug, trace};

mod config;
pub use config::create;
mod file;

pub(crate) struct This {
    home: Option<PathBuf>,
    autox: AutoX,
}

impl This {
    pub async fn new(is_system: bool) -> Result<Self> {
        let autox = AutoX::new(is_system).await.map_err(Error::unknown)?;
        Ok(Self {
            home: home::home_dir(),
            autox,
        })
    }
    fn canonicalize<'a, 'b: 'a>(&'b self, path: &'a str) -> Result<Cow<'a, Path>> {
        let mut new = String::with_capacity(path.len());
        let mut last_match = 0;
        for caps in VARIABLE_RE.captures_iter(path) {
            let m = caps.get(0).unwrap();
            let var = caps.get(1).unwrap().as_str();
            let Ok(value) = std::env::var(var) else {
                //TODO:should collect all envs ?
                whatever!("unknown variable {}", var)
            };
            new.push_str(&path[last_match..m.start()]);
            new.push_str(&value);
            last_match = m.end();
        }
        let path: Cow<'a, str> = if last_match == 0 {
            path.into()
        } else {
            new.push_str(&path[last_match..]);
            new.into()
        };
        debug!("try to expand home for {}", path);
        Ok(if let Some(path) = path.strip_prefix("~") {
            let Some(home) = self.home.as_ref() else {
                whatever!("unknown home")
            };
            debug!("try to expand home for {}", path);
            if let Some(path) = path.strip_prefix("/") {
                debug!("try to expand home for {}", path);
                home.join(path).into()
            } else {
                home.into()
            }
        } else {
            match path {
                Cow::Borrowed(path) => Path::new(path).into(),
                Cow::Owned(path) => PathBuf::from(path).into(),
            }
        })
    }
}

#[async_trait]
impl UserImpl for This {
    async fn exist(&self, path: &U8Path) -> Result<bool> {
        let path2 = self.canonicalize(path.as_str())?;
        Ok(std::fs::exists(&path2)?)
    }
    async fn file_attributes(&self, path: &U8Path) -> (U8PathBuf, Result<FileAttributes>) {
        let path2 = attach!(self.canonicalize(path.as_str()), ..);
        if path2.is_err() {
            return (path.into(), Err(path2.unwrap_err()));
        }
        let path2 = path2.unwrap();
        (
            path2.to_string_lossy().to_string().into(),
            std::fs::metadata(&path2)
                .map(|meta| (&meta).into())
                .map_err(|e| e.into()),
        )
    }
    async fn glob_file_meta(&self, path2: &U8Path) -> Result<Vec<Metadata>> {
        let metadata = path2.metadata()?;
        if metadata.is_dir() {
            let mut result = Vec::new();
            for entry in walkdir::WalkDir::new(path2)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                let file_path = entry.path();
                let metadata = match file_path.metadata() {
                    Ok(meta) => meta,
                    Err(_) => continue,
                };
                if metadata.is_dir() {
                    continue;
                }
                let Ok(rel_path) = file_path.strip_prefix(path2) else {
                    continue;
                };
                result.push(Metadata {
                    path: rel_path.to_string_lossy().to_string().into(),
                    attr: (&metadata).into(),
                });
            }
            Ok(result)
        } else {
            whatever!("{} not a directory", path2)
        }
    }
    async fn auto(&self, name: &str, action: &str, args: Option<&str>) -> Result<()> {
        match (action, args) {
            ("setup", Some(args)) => self.autox.setup(name, args).await.map_err(Error::unknown)?,
            ("reload", None) => self.autox.reload(name).await.map_err(Error::unknown)?,
            ("destroy", None) => self.autox.destroy(name).await.map_err(Error::unknown)?,
            _ => unimplemented!(),
        };
        Ok(())
    }
    async fn exec(&self, script: Script<'_, '_>) -> Result<Output> {
        let mut builder = script.into_command()?;
        builder
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        let output = builder.output()?;
        Ok(Output {
            code: exit_status2exit_code(output.status),
            stdout: output.stdout,
            stderr: output.stderr,
        })
    }
    async fn pty(&self, command: Script<'_, '_>, win_size: WindowSize) -> Result<BoxedPty> {
        trace!("try to exec command");
        let pty = openpty_local(win_size, command)?;
        Ok(pty)
    }
    async fn open(&self, path: &str, flags: OpenFlags, attr: FileAttributes) -> Result<BoxedFile> {
        let path2 = Path::new(path);
        let mut open_options = tokio::fs::OpenOptions::from(flags);

        #[cfg(unix)]
        open_options.mode(attr.permissions.unwrap_or_default());
        #[cfg(windows)]
        {
            const GENERIC_READ: u32 = 0x80000000;
            const GENERIC_WRITE: u32 = 0x40000000;
            const GENERIC_EXECUTE: u32 = 0x20000000;
            // const GENERIC_ALL: u32 = 0x10000000;
            let mut access = 0;
            let permissions = attr.permissions();
            if permissions.owner_read {
                access |= GENERIC_READ;
            }
            if permissions.owner_write {
                access |= GENERIC_WRITE;
            }
            if permissions.owner_exec {
                access |= GENERIC_EXECUTE;
            }
            open_options.access_mode(access);
        }

        let file = loop {
            match open_options.open(&path2).await {
                Ok(file) => break Ok(file),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    let parent = path2.parent().unwrap();
                    debug!("try to create dir {}", parent.display());
                    tokio::fs::create_dir_all(parent).await?;
                }
                Err(e) => break Err(e),
            }
        };
        let file = file?;
        Ok(Box::new(file))
    }
}

#[cfg(not(windows))]
pub fn exit_status2exit_code(es: std::process::ExitStatus) -> i32 {
    use std::os::unix::process::ExitStatusExt;
    es.code()
        .unwrap_or_else(|| es.signal().map_or(1, |v| 128 + v))
}

#[cfg(windows)]
pub fn exit_status2exit_code(es: std::process::ExitStatus) -> i32 {
    es.code().unwrap_or(1)
}
