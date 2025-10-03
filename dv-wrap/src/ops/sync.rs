use super::dev::*;
use anyhow::Result;
use dv_api::fs::{Metadata, U8Path, U8PathBuf};
use std::fmt::Write;
use tracing::{debug, info};

use crate::{Context, MultiDB, interactor::DynInteractor};

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

bitflags::bitflags! {
    #[derive(Default,Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct Opt: u8 {
        const OVERWRITE = 0b000001;
        const UPDATE = 0b000010;
        const DELETEDST = 0b000100;
        const DELETESRC = 0b001000;
        const UPLOAD = 0b010000;
        const DOWNLOAD = 0b100000;
    }
}

struct ScanContext<'a> {
    db: &'a MultiDB,
    int: &'a DynInteractor,
    opts: &'a [Opt],
    suid: &'a str,
    duid: &'a str,
}

#[derive(Default)]
pub struct Entry {
    pub src: U8PathBuf,
    pub dst: U8PathBuf,
    pub src_attr: Option<i64>,
    pub dst_attr: Option<i64>,
    pub opt: Opt,
}

impl<'a> ScanContext<'a> {
    async fn select(&self, sp: &U8Path, dp: &U8Path, opt: Opt) -> Result<Opt> {
        if opt.is_empty() {
            return Ok(Opt::empty());
        }
        if let Some(o) = self
            .opts
            .iter()
            .find(|&&o| o.is_empty() || ((o & opt) == o))
        {
            return Ok(*o);
        }
        let mut hint = String::new();
        let mut opts = Vec::new();
        write!(&mut hint, "{}:{sp} -> {}:{dp}", self.suid, self.duid).unwrap();
        for o in opt.iter() {
            match o {
                Opt::OVERWRITE => {
                    opts.push("y/overwrite");
                }
                Opt::UPDATE => {
                    opts.push("u/update");
                }
                Opt::DELETEDST => {
                    opts.push("d/delete remote");
                }
                Opt::DELETESRC => {
                    opts.push("d/delete local");
                }
                Opt::UPLOAD => {
                    opts.push("y/upload");
                }
                Opt::DOWNLOAD => {
                    opts.push("y/download");
                }
                _ => {}
            }
        }
        opts.push("n/skip");
        let sel = self.int.confirm(hint, &opts).await?;
        Ok(opt.iter().nth(sel).unwrap_or(Opt::empty()))
    }
    async fn select_src(
        &self,
        src: impl Into<U8PathBuf>,
        dst: impl Into<U8PathBuf>,
        sa: dv_api::fs::FileAttributes,
    ) -> Result<Option<Entry>> {
        let src = src.into();
        let dst = dst.into();
        let opt = self
            .select(&src, &dst, Opt::UPLOAD | Opt::DELETESRC)
            .await?;
        if opt.is_empty() {
            return Ok(None);
        }
        Ok(Some(Entry {
            src,
            dst,
            src_attr: sa.mtime.map(|t| t as i64),
            dst_attr: None,
            opt,
        }))
    }
    async fn select_dst(
        &self,
        src: impl Into<U8PathBuf>,
        dst: impl Into<U8PathBuf>,
        da: dv_api::fs::FileAttributes,
    ) -> Result<Option<Entry>> {
        let src = src.into();
        let dst = dst.into();
        let opt = self
            .select(&src, &dst, Opt::DOWNLOAD | Opt::DELETEDST)
            .await?;
        if opt.is_empty() {
            return Ok(None);
        }
        Ok(Some(Entry {
            src,
            dst,
            src_attr: None,
            dst_attr: da.mtime.map(|t| t as i64),
            opt,
        }))
    }
    async fn select_both(
        &self,
        src: impl Into<U8PathBuf>,
        dst: impl Into<U8PathBuf>,
        sa: dv_api::fs::FileAttributes,
        da: dv_api::fs::FileAttributes,
    ) -> Result<Option<Entry>> {
        let src = src.into();
        let dst = dst.into();
        let mut flag = Opt::empty();
        let db = self.db.get_as::<i64>(self.duid, dst.as_str()).await?;
        debug!(db = ?db, "{} : {} = {}, {} : {} = {}",self.suid, src.as_str(), sa.mtime.unwrap_or_default(),self.duid, dst.as_str(), da.mtime.unwrap_or_default());
        if sa
            .mtime
            .is_some_and(|mt| db.is_none_or(|(ver, _)| ver != mt as i64))
        {
            flag |= Opt::OVERWRITE;
        }
        if da
            .mtime
            .is_some_and(|mt| db.is_none_or(|(_, old)| old != mt as i64))
        {
            flag |= Opt::UPDATE;
        }
        let opt = self.select(&src, &dst, flag).await?;
        if opt.is_empty() {
            return Ok(None);
        }
        Ok(Some(Entry {
            src,
            dst,
            src_attr: sa.mtime.map(|t| t as i64),
            dst_attr: da.mtime.map(|t| t as i64),
            opt,
        }))
    }

