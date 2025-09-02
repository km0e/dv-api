pub use camino::{Utf8Path as U8Path, Utf8PathBuf as U8PathBuf};
pub use russh_sftp::protocol::FileAttributes;

#[derive(Debug, Clone)]
pub struct Metadata {
    pub path: U8PathBuf,
    pub attr: FileAttributes,
}

#[derive(Debug, Clone)]
pub struct DirInfo {
    pub path: U8PathBuf,
    pub files: Vec<Metadata>,
}

#[derive(Debug, Clone)]
pub enum CheckInfo {
    Dir(DirInfo),
    File(Metadata),
}

#[derive(Debug, Clone, Copy, Default)]
pub struct OpenFlags(u32);

bitflags::bitflags! {
    impl OpenFlags: u32 {
        const READ = 0x00000001;
        const WRITE = 0x00000002;
        const APPEND = 0x00000004;
        const CREATE = 0x00000008;
        const TRUNCATE = 0x00000010;
        const EXCLUDE = 0x00000020;
    }
}

pub trait FileImpl: tokio::io::AsyncRead + tokio::io::AsyncWrite {}

pub type BoxedFile = Box<dyn FileImpl + Unpin + Send>;
