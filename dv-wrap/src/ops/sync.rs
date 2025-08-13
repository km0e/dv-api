use std::fmt::Write;
use std::ops::Deref;

use super::dev::*;
use tracing::{debug, info};

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

pub struct SyncContext<'a> {
    ctx: &'a Context,
    pub src: &'a User,
    suid: &'a str,
    pub dst: &'a User,
    duid: &'a str,
    pub opt: &'a str,
}

impl<'a> Deref for SyncContext<'a> {
    type Target = Context;
    fn deref(&self) -> &Self::Target {
        self.ctx
    }
}
impl<'a> SyncContext<'a> {
    pub fn new(
        ctx: &'a Context,
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
            suid: src_uid,
            dst,
            duid: dst_uid,
            opt: opt.unwrap_or(""),
        })
    }
    async fn select(
        &self,
        sp: &U8Path,
        dp: &U8Path,
        overwrite: Option<&str>,
        update: Option<&str>,
    ) -> Result<Option<bool>> {
        for opt in self.opt.chars() {
            match opt {
                'y' if overwrite.is_some() => return Ok(Some(false)),
                'u' if update.is_some() => return Ok(Some(true)),
                'n' => return Ok(None),
                _ => continue,
            }
        }
        let mut hint = String::new();
        let mut opts = Vec::new();
        write!(&mut hint, "{}:{sp} -> {}:{dp}", self.suid, self.duid).unwrap();
        let mut storage = Vec::new();
        if let Some(overwrite) = overwrite {
            if overwrite.is_empty() {
                opts.push("y");
            } else {
                storage.push(format!("y/{overwrite}"));
            }
        }
        if let Some(update) = update {
            if update.is_empty() {
                opts.push("y");
            } else {
                storage.push(format!("y/{update}"));
            }
        }
        opts.extend(storage.iter().map(String::as_str));
        opts.push("n/skip");
        let sel = self.interactor.confirm(hint, &opts).await?;
        Ok(match opts[sel].chars().nth(0) {
            Some('y') => Some(false),
            Some('u') => Some(true),
            Some('n') => None,
            _ => unreachable!(),
        })
    }

    async fn update_cache(
        &self,
        sp: &U8Path,
        dp: &U8Path,
        src_ts: Option<i64>,
        dst_ts: Option<i64>,
    ) -> Result<()> {
        let Some(src_ts) = src_ts else {
            whatever!("get {} mtime failed", sp)
        };
        let Some(dst_ts) = dst_ts else {
            whatever!("get {} mtime failed", dp)
        };
        self.cache
            .set(
                self.duid,
                dp.as_str(),
                &src_ts.to_string(),
                &dst_ts.to_string(),
            )
            .await?;
        Ok(())
    }
    async fn rm(&self, sp: &U8Path, dm: &Metadata) -> Result<bool> {
        let res = self
            .select(sp, &dm.path, Some("rm"), Some("download"))
            .await?;
        if let Some(rev) = if !self.dry_run { res } else { None } {
            if !rev {
                try_copy(self.dst, &dm.path, self.src, sp).await?;
                let dst_ts = match dm.attr.mtime {
                    Some(ts) => Some(ts as i64),
                    None => self.dst.get_mtime(&dm.path).await?,
                };
                self.update_cache(sp, &dm.path, self.src.get_mtime(sp).await?, dst_ts)
                    .await?;
            } else {
                self.cache.del(self.duid, dm.path.as_str()).await?;
                self.dst.rm(&dm.path).await?;
            }
        }
        match res {
            Some(true) => {
                action!(
                    self,
                    true,
                    "download {}:{} <- {}:{}",
                    self.suid,
                    sp,
                    self.duid,
                    dm.path
                );
            }
            res => {
                action!(self, res.is_some(), "remove {}:{}", self.duid, dm.path);
            }
        }
        Ok(res.is_some())
    }

    async fn copy_file(
        &self,
        sp: &U8Path,
        dp: &U8Path,
        sa: &FileAttributes,
        da: Option<FileAttributes>,
    ) -> Result<bool> {
        debug!(
            "{}:{}({:?}) - {}:{}({:?})",
            self.suid,
            sp,
            sa.mtime,
            self.duid,
            dp,
            da.as_ref().map(|a| a.mtime),
        );
        let dst_mtime = da.as_ref().and_then(|a| a.mtime);

        let res = if da.is_none() {
            Some(false)
        } else {
            'check_opt: {
                let cache = self.ctx.cache.get_as::<i64>(self.duid, dp.as_str()).await?;
                debug!("check {}:{} cache: {:?}", self.duid, dp, cache);
                let overwrite = sa.mtime.is_some_and(|mt| {
                    cache.is_none_or(|(ver, _)| ver != mt as i64) || dst_mtime.is_none()
                });
                let update = dst_mtime.is_some_and(|mt| {
                    cache.is_none_or(|(_, old)| old != mt as i64) || sa.mtime.is_none()
                });
                if !overwrite && !update {
                    break 'check_opt None;
                }
                self.select(
                    sp,
                    dp,
                    overwrite.then_some("overwrite"),
                    update.then_some("update"),
                )
                .await?
            }
        };

        if let Some(rev) = if !self.dry_run { res } else { None } {
            let (src_ts, dst_ts) = if !rev {
                try_copy(self.src, sp, self.dst, dp).await?;
                let src_ts = match sa.mtime {
                    Some(ts) => Some(ts as i64),
                    None => self.src.get_mtime(sp).await?,
                };
                (src_ts, self.dst.get_mtime(dp).await?)
            } else {
                try_copy(self.dst, dp, self.src, sp).await?;
                let dst_ts = match dst_mtime {
                    Some(ts) => Some(ts as i64),
                    None => self.dst.get_mtime(dp).await?,
                };
                (self.src.get_mtime(sp).await?, dst_ts)
            };
            self.update_cache(sp, dp, src_ts, dst_ts).await?;
        }
        let update = res.is_some_and(|do_| do_);
        action!(
            self,
            res.is_some(),
            "{} {}:{} {} {}:{}",
            if update { "update" } else { "copy" },
            self.suid,
            sp,
            if update { "<-" } else { "->" },
            self.duid,
            dp
        );
        Ok(res.is_some())
    }

    async fn check_copy_dir(
        &self,
        sp: U8PathBuf,
        dp: U8PathBuf,
        mut dst_files: Vec<Metadata>,
    ) -> Result<bool> {
        debug!(
            "check_copy_dir {}:{} -> {}:{}",
            self.suid,
            sp.as_str(),
            self.duid,
            dp.as_str()
        );
        let mut src_files = self.src.glob(&sp).await?;
        src_files.sort_by(|m1, m2| m1.path.as_str().cmp(m2.path.as_str()));
        dst_files.sort_by(|m1, m2| m1.path.as_str().cmp(m2.path.as_str()));
        let mut si = src_files.iter().peekable();
        let mut di = dst_files.iter().peekable();
        let mut res = false;
        loop {
            match (si.peek(), di.peek()) {
                (Some(sm), Some(dm)) => {
                    let ss = sm.path.strip_prefix(&sp).unwrap();
                    let ds = dm.path.strip_prefix(&dp).unwrap();
                    if ss == ds {
                        res |= self
                            .copy_file(&sm.path, &dm.path, &sm.attr, Some(dm.attr.clone()))
                            .await?;
                        si.next();
                        di.next();
                    } else if ss < ds {
                        let dp = dp.join(ss);
                        res |= self.copy_file(&sm.path, &dp, &sm.attr, None).await?;
                        si.next();
                    } else {
                        let sp = sp.join(ds);
                        res |= self.rm(&sp, dm).await?;
                        di.next();
                    }
                }
                (Some(_), None) => {
                    for sm in si {
                        let dp = dp.join(sm.path.strip_prefix(&sp).unwrap());
                        res |= self.copy_file(&sm.path, &dp, &sm.attr, None).await?;
                    }
                    break;
                }
                (None, Some(_)) => {
                    for dm in di {
                        let ds = dm.path.strip_prefix(&dp).unwrap();
                        let sp = sp.join(ds);
                        res |= self.rm(&sp, dm).await?;
                    }
                    break;
                }
                (None, None) => break,
            }
        }

        Ok(res)
    }

    pub async fn sync(&self, src_path: impl AsRef<str>, dst_path: impl AsRef<str>) -> Result<bool> {
        let src_path = src_path.as_ref();
        let dst_path: &str = dst_path.as_ref();
        info!(
            "sync {}:{} -> {}:{}",
            self.suid, src_path, self.duid, dst_path
        );
        let (src_path, src_attr) = self.src.file_attributes(src_path.into()).await?;
        let Some(src_attr) = src_attr else {
            debug!("{}:{} not found", self.suid, src_path);
            return Ok(false);
        };

        let (dst_path, dst_attr) = self.dst.file_attributes(dst_path.into()).await?;
        match (src_attr.is_dir(), dst_attr) {
            (true, None) => {
                // Both are directories
                self.check_copy_dir(src_path, dst_path, Vec::new()).await
            }
            (true, Some(attr)) if attr.is_dir() => {
                let dst_files = self.dst.glob(&dst_path).await?;
                self.check_copy_dir(src_path, dst_path, dst_files).await
            }
            (false, dst_attr) if dst_attr.as_ref().is_none_or(|a| !a.is_dir()) => {
                // Source is a file, destination is not a directory
                self.copy_file(&src_path, &dst_path, &src_attr, dst_attr)
                    .await
            }
            (_, dst_attr) => {
                // Mismatched types: source is a directory but destination is a file or vice versa
                whatever!(
                    "mismatched types: {}:{} is {} but {}:{} is {}",
                    self.suid,
                    src_path,
                    if src_attr.is_dir() {
                        "directory"
                    } else {
                        "file"
                    },
                    self.duid,
                    dst_path,
                    if dst_attr.is_some_and(|a| a.is_dir()) {
                        "directory"
                    } else {
                        "file"
                    }
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, path::Path, time::Duration};

    use crate::{
        Context,
        cache::{MultiCache, SqliteCache},
        dev::User,
        interactor::TermInteractor,
    };

    use assert_fs::{TempDir, fixture::ChildPath, prelude::*};
    use dv_api::multi::Config;

    use super::SyncContext;

    ///Prepare a test environment with a source and destination directory.
    /// # Parameters
    ///
    /// - `src`: list of (name, content) pairs to create in the source directory
    /// - `dst`: list of (name, content) pairs to create in the destination directory
    async fn tenv(src: &[(&str, &str)], dst: &[(&str, &str)]) -> (Context, TempDir) {
        let interactor = TermInteractor::new().unwrap();
        let mut cache = MultiCache::default();
        cache.add_cache(SqliteCache::memory());
        let dir = TempDir::new().unwrap();
        let mut cfg = Config::default();
        cfg.set("mount", dir.to_string_lossy());
        let mut users = HashMap::new();
        users.insert("this".to_string(), User::local(cfg).await.unwrap());
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
            Context {
                dry_run: false,
                cache,
                interactor,
                users,
                devices: HashMap::new(),
            },
            dir,
        )
    }
    fn content_assert(dir: &ChildPath, pairs: &[(&str, &str)]) {
        for (name, content) in pairs {
            dir.child(name).assert(*content);
        }
    }
    async fn cache_assert(cache: &MultiCache, src: &Path, dst: &Path) {
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
                .get_as::<i64>("this", dst.to_str().unwrap())
                .await
                .unwrap()
                .unwrap(),
            "about path: {}",
            dst.display()
        );
    }
    async fn cache_assert2(cache: &MultiCache, src: ChildPath, dst: ChildPath, subpaths: &[&str]) {
        for subpath in subpaths {
            cache_assert(cache, src.child(subpath).path(), dst.child(subpath).path()).await;
        }
    }
    async fn copy_dir_fixture(src: &str, dst: &str) {
        let (ctx, dir) = tenv(&[("f0", "f0"), ("f1", "f1")], &[]).await;
        let ctx = SyncContext::new(&ctx, "this", "this", Some("y")).unwrap();
        assert!(ctx.sync(src, dst).await.unwrap(), "copy should success");
        content_assert(&dir.child("dst"), &[("f0", "f0"), ("f1", "f1")]);
        cache_assert2(
            &ctx.cache,
            dir.child("src"),
            dir.child("dst"),
            &["f0", "f1"],
        )
        .await;
    }

    /// Test operation of copy("src/f0", `dst`) will generate `expect`
    async fn copy_file_fixture(dst: &str, expect: &str, default: &str) {
        let (ctx, dir) = tenv(&[("f0", "f0")], &[]).await;
        let ctx = SyncContext::new(&ctx, "this", "this", Some(default)).unwrap();
        assert!(
            ctx.sync("src/f0", dst).await.unwrap(),
            "copy should success"
        );
        dir.child(expect).assert("f0");
        cache_assert(
            &ctx.cache,
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
        let (ctx, dir) = tenv(&[("f0", "f00"), ("f1", "f11")], &[]).await;
        let ctx = SyncContext::new(&ctx, "this", "this", Some("y")).unwrap();
        assert!(ctx.sync("src", "dst").await.unwrap(), "sync should success");
        tokio::time::sleep(Duration::from_secs(2)).await;
        let src = dir.child("src");
        src.child("f0").write_str("f0").unwrap();
        src.child("f1").write_str("f1").unwrap();
        assert!(
            ctx.sync("src/", "dst").await.unwrap(),
            "sync should success"
        );
        let dst = dir.child("dst");
        dst.child("f0").assert("f0");
        dst.child("f1").assert("f1");
        cache_assert(&ctx.cache, src.child("f0").path(), dst.child("f0").path()).await;
        cache_assert(&ctx.cache, src.child("f1").path(), dst.child("f1").path()).await;
    }
    #[tokio::test]
    async fn test_donothing() {
        let (ctx, dir) = tenv(&[("f0", "f0"), ("f1", "f1")], &[]).await;
        let mut ctx = SyncContext::new(&ctx, "this", "this", Some("y")).unwrap();
        let src = dir.child("src");
        assert!(
            ctx.sync("src/", "dst").await.unwrap(),
            "sync should success"
        );
        ctx.opt = "n";
        assert!(
            !ctx.sync("src/", "dst").await.unwrap(),
            "sync should do nothing"
        );
        src.child("f0").assert("f0");
        src.child("f1").assert("f1");
        cache_assert(
            &ctx.cache,
            dir.child("src/f0").path(),
            dir.child("dst/f0").path(),
        )
        .await;
        cache_assert(
            &ctx.cache,
            dir.child("src/f1").path(),
            dir.child("dst/f1").path(),
        )
        .await;
    }
}