    async fn check_copy_dir2(
        &self,
        sp: U8PathBuf,
        mut src_files: Vec<Metadata>,
        dp: U8PathBuf,
        mut dst_files: Vec<Metadata>,
    ) -> Result<Vec<Entry>> {
        debug!(
            "check_copy_dir {}:{} -> {}:{}",
            self.suid,
            sp.as_str(),
            self.duid,
            dp.as_str()
        );
        src_files.sort_by(|m1, m2| m1.path.as_str().cmp(m2.path.as_str()));
        dst_files.sort_by(|m1, m2| m1.path.as_str().cmp(m2.path.as_str()));
        let mut si = src_files.into_iter().peekable();
        let mut di = dst_files.into_iter().peekable();
        let mut entries = Vec::new();
        loop {
            match (si.peek(), di.peek()) {
                (Some(sm), Some(dm)) => {
                    let ss = sm.path.strip_prefix(&sp).unwrap();
                    let ds = dm.path.strip_prefix(&dp).unwrap();
                    if ss == ds {
                        let sm = si.next().unwrap();
                        let dm = di.next().unwrap();
                        entries.extend(
                            self.select_both(&sm.path, &dm.path, sm.attr, dm.attr)
                                .await?,
                        );
                    } else if ss < ds {
                        let dp = dp.join(ss);
                        let sm = si.next().unwrap();
                        entries.extend(self.select_src(sm.path, dp, sm.attr).await?);
                    } else {
                        let sp = sp.join(ds);
                        let dm = di.next().unwrap();
                        entries.extend(self.select_dst(sp, dm.path, dm.attr).await?);
                    }
                }
                (Some(_), None) => {
                    for sm in si {
                        let dp = dp.join(sm.path.strip_prefix(&sp).unwrap());
                        entries.extend(self.select_src(sm.path, dp, sm.attr).await?);
                    }
                    break;
                }
                (None, Some(_)) => {
                    for dm in di {
                        let sp = sp.join(dm.path.strip_prefix(&dp).unwrap());
                        entries.extend(self.select_dst(sp, dm.path, dm.attr).await?);
                    }
                    break;
                }
                (None, None) => break,
            }
        }
        Ok(entries)
    }
}
pub struct SyncContext3<'a> {
    ctx: &'a Context,
    opts: &'a [Opt],
    suid: &'a str,
    duid: &'a str,
}

impl<'a> SyncContext3<'a> {
    pub fn new(ctx: &'a Context, suid: &'a str, duid: &'a str, opts: &'a [Opt]) -> Self {
        Self {
            ctx,
            suid,
            duid,
            opts,
        }
    }

