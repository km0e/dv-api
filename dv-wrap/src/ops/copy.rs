use std::{borrow::Cow, ops::Deref};

use super::dev::*;
use tracing::{debug, trace};

pub async fn try_copy(src: &User, src_path: &U8Path, dst: &User, dst_path: &U8Path) -> Result<()> {
    let mut src = src.open(src_path, OpenFlags::READ).await?;
    let mut dst = dst
        .open(
            dst_path,
            OpenFlags::WRITE | OpenFlags::CREATE | OpenFlags::TRUNCATE,
        )
        .await?;
    tokio::io::copy(&mut src, &mut dst).await?;
    Ok(())
}

pub struct CopyContext<'a> {
    ctx: Context<'a>,
    pub src: &'a User,
    src_uid: &'a str,
    dst: &'a User,
    dst_uid: &'a str,
    pub opt: &'a str,
}

impl<'a> Deref for CopyContext<'a> {
    type Target = Context<'a>;
    fn deref(&self) -> &Self::Target {
        &self.ctx
    }
}
impl<'a> CopyContext<'a> {
    pub fn new(
        ctx: Context<'a>,
        src_uid: &'a str,
        dst_uid: &'a str,
        mut opt: Option<&'a str>,
    ) -> Result<Self> {
        let src = ctx.get_user(src_uid)?;
        let dst = ctx.get_user(dst_uid)?;
        if opt.is_some_and(|o| !o.chars().all(|c| c == 'y' || c == 'n' || c == 'u')) {
            opt = None;
        }
        Ok(Self {
            ctx,
            src,
            src_uid,
            dst,
            dst_uid,
            opt: opt.unwrap_or(""),
        })
    }

    async fn check_copy_file(
        &self,
        src_path: &U8Path,
        dst_path: &U8Path,
        src_attr: FileAttributes,
        dst_attr: Option<FileAttributes>,
    ) -> Result<bool> {
        trace!(
            "check_copy_file {}:{} -> {}:{}",
            self.src_uid, src_path, self.dst_uid, dst_path
        );
        let dst_mtime = dst_attr.as_ref().and_then(|a| a.mtime);
        let cache = self.ctx.cache.get(self.dst_uid, dst_path.as_str()).await?;
        let overwrite = src_attr
            .mtime
            .is_some_and(|mt| cache.is_none_or(|(ver, _)| ver != mt as i64) || dst_mtime.is_none());
        let update = dst_mtime.is_some_and(|mt| {
            cache.is_none_or(|(_, old)| old != mt as i64) || src_attr.mtime.is_none()
        });
        debug!(
            "{}:{}({:?}) - {}:{}({:?}) - {:?}",
            self.src_uid,
            src_path,
            src_attr.mtime,
            self.dst_uid,
            dst_path,
            dst_attr.as_ref().map(|a| a.mtime),
            cache
        );
        let res = 'check_opt: {
            if !overwrite && !update {
                break 'check_opt None;
            }
            for opt in self.opt.chars() {
                match opt {
                    'y' if overwrite => break 'check_opt Some(false),
                    'u' if update => break 'check_opt Some(true),
                    'n' => break 'check_opt None,
                    _ => continue,
                }
            }
            let mut hint = String::new();
            let mut opts = Vec::new();
            if overwrite {
                hint.push_str(self.src_uid);
                hint.push(':');
                hint.push_str(src_path.as_str());
                hint.push_str(" is newer, ");
                opts.push("y/overwrite");
            }
            if update {
                hint.push_str(self.dst_uid);
                hint.push(':');
                hint.push_str(dst_path.as_str());
                hint.push_str(" is newer, ");
                opts.push("u/update");
            }
            hint.push_str("do what?");
            opts.push("n/skip");
            let sel = self.interactor.confirm(hint, &opts).await?;
            match opts[sel].chars().nth(0) {
                Some('y') => Some(false),
                Some('u') => Some(true),
                Some('n') => None,
                _ => unreachable!(),
            }
        };

