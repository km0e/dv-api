mod dev {
    pub use super::super::dev::*;
    pub use super::super::schema::*;
}
use dev::*;

use os2::Os;
use tracing::debug;

use crate::ops::sync::Entry;

pub struct Source {
    pub user: String,
    pub path: U8PathBuf,
    pub storage: SchemaStorage<String>,
}

impl Source {
    pub fn new(
        user: impl Into<String>,
        path: impl Into<U8PathBuf>,
        storage: SchemaStorage<String>,
    ) -> Self {
        Self {
            user: user.into(),
            path: path.into(),
            storage,
        }
    }
}

pub struct Op<'a> {
    pub user: &'a String,
    pub path: &'a U8PathBuf,
    pub source: &'a Schema<String>,
}

impl Op<'_> {
    pub async fn sync(
        &self,
        ctx: &Context,
        opt: DotConfig,
        dst: &str,
        schema: &Schema<Vec<String>>,
    ) -> Result<Vec<Entry>> {
        let src = ctx.get_user(self.user)?;
        let copy_ctx = crate::ops::SyncContext3::new(ctx, self.user, dst, &opt.copy_action);
        let mut entries = Vec::new();
        for (name, src_path) in &self.source.paths {
            let Some(dst_paths) = schema.paths.get(name) else {
                continue;
            };
            let src_path = self.path.join(src_path);
            if !src.exist(&src_path).await.is_ok_and(|b| b) {
                debug!("source path {:?} not exists, skip", src_path);
                continue;
            }
            let mut suc = false;
            for dst in dst_paths.iter() {
                if let Ok(e) = copy_ctx.scan(&src_path, dst).await {
                    entries.extend(e);
                    suc = true;
                    break;
                }
            }
            if !suc {
                bail!("no valid destination for source path {:?}", src_path);
            }
        }
        Ok(entries)
    }
    pub async fn upload(
        &self,
        ctx: &Context,
        opt: DotConfig,
        src: &str,
        schema: &Schema<Vec<String>>,
    ) -> Result<Vec<Entry>> {
        let copy_ctx = crate::ops::SyncContext3::new(ctx, src, self.user, &opt.copy_action);
        let src = ctx.get_user(src)?;
        let mut entries = Vec::new();
        for (name, src_paths) in &schema.paths {
            let Some(dst_path) = self.source.paths.get(name) else {
                continue;
            };
            let dst_path = self.path.join(dst_path);
            let mut suc = false;
            for src_path in src_paths.iter() {
                if src.exist(src_path).await.is_ok_and(|b| b) {
                    debug!("scan upload from {:?} to {:?}", src_path, dst_path);
                    entries.extend(copy_ctx.scan(&src_path, &dst_path).await?);
                    suc = true;
                    break;
                }
            }
            if !suc {
                bail!("no valid source for destination path {:?}", dst_path);
            }
        }
        Ok(entries)
    }
}

impl Source {
    pub fn search(&'_ self, name: &str, os: Os) -> Option<Op<'_>> {
        Some(Op {
            user: &self.user,
            path: &self.path,
            source: self.storage.search_compatible(os, name)?,
        })
    }
}