    pub async fn scan(
        &self,
        src_path: impl AsRef<str>,
        dst_path: impl AsRef<str>,
    ) -> Result<Vec<Entry>> {
        let src = self.ctx.get_user(self.suid)?;
        let dst = self.ctx.get_user(self.duid)?;
        let src_path = src_path.as_ref();
        let dst_path: &str = dst_path.as_ref();
        let (src_path, src_attr) = src.file_attributes(src_path.into()).await?;
        let (dst_path, dst_attr) = dst.file_attributes(dst_path.into()).await?;
        info!(
            "sync {}:{} -> {}:{}",
            self.suid, src_path, self.duid, dst_path
        );
        let ctx = ScanContext {
            db: &self.ctx.db,
            int: &*self.ctx.interactor,
            opts: self.opts,
            suid: self.suid,
            duid: self.duid,
        };
        match (src_attr, dst_attr) {
            (Some(src_attr), Some(dst_attr)) if src_attr.is_dir() && dst_attr.is_dir() => {
                let src_files = src.glob(&src_path).await?;
                let dst_files = dst.glob(&dst_path).await?;
                ctx.check_copy_dir2(src_path, src_files, dst_path, dst_files)
                    .await
            }
            (Some(src_attr), None) if src_attr.is_dir() => {
                let src_files = src.glob(&src_path).await?;
                ctx.check_copy_dir2(src_path, src_files, dst_path, Vec::new())
                    .await
            }
            (None, Some(dst_attr)) if dst_attr.is_dir() => {
                let dst_files = dst.glob(&dst_path).await?;
                ctx.check_copy_dir2(src_path, Vec::new(), dst_path, dst_files)
                    .await
            }
            (Some(src_attr), Some(dst_attr)) if !src_attr.is_dir() && !dst_attr.is_dir() => {
                Ok(Vec::from_iter(
                    ctx.select_both(&src_path, &dst_path, src_attr, dst_attr)
                        .await?,
                ))
            }
            (Some(src_attr), None) if !src_attr.is_dir() => Ok(Vec::from_iter(
                ctx.select_src(src_path, dst_path, src_attr).await?,
            )),
            (None, Some(dst_attr)) if !dst_attr.is_dir() => Ok(Vec::from_iter(
                ctx.select_dst(src_path, dst_path, dst_attr).await?,
            )),
            (src_attr, dst_attr) => {
                bail!(
                    "mismatched types: {}:{} is {} but {}:{} is {}",
                    self.suid,
                    src_path,
                    match src_attr {
                        Some(a) if a.is_dir() => "directory",
                        Some(_) => "file",
                        None => "not found",
                    },
                    self.duid,
                    dst_path,
                    match dst_attr {
                        Some(a) if a.is_dir() => "directory",
                        Some(_) => "file",
                        None => "not found",
                    }
                )
            }
        }
    }
    pub async fn execute(&self, entres: &[Entry]) -> Result<bool> {
        let src = self.ctx.get_user(self.suid)?;
        let dst = self.ctx.get_user(self.duid)?;
        for entry in entres {
            match entry.opt {
                Opt::OVERWRITE | Opt::UPLOAD => {
                    try_copy(src, &entry.src, dst, &entry.dst).await?;
                    let src_mtime = match entry.src_attr {
                        Some(t) => t,
                        None => src.get_mtime(&entry.src).await?.expect("get mtime"),
                    }
                    .to_string();
                    let dst_mtime = dst
                        .get_mtime(&entry.dst)
                        .await?
                        .expect("get mtime")
                        .to_string();
                    debug!(
                        "set db {} : {} = {}, {}",
                        self.duid,
                        entry.dst.as_str(),
                        src_mtime,
                        dst_mtime
                    );
                    self.ctx
                        .db
                        .set(self.duid, entry.dst.as_str(), &src_mtime, &dst_mtime)
                        .await?;
                }
                Opt::UPDATE | Opt::DOWNLOAD => {
                    try_copy(dst, &entry.dst, src, &entry.src).await?;
                    let src_mtime = src
                        .get_mtime(&entry.src)
                        .await?
                        .expect("get mtime")
                        .to_string();
                    let dst_mtime = match entry.dst_attr {
                        Some(t) => t,
                        None => dst.get_mtime(&entry.dst).await?.expect("get mtime"),
                    }
                    .to_string();
                    debug!(
                        "set db {} : {} = {}, {}",
                        self.duid,
                        entry.dst.as_str(),
                        src_mtime,
                        dst_mtime
                    );
                    self.ctx
                        .db
                        .set(self.duid, entry.dst.as_str(), &src_mtime, &dst_mtime)
                        .await?;
                }
                Opt::DELETEDST => {
                    self.ctx.db.del(self.duid, entry.dst.as_str()).await?;
                    dst.rm(&entry.dst).await?;
                }
                Opt::DELETESRC => {
                    self.ctx.db.del(self.duid, entry.src.as_str()).await?;
                    src.rm(&entry.src).await?;
                }
                _ => {}
            }
        }
        Ok(true)
    }
}
#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::{
        Context,
        db::{MultiDB, Sqlite},
        dev::User,
        interactor::TermInteractor,
    };

    use assert_fs::{TempDir, prelude::*};
    use dv_api::multi::Config;

    use super::Opt;
    use super::SyncContext3;

    fn mtime(path: &Path) -> u64 {
        path.metadata()
            .unwrap()
            .modified()
            .unwrap()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    ///Prepare a test environment with a source and destination directory.
    /// # Parameters
    ///
    /// - `src`: list of (name, content) pairs to create in the source directory
    /// - `dst`: list of (name, content) pairs to create in the destination directory
    async fn tenv(src: &[(&str, &str)], dst: &[(&str, &str)]) -> (Context, TempDir) {
        let interactor = TermInteractor::new().unwrap();
        let mut db = MultiDB::default();
        db.add_db(Sqlite::memory());
        let dir = TempDir::new().unwrap();
        let mut cfg = Config::default();
        cfg.set("mount", dir.to_string_lossy());
        let mut ctx = Context::new(db, None, interactor);
        ctx.add_user("this".to_string(), User::local(cfg).await.unwrap())
            .await
            .expect("add user");
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
        (ctx, dir)
    }
    #[tokio::test]
    async fn no_file() {
        let (ctx, _) = tenv(&[], &[]).await;
        let ctx = SyncContext3::new(&ctx, "this", "this", &[Opt::UPDATE, Opt::OVERWRITE]);
        let entries = ctx.scan("src/f0", "dst/f0").await;
        assert!(entries.is_err());
    }
    struct LocalFixtureResult {
        len: usize,
        opt: Opt,
        res: bool,
        content: Option<&'static str>,
    }
    impl LocalFixtureResult {
        fn new(len: usize, opt: Opt, res: bool, content: Option<&'static str>) -> Self {
            Self {
                len,
                opt,
                res,
                content,
            }
        }
        fn suc() -> LocalFixtureResult {
            LocalFixtureResult::new(1, Opt::UPLOAD, true, Some("f0"))
        }
        fn none() -> LocalFixtureResult {
            LocalFixtureResult::new(0, Opt::empty(), false, None)
        }
        fn no_local() -> LocalFixtureResult {
            LocalFixtureResult::new(1, Opt::DELETESRC, true, None)
        }
    }
    async fn local_fixture(ops: &[Opt], res: LocalFixtureResult) {
        let (ctx, dir) = tenv(&[("f0", "f0")], &[]).await;
        let ctx = SyncContext3::new(&ctx, "this", "this", ops);
        let entries = ctx.scan("src/f0", "dst/f0").await.unwrap();
        assert_eq!(entries.len(), res.len);
        if res.len == 0 {
            return;
        }
        assert_eq!(entries[0].opt, res.opt);
        let tres = ctx.execute(&entries).await.unwrap();
        assert_eq!(res.res, tres);
        let src = dir.child("src/f0");
        let dst = dir.child("dst/f0");
        let dst_db = ctx
            .ctx
            .db
            .get_as::<u64>("this", dst.to_str().unwrap())
            .await
            .unwrap();
        if let Some(content) = res.content {
            dst.assert(content);
            let (db_s, db_t) = dst_db.unwrap();
            assert_eq!(mtime(&src), db_s);
            assert_eq!(mtime(&dst), db_t);
        } else {
            assert!(!src.exists());
            assert!(dst_db.is_none());
        }
    }
    #[tokio::test]
    async fn local() {
        local_fixture(&[Opt::UPLOAD], LocalFixtureResult::suc()).await;
        local_fixture(&[Opt::UPLOAD, Opt::UPDATE], LocalFixtureResult::suc()).await;
        local_fixture(&[Opt::UPLOAD, Opt::OVERWRITE], LocalFixtureResult::suc()).await;
        local_fixture(&[Opt::UPDATE, Opt::empty()], LocalFixtureResult::none()).await;
        local_fixture(&[Opt::empty(), Opt::UPLOAD], LocalFixtureResult::none()).await;
        local_fixture(&[Opt::DELETESRC], LocalFixtureResult::no_local()).await;
        local_fixture(
            &[Opt::DELETESRC, Opt::UPLOAD],
            LocalFixtureResult::no_local(),
        )
        .await;
        local_fixture(&[Opt::empty()], LocalFixtureResult::none()).await;
        local_fixture(
            &[Opt::DELETESRC, Opt::empty()],
            LocalFixtureResult::no_local(),
        )
        .await;
        local_fixture(&[Opt::UPLOAD, Opt::DELETESRC], LocalFixtureResult::suc()).await;
        local_fixture(
            &[Opt::DELETESRC, Opt::UPLOAD, Opt::empty()],
            LocalFixtureResult::no_local(),
        )
        .await;
    }
    struct RemoteFixtureResult {
        len: usize,
        opt: Opt,
        res: bool,
        content: Option<&'static str>,
    }
    impl RemoteFixtureResult {
        fn new(len: usize, opt: Opt, res: bool, content: Option<&'static str>) -> Self {
            Self {
                len,
                opt,
                res,
                content,
            }
        }
        fn suc() -> RemoteFixtureResult {
            RemoteFixtureResult::new(1, Opt::DOWNLOAD, true, Some("f0"))
        }
        fn none() -> RemoteFixtureResult {
            RemoteFixtureResult::new(0, Opt::empty(), false, None)
        }
        fn no_remote() -> RemoteFixtureResult {
            RemoteFixtureResult::new(1, Opt::DELETEDST, true, None)
        }
    }
    async fn remote_fixture(ops: &[Opt], res: RemoteFixtureResult) {
        let (ctx, dir) = tenv(&[], &[("f0", "f0")]).await;
        let ctx = SyncContext3::new(&ctx, "this", "this", ops);
        let entries = ctx.scan("src/f0", "dst/f0").await.unwrap();
        assert_eq!(entries.len(), res.len);
        if res.len == 0 {
            return;
        }
        assert_eq!(entries[0].opt, res.opt);
        let tres = ctx.execute(&entries).await.unwrap();
        assert_eq!(res.res, tres);
        let src = dir.child("src/f0");
        let dst = dir.child("dst/f0");
        let dst_db = ctx
            .ctx
            .db
            .get_as::<u64>("this", dst.to_str().unwrap())
            .await
            .unwrap();

        if let Some(content) = res.content {
            src.assert(content);
            let (db_s, db_t) = dst_db.unwrap();
            assert_eq!(mtime(&src), db_s);
            assert_eq!(mtime(&dst), db_t);
        } else {
            assert!(!dst.exists());
            assert!(dst_db.is_none());
        }
    }
    #[tokio::test]
    async fn remote() {
        remote_fixture(&[Opt::DOWNLOAD], RemoteFixtureResult::suc()).await;
        remote_fixture(&[Opt::DOWNLOAD, Opt::UPDATE], RemoteFixtureResult::suc()).await;
        remote_fixture(&[Opt::DOWNLOAD, Opt::OVERWRITE], RemoteFixtureResult::suc()).await;
        remote_fixture(&[Opt::UPDATE, Opt::empty()], RemoteFixtureResult::none()).await;
        remote_fixture(&[Opt::empty(), Opt::DOWNLOAD], RemoteFixtureResult::none()).await;
        remote_fixture(&[Opt::DELETEDST], RemoteFixtureResult::no_remote()).await;
        remote_fixture(
            &[Opt::DELETEDST, Opt::DOWNLOAD],
            RemoteFixtureResult::no_remote(),
        )
        .await;
        remote_fixture(&[Opt::empty()], RemoteFixtureResult::none()).await;
        remote_fixture(
            &[Opt::DELETEDST, Opt::empty()],
            RemoteFixtureResult::no_remote(),
        )
        .await;
        remote_fixture(&[Opt::DOWNLOAD, Opt::DELETEDST], RemoteFixtureResult::suc()).await;
        remote_fixture(
            &[Opt::DELETEDST, Opt::DOWNLOAD, Opt::empty()],
            RemoteFixtureResult::no_remote(),
        )
        .await;
    }
    struct Bfr {
        len: usize,
        opt: Opt,
        res: bool,
        content: &'static str,
    }
    impl Bfr {
        fn new(len: usize, opt: Opt, res: bool, content: &'static str) -> Self {
            Self {
                len,
                opt,
                res,
                content,
            }
        }
        fn overwrite() -> Bfr {
            Bfr::new(1, Opt::OVERWRITE, true, "f0")
        }
        fn update() -> Bfr {
            Bfr::new(1, Opt::UPDATE, true, "f1")
        }
        fn none() -> Bfr {
            Bfr::new(0, Opt::empty(), false, "")
        }
    }
    async fn both_fixture(ops: &[Opt], db: u8, res: &Bfr, id: usize) {
        let (ctx, dir) = tenv(&[("f0", "f0")], &[("f0", "f1")]).await;
        ctx.interactor
            .log(format!("both_fixture case {}", id))
            .await;
        let src = dir.child("src/f0");
        let dst = dir.child("dst/f0");
        match db {
            1 => {
                ctx.db
                    .set("this", dst.to_str().unwrap(), "0", "0")
                    .await
                    .unwrap();
            }
            2 => {
                ctx.db
                    .set(
                        "this",
                        dst.to_str().unwrap(),
                        "0",
                        &mtime(&dir.child("dst/f0")).to_string(),
                    )
                    .await
                    .unwrap();
            }
            3 => {
                ctx.db
                    .set(
                        "this",
                        dst.to_str().unwrap(),
                        &mtime(&dir.child("src/f0")).to_string(),
                        "0",
                    )
                    .await
                    .unwrap();
            }
            4 => {
                ctx.db
                    .set(
                        "this",
                        dst.to_str().unwrap(),
                        &mtime(&dir.child("src/f0")).to_string(),
                        &mtime(&dir.child("dst/f0")).to_string(),
                    )
                    .await
                    .unwrap();
            }
            _ => {}
        }
        let ctx = SyncContext3::new(&ctx, "this", "this", ops);
        let entries = ctx.scan("src/f0", "dst/f0").await.unwrap();
        assert_eq!(entries.len(), res.len);
        if res.len == 0 {
            return;
        }
        assert_eq!(entries[0].opt, res.opt);
        let tres = ctx.execute(&entries).await.unwrap();
        assert_eq!(res.res, tres);
        src.assert(res.content);
        dst.assert(res.content);
        let dst_db = ctx
            .ctx
            .db
            .get_as::<u64>("this", dst.to_str().unwrap())
            .await
            .unwrap();
        let (db_s, db_t) = dst_db.unwrap();
        assert_eq!(mtime(&src), db_s);
        assert_eq!(mtime(&dst), db_t);
    }
    #[tokio::test]
    async fn both() {
        let tests = [
            (vec![Opt::OVERWRITE], 0, Bfr::overwrite()),
            (vec![Opt::UPDATE], 0, Bfr::update()),
            (vec![Opt::OVERWRITE, Opt::UPDATE], 0, Bfr::overwrite()),
            (vec![Opt::UPDATE, Opt::OVERWRITE], 0, Bfr::update()),
            (vec![Opt::OVERWRITE, Opt::empty()], 0, Bfr::overwrite()),
            (vec![Opt::UPDATE, Opt::empty()], 0, Bfr::update()),
            (vec![Opt::empty(), Opt::OVERWRITE], 0, Bfr::none()),
            (vec![Opt::empty(), Opt::UPDATE], 0, Bfr::none()),
            (vec![Opt::OVERWRITE], 1, Bfr::overwrite()),
            (vec![Opt::UPDATE], 1, Bfr::update()),
            (vec![Opt::OVERWRITE, Opt::UPDATE], 1, Bfr::overwrite()),
            (vec![Opt::UPDATE, Opt::OVERWRITE], 1, Bfr::update()),
            (vec![Opt::OVERWRITE, Opt::empty()], 1, Bfr::overwrite()),
            (vec![Opt::UPDATE, Opt::empty()], 1, Bfr::update()),
            (vec![Opt::empty(), Opt::OVERWRITE], 1, Bfr::none()),
            (vec![Opt::empty(), Opt::UPDATE], 1, Bfr::none()),
            (vec![Opt::OVERWRITE], 2, Bfr::overwrite()),
            (vec![Opt::UPDATE, Opt::empty()], 2, Bfr::none()),
            (vec![Opt::OVERWRITE, Opt::UPDATE], 2, Bfr::overwrite()),
            (vec![Opt::UPDATE, Opt::OVERWRITE], 2, Bfr::overwrite()),
            (vec![Opt::OVERWRITE, Opt::empty()], 2, Bfr::overwrite()),
            (vec![Opt::UPDATE, Opt::empty()], 2, Bfr::none()),
            (vec![Opt::empty(), Opt::OVERWRITE], 2, Bfr::none()),
            (vec![Opt::empty(), Opt::UPDATE], 2, Bfr::none()),
            (vec![Opt::OVERWRITE, Opt::empty()], 3, Bfr::none()),
            (vec![Opt::UPDATE], 3, Bfr::update()),
            (vec![Opt::OVERWRITE, Opt::UPDATE], 3, Bfr::update()),
            (vec![Opt::UPDATE, Opt::OVERWRITE], 3, Bfr::update()),
            (vec![Opt::OVERWRITE, Opt::empty()], 3, Bfr::none()),
            (vec![Opt::UPDATE, Opt::empty()], 3, Bfr::update()),
            (vec![Opt::empty(), Opt::OVERWRITE], 3, Bfr::none()),
            (vec![Opt::empty(), Opt::UPDATE], 3, Bfr::none()),
            (vec![Opt::OVERWRITE], 4, Bfr::none()),
            (vec![Opt::UPDATE], 4, Bfr::none()),
            (vec![Opt::OVERWRITE, Opt::UPDATE], 4, Bfr::none()),
            (vec![Opt::UPDATE, Opt::OVERWRITE], 4, Bfr::none()),
            (vec![Opt::OVERWRITE, Opt::empty()], 4, Bfr::none()),
            (vec![Opt::UPDATE, Opt::empty()], 4, Bfr::none()),
            (vec![Opt::empty(), Opt::OVERWRITE], 4, Bfr::none()),
            (vec![Opt::empty(), Opt::UPDATE], 4, Bfr::none()),
        ];
        for (i, (ops, db, res)) in tests.iter().enumerate() {
            both_fixture(ops, *db, res, i).await;
        }
    }
}
