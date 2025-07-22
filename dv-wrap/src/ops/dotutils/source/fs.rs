use dv_api::whatever;

use super::dev::*;

use super::Source;
use super::SourceAction;

pub struct FileSystemSource {
    pub user: String,
    pub path: U8PathBuf,
    pub storage: SchemaStorage,
}
impl FileSystemSource {
    pub fn new(
        user: impl Into<String>,
        path: impl Into<U8PathBuf>,
        storage: SchemaStorage,
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
    pub source: &'a AppSchema,
}

#[async_trait::async_trait]
impl SourceAction for Op<'_> {
    async fn sync(
        &self,
        ctx: Context<'_>,
        opt: &DotConfig,
        dst: &str,
        schema: &AppSchema,
    ) -> Result<()> {
        let copy_ctx = crate::ops::CopyContext::new(ctx, self.user, dst, Some(&opt.copy_action))?;
        for (name, cfg) in &self.source.paths {
            let Some(dst_cfg) = schema.paths.get(name) else {
                continue;
            };
            let mut src_path = self.path.clone();
            for sub in cfg {
                src_path.push(sub);
                if copy_ctx.src.exist(&src_path).await? {
                    break;
                }
                src_path.clone_from(self.path);
            }
            if src_path.as_str() == self.path.as_str() {
                whatever!("app {} not found in source config", name)
            }
            let mut success = false;
            for dst_path in dst_cfg {
                if copy_ctx
                    .copy(src_path.as_str(), dst_path.as_str())
                    .await
                    .is_ok()
                {
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
}

impl Source for FileSystemSource {
    fn try_sync<'a>(&'a self, name: &str, os: Os) -> Option<Box<dyn 'a + SourceAction>> {
        Some(Box::new(Op {
            user: &self.user,
            path: &self.path,
            source: self.storage.search_compatible(os, name)?,
        }))
    }
}
