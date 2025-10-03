mod dev {
    pub use super::super::dev::*;
    pub use super::DotConfig;
    pub use os2::Os;
}
use dev::*;

use schema::{SchemaStorage, SerdeSchemaStorage};
use source::Source;
use std::{collections::HashMap, ops::Deref};
use tokio::io::AsyncReadExt;
use tracing::{debug, trace};

use crate::ops::{
    SyncEntry, SyncOpt,
    dotutils::{schema::Schema, source::Op},
};

mod schema;
mod source;

#[derive(Debug, Default, Clone)]
pub struct DotConfig {
    pub name: String,
    pub copy_action: Vec<SyncOpt>,
}

impl DotConfig {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            copy_action: Vec::new(),
        }
    }
}

pub struct DotUtil<T: AsRefContext> {
    pub copy_action: Vec<SyncOpt>,
    schema: HashMap<String, SchemaStorage<Vec<String>>>,
    source: HashMap<String, Source>,
    pub ctx: T,
}

pub struct Entry {
    pub src: String,
    pub dst: String,
    pub entries: Vec<SyncEntry>,
}

impl<T: AsRefContext> DotUtil<T> {
    pub fn new(ctx: T, copy_action: Vec<SyncOpt>) -> Self {
        Self {
            copy_action,
            schema: HashMap::new(),
            source: HashMap::new(),
            ctx,
        }
    }
    pub async fn add_schema(&mut self, user: &str, path: &str) -> Result<()> {
        let ctx = self.ctx.as_ref();
        let user = ctx.get_user(user)?;
        let mut file = user.open(path, OpenFlags::READ).await?;
        let mut content = String::new();
        file.read_to_string(&mut content).await?;
        let schema: SerdeSchemaStorage<Vec<String>> = toml::from_str(&content)?;
        self.schema
            .insert(schema.name.clone(), schema.into_storage());
        Ok(())
    }
    pub async fn add_source(&mut self, user: &str, path: &str) -> Result<()> {
        let ctx = self.ctx.as_ref();
        let u = ctx.get_user(user)?;
        let cfg_path = U8Path::new(path).join("config.toml");
        let mut file = u.open(&cfg_path, OpenFlags::READ).await?;
        let mut content = String::new();
        file.read_to_string(&mut content).await?;
        let schema: SerdeSchemaStorage<String> = toml::from_str(&content)?;
        for (name, schema) in &schema.schema {
            trace!("source {}: {:?}", name, schema);
        }
        self.source.insert(
            schema.name.clone(),
            Source::new(user, path, schema.into_storage()),
        );
        Ok(())
    }
    async fn each(
        &'_ self,
        apps: Vec<DotConfig>,
        dst: &str,
    ) -> Result<impl Iterator<Item = Result<(Op<'_>, DotConfig, &'_ Schema<Vec<String>>)>> + '_>
    {
        let os = self.ctx.as_ref().get_user(dst)?.os();
        Ok(apps.into_iter().map(move |mut opt| {
            debug!("search app {}", opt.name);
            let Some(source) = self
                .source
                .iter()
                .find_map(|(_, source)| source.search(&opt.name, os))
            else {
                bail!("app {} not found in source config", opt.name)
            };
            let Some(schema) = self
                .schema
                .values()
                .find_map(|s| s.search_compatible(os, &opt.name))
            else {
                bail!("app {} not found in schema", opt.name)
            };
            opt.copy_action.extend_from_slice(&self.copy_action);
            Ok((source, opt, schema))
        }))
    }
    pub async fn sync(&self, apps: Vec<DotConfig>, dst: &str) -> Result<Vec<Entry>> {
        let ctx = self.ctx.as_ref();
        let mut entries = Vec::new();
        for v in self.each(apps, dst).await? {
            let (source, opt, schema) = v?;
            debug!("sync app {}", opt.name);
            entries.push(Entry {
                src: source.user.to_string(),
                dst: dst.to_string(),
                entries: source.sync(ctx.deref(), opt.clone(), dst, schema).await?,
            });
        }
        Ok(entries)
    }
    pub async fn upload(&self, apps: Vec<DotConfig>, dst: &str) -> Result<Vec<Entry>> {
        let ctx = self.ctx.as_ref();
        let mut entries = Vec::new();
        for v in self.each(apps, dst).await? {
            let (source, opt, schema) = v?;
            debug!("upload app {}", opt.name);
            entries.push(Entry {
                src: dst.to_string(),
                dst: source.user.to_string(),
                entries: source.upload(ctx.deref(), opt.clone(), dst, schema).await?,
            });
        }
        Ok(entries)
    }
}
