use super::dev::{self, *};
use std::path::{Path, PathBuf};

mod config;
pub use config::create;
mod file;

pub(crate) struct This {
    home: Option<PathBuf>,
}

impl This {
    pub async fn new() -> Result<Self> {
        Ok(Self {
            home: std::env::home_dir(),
        })
    }
    fn canonicalize<'a, 'b: 'a>(&'b self, path: &'a str) -> Result<Cow<'a, Path>> {
        Ok(if let Some(path) = path.strip_prefix("~") {
            let Some(home) = self.home.as_ref() else {
                whatever!("unknown home")
            };
            if let Some(path) = path.strip_prefix("/") {
                home.join(path).into()
            } else {
                home.into()
            }
        } else {
            Path::new(path).into()
        })
    }
}

#[async_trait]
impl UserImpl for This {
    async fn file_attributes(&self, path: &U8Path) -> Result<(U8PathBuf, Option<FileAttributes>)> {
        let path = self
            .canonicalize(path.as_str())?
            .to_string_lossy()
            .to_string();
        match std::fs::metadata(&path).map(|meta| (&meta).into()) {
            Ok(attr) => Ok((path.into(), Some(attr))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                debug!("{} not found", path);
                Ok((path.into(), None))
            }
            Err(e) => Err(e.into()),
        }
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
                result.push(Metadata {
                    path: file_path.to_string_lossy().to_string().into(),
                    attr: (&metadata).into(),
                });
            }
            Ok(result)
        } else {
            whatever!("{} not a directory", path2)
        }
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

    async fn rm(&self, path: &U8Path) -> Result<()> {
        let path = self.canonicalize(path.as_str())?;
        debug!("rm:{}", path.display());
        match std::fs::remove_file(&path) {
            Ok(_) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                debug!("{} not found", path.display());
                Ok(())
            }
            Err(e) => Err(e.into()),
        }
    }
    async fn open(
        &self,
        path: &U8Path,
        flags: OpenFlags,
        attr: FileAttributes,
    ) -> Result<BoxedFile> {
        let path = Path::new(path.as_str());
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
            match open_options.open(&path).await {
                Ok(file) => break Ok(file),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    let parent = path.parent().unwrap();
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
