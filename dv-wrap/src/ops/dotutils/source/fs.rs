use dv_api::whatever;
use tracing::{debug, warn};

use super::dev::*;

use super::{Source, SourceAction};

pub struct FileSystemSource {
    pub user: String,
    pub path: U8PathBuf,
    pub storage: SourceStorage,
}
impl FileSystemSource {
    pub fn new(
        user: impl Into<String>,
        path: impl Into<U8PathBuf>,
        storage: SourceStorage,
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
    pub source: &'a SourceSchema,
}

#[async_trait::async_trait]
impl SourceAction for Op<'_> {
    async fn sync(
        &self,
        ctx: &Context,
        opt: DotConfig,
        dst: &str,
        schema: &AppSchema,
    ) -> Result<()> {
        let copy_ctx = crate::ops::SyncContext::new(ctx, self.user, dst, Some(&opt.copy_action))?;
        for (name, cfg) in &self.source.paths {
            let Some(dst_cfg) = schema.paths.get(name) else {
                continue;
            };
            let src_path = self.path.join(cfg);
            if !copy_ctx.src.exist(&src_path).await? {
                warn!("source path not exist: {}", src_path);
                break;
            }
            let mut success = false;
            for dst_path in dst_cfg {
                debug!("try to sync {} to {}", src_path, dst_path);
                if copy_ctx.sync(&src_path, &dst_path).await.is_ok() {
                    success = true;
                    break;
                }
            }
            if !success {
                whatever!("app {} not found in destination config", name)
            }
        }
        Ok(())
    }
    async fn upload(
        &self,
        ctx: &Context,
        opt: DotConfig,
        src: &str,
        schema: &AppSchema,
    ) -> Result<()> {
        let copy_ctx = crate::ops::SyncContext::new(ctx, src, self.user, Some(&opt.copy_action))?;
        for (name, cfg) in &schema.paths {
            let Some(dst_cfg) = self.source.paths.get(name) else {
                continue;
            };
            let dst_path = self.path.join(dst_cfg);
            let mut eq = Vec::new();
            for src_path in cfg {
                let sp = src_path.as_ref();
                if copy_ctx.src.exist(sp).await.is_ok_and(|exists| exists) {
                    let Err(e) = copy_ctx.sync(&sp, &dst_path).await else {
                        break;
                    };
                    eq.push((sp.to_string(), e));
                }
            }
            if eq.len() == cfg.len() {
                whatever!(
                    "all source paths failed to upload to {}: {:?}",
                    dst_path,
                    eq
                )
            }
        }
        Ok(())
    }
}

impl Source for FileSystemSource {
    fn search<'a>(&'a self, name: &str, os: Os) -> Option<Box<dyn 'a + SourceAction>> {
        Some(Box::new(Op {
            user: &self.user,
            path: &self.path,
            source: self.storage.search_compatible(os, name)?,
        }))
    }
}
