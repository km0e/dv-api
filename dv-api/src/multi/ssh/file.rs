use async_trait::async_trait;

use super::{FileImpl, OpenFlags};
use russh_sftp::protocol::OpenFlags as ThisOpenFlags;

impl From<OpenFlags> for ThisOpenFlags {
    fn from(value: OpenFlags) -> Self {
        let mut open_flags = ThisOpenFlags::default();
        if value.contains(OpenFlags::READ) {
            open_flags |= ThisOpenFlags::READ;
        }
        if value.contains(OpenFlags::WRITE) {
            open_flags |= ThisOpenFlags::WRITE;
        }
        if value.contains(OpenFlags::APPEND) {
            open_flags |= ThisOpenFlags::APPEND;
        }
        if value.contains(OpenFlags::CREATE) {
            open_flags |= ThisOpenFlags::CREATE;
        }
        if value.contains(OpenFlags::EXCLUDE) {
            open_flags |= ThisOpenFlags::EXCLUDE;
        }
        if value.contains(OpenFlags::TRUNCATE) {
            open_flags |= ThisOpenFlags::TRUNCATE;
        }
        open_flags
    }
}

#[async_trait]
impl FileImpl for russh_sftp::client::fs::File {}