        if let Some(rev) = if !self.dry_run { res } else { None } {
            let (src_ts, dst_ts) = if !rev {
                try_copy(self.src, src_path, self.dst, dst_path).await?;
                let src_ts = match src_attr.mtime {
                    Some(ts) => Some(ts as i64),
                    None => self.src.get_mtime(src_path).await?,
                };
                (src_ts, self.dst.get_mtime(dst_path).await?)
            } else {
                try_copy(self.dst, dst_path, self.src, src_path).await?;
                let dst_ts = match dst_mtime {
                    Some(ts) => Some(ts as i64),
                    None => self.dst.get_mtime(dst_path).await?,
                };
                (self.src.get_mtime(src_path).await?, dst_ts)
            };
            let Some(src_ts) = src_ts else {
                whatever!("get {} mtime failed", src_path)
            };
            let Some(dst_ts) = dst_ts else {
                whatever!("get {} mtime failed", dst_path)
            };
            self.cache
                .set(self.dst_uid, dst_path.as_str(), src_ts, dst_ts)
                .await?;
        }
        let update = res.is_some_and(|do_| !do_);
        action!(
            self,
            res.is_some(),
            "{} {}:{} {} {}:{}",
            if update { "update" } else { "copy" },
            self.src_uid,
            src_path,
            if update { "<-" } else { "->" },
            self.dst_uid,
            dst_path
        );
        Ok(res.is_some())
    }

    async fn check_copy_dir(
        &self,
        src_path: U8PathBuf,
        dst_path: U8PathBuf,
        meta: Vec<Metadata>,
    ) -> Result<bool> {
        let mut success = false;
        let mut src_file = src_path.clone();
        let mut dst_file = dst_path.clone();
        for Metadata { path, attr } in meta {
            src_file.push(&path);
            dst_file.push(&path);
            let (full_dst_file, dst_attr) = self.dst.check_file(&dst_file).await?;
            let res = self
                .check_copy_file(
                    src_file.as_str().into(),
                    full_dst_file.as_str().into(),
                    attr,
                    dst_attr,
                )
                .await?;
            src_file.clone_from(&src_path);
            dst_file.clone_from(&dst_path);
            success |= res;
        }
        Ok(success)
    }

    pub async fn copy(&self, src_path: impl AsRef<str>, dst_path: impl AsRef<str>) -> Result<bool> {
        let src_path = src_path.as_ref();
        let dst_path: &str = dst_path.as_ref();
        trace!(
            "copy {}:{} -> {}:{}",
            self.src_uid, src_path, self.dst_uid, dst_path
        );
        if src_path.ends_with('/') {
            let DirInfo { path, files } = self.src.check_dir(src_path).await?;
            self.check_copy_dir(path, dst_path.into(), files).await
        } else {
            let info = self.src.check_path(src_path).await?;
            let dst_path2 = if dst_path.ends_with('/') {
                format!(
                    "{dst_path}{}",
                    src_path
                        .rsplit_once('/')
                        .map(|(_, name)| name)
                        .unwrap_or(src_path)
                )
                .into()
            } else {
                Cow::Borrowed(dst_path)
            };
            let (dst_path2, fa) = self.dst.check_file(dst_path2.as_ref().into()).await?;
            match info {
                CheckInfo::Dir(DirInfo { path, files }) => {
                    self.check_copy_dir(path, dst_path2, files).await
                }
                CheckInfo::File(Metadata { path, attr }) => {
                    self.check_copy_file(&path, &dst_path2, attr, fa).await
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, path::Path, time::Duration};

    use crate::{cache::SqliteCache, dev::User, dv::tests::TestDv, interactor::TermInteractor};

    use assert_fs::{TempDir, fixture::ChildPath, prelude::*};
    use dv_api::multi::Config;

    use super::CopyContext;

    ///Prepare a test environment with a source and destination directory.
    /// # Parameters
    ///
    /// - `src`: list of (name, content) pairs to create in the source directory
    /// - `dst`: list of (name, content) pairs to create in the destination directory
    async fn tenv(src: &[(&str, &str)], dst: &[(&str, &str)]) -> (TestDv, TempDir) {
        let int = TermInteractor::new().unwrap();
        let cache = SqliteCache::memory();
        let dir = TempDir::new().unwrap();
        let mut cfg = Config::default();
        cfg.set("mount", dir.to_string_lossy());
        let mut users = HashMap::new();
        users.insert("this".to_string(), User::new(cfg).await.unwrap());
        let src_dir = dir.child("src");
        for (name, content) in src {
            let f = src_dir.child(name);
            f.write_str(content).unwrap();
        }
        let dst_dir = dir.child("dst");
        for (name, content) in dst {
            let f = dst_dir.child(name);
            f.write_str(content).unwrap();
        }
        (
            TestDv {
                dry_run: false,
                users,
                cache,
                interactor: int,
            },
            dir,
        )
    }
    fn content_assert(dir: &ChildPath, pairs: &[(&str, &str)]) {
        for (name, content) in pairs {
            dir.child(name).assert(*content);
        }
    }
    async fn cache_assert(cache: &SqliteCache, src: &Path, dst: &Path) {
        let src_meta = src.metadata().unwrap();
        let dst_meta = dst.metadata().unwrap();
        let mtime = {
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                (
                    src_meta.last_write_time() as i64,
                    dst_meta.last_write_time() as i64,
                )
            }
            #[cfg(not(windows))]
            {
                use std::os::unix::fs::MetadataExt;
                (src_meta.mtime(), dst_meta.mtime())
            }
        };
        assert_eq!(
            mtime,
            cache
                .get("this", dst.to_str().unwrap())
                .await
                .unwrap()
                .unwrap(),
            "about path: {}",
            dst.display()
        );
    }
    async fn cache_assert2(cache: &SqliteCache, src: ChildPath, dst: ChildPath, subpaths: &[&str]) {
        for subpath in subpaths {
            cache_assert(cache, src.child(subpath).path(), dst.child(subpath).path()).await;
        }
    }
    async fn copy_dir_fixture(src: &str, dst: &str) {
        let (dv, dir) = tenv(&[("f0", "f0"), ("f1", "f1")], &[]).await;
        let ctx = CopyContext::new(dv.context(), "this", "this", Some("y")).unwrap();
        assert!(ctx.copy(src, dst).await.unwrap(), "copy should success");
        content_assert(&dir.child("dst"), &[("f0", "f0"), ("f1", "f1")]);
        cache_assert2(ctx.cache, dir.child("src"), dir.child("dst"), &["f0", "f1"]).await;
    }

    /// Test operation of copy("src/f0", `dst`) will generate `expect`
    async fn copy_file_fixture(dst: &str, expect: &str, default: &str) {
        let (dv, dir) = tenv(&[("f0", "f0")], &[]).await;
        let ctx = CopyContext::new(dv.context(), "this", "this", Some(default)).unwrap();
        assert!(
            ctx.copy("src/f0", dst).await.unwrap(),
            "copy should success"
        );
        dir.child(expect).assert("f0");
        cache_assert(
            ctx.cache,
            dir.child("src/f0").path(),
            dir.child(expect).path(),
        )
        .await;
    }
    #[tokio::test]
    async fn copy_dir() {
        copy_dir_fixture("src/", "dst").await;
        copy_dir_fixture("src/", "dst/").await;
        copy_dir_fixture("src", "dst").await;
        copy_dir_fixture("src", "dst").await;
    }
    #[tokio::test]
    async fn copy_file() {
        copy_file_fixture("dst", "dst", "y").await;
        copy_file_fixture("dst/", "dst/f0", "y").await;
    }
    #[tokio::test]
    async fn test_update() {
        let (dv, dir) = tenv(&[("f0", "f00"), ("f1", "f11")], &[]).await;
        let ctx = CopyContext::new(dv.context(), "this", "this", Some("y")).unwrap();
        assert!(ctx.copy("src", "dst").await.unwrap(), "sync should success");
        tokio::time::sleep(Duration::from_secs(2)).await;
        let src = dir.child("src");
        src.child("f0").write_str("f0").unwrap();
        src.child("f1").write_str("f1").unwrap();
        assert!(
            ctx.copy("src/", "dst").await.unwrap(),
            "sync should success"
        );
        let dst = dir.child("dst");
        dst.child("f0").assert("f0");
        dst.child("f1").assert("f1");
        cache_assert(ctx.cache, src.child("f0").path(), dst.child("f0").path()).await;
        cache_assert(ctx.cache, src.child("f1").path(), dst.child("f1").path()).await;
    }
    #[tokio::test]
    async fn test_donothing() {
        let (dv, dir) = tenv(&[("f0", "f0"), ("f1", "f1")], &[]).await;
        let mut ctx = CopyContext::new(dv.context(), "this", "this", Some("y")).unwrap();
        let src = dir.child("src");
        assert!(
            ctx.copy("src/", "dst").await.unwrap(),
            "sync should success"
        );
        ctx.opt = "n";
        assert!(
            !ctx.copy("src/", "dst").await.unwrap(),
            "sync should do nothing"
        );
        src.child("f0").assert("f0");
        src.child("f1").assert("f1");
        cache_assert(
            ctx.cache,
            dir.child("src/f0").path(),
            dir.child("dst/f0").path(),
        )
        .await;
        cache_assert(
            ctx.cache,
            dir.child("src/f1").path(),
            dir.child("dst/f1").path(),
        )
        .await;
    }
}
