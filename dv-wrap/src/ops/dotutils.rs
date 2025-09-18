mod dev {
    pub use super::super::dev::*;
    pub use super::DotConfig;
    pub use os2::Os;
}
use dev::*;

use schema::{AppSchema, SchemaStorage, SerdeSchemaStorage, SerdeSourceStorage};
use source::{FileSystemSource, Source, SourceAction};
use std::collections::HashMap;
use tokio::io::AsyncReadExt;
use tracing::{debug, trace};

mod schema;
mod source;

#[derive(Debug, Default, Clone)]
pub struct DotConfig {
    pub name: String,
    pub copy_action: String,
}

impl DotConfig {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            copy_action: String::new(),
        }
    }
}

#[derive(Default)]
pub struct DotUtil {
    pub copy_action: String,
    schema: HashMap<String, SchemaStorage>,
    source: HashMap<String, Box<dyn Source>>,
}

impl DotUtil {
    pub fn new(copy_action: Option<String>) -> Self {
        Self {
            copy_action: copy_action.unwrap_or_default(),
            ..Default::default()
        }
    }
    pub async fn add_schema(&mut self, ctx: &Context, user: &str, path: &str) -> Result<()> {
        let mut content = String::new();
        let user = ctx.get_user(user)?;
        let mut file = user.open(path, OpenFlags::READ).await?;
        file.read_to_string(&mut content).await?;
        let schema: SerdeSchemaStorage = toml::from_str(&content)?;
        self.schema
            .insert(schema.name.clone(), schema.into_schema_storage());
        Ok(())
    }
    pub async fn add_source(&mut self, ctx: &Context, user: &str, path: &str) -> Result<()> {
        let u = ctx.get_user(user)?;
        let cfg_path = U8Path::new(path).join("config.toml");
        let mut file = u.open(&cfg_path, OpenFlags::READ).await?;
        let mut content = String::new();
        file.read_to_string(&mut content).await?;
        let schema: SerdeSourceStorage = toml::from_str(&content)?;
        for (name, schema) in &schema.schema {
            trace!("source {}: {:?}", name, schema);
        }
        self.source.insert(
            schema.name.clone(),
            Box::new(FileSystemSource::new(
                user,
                path,
                schema.into_source_storage(),
            )),
        );
        Ok(())
    }
    async fn each<'a, 'b: 'a>(
        &'a self,
        ctx: &'b Context,
        apps: Vec<DotConfig>,
        dst: &str,
    ) -> Result<
        impl Iterator<Item = Result<(Box<dyn SourceAction + 'a>, DotConfig, &'a AppSchema)>> + 'a,
    > {
        let dst = ctx.get_user(dst)?;
        Ok(apps.into_iter().map(move |mut opt| {
            debug!("search app {}", opt.name);
            let Some(source) = self
                .source
                .iter()
                .find_map(|(_, source)| source.search(&opt.name, dst.os()))
            else {
                whatever!("app {} not found in source config", opt.name)
            };
            let Some(schema) = self
                .schema
                .values()
                .find_map(|s| s.search_compatible(dst.os(), &opt.name))
            else {
                whatever!("app {} not found in schema", opt.name)
            };
            opt.copy_action = opt.copy_action + &self.copy_action;
            Ok((source, opt, schema))
        }))
    }
    pub async fn sync(&self, ctx: &Context, apps: Vec<DotConfig>, dst: &str) -> Result<()> {
        for v in self.each(ctx, apps, dst).await? {
            let (source, opt, schema) = v?;
            debug!("sync app {}", opt.name);
            source.sync(ctx, opt, dst, schema).await?;
        }
        Ok(())
    }
    pub async fn upload(&self, ctx: &Context, apps: Vec<DotConfig>, dst: &str) -> Result<()> {
        for v in self.each(ctx, apps, dst).await? {
            let (source, opt, schema) = v?;
            debug!("upload app {}", opt.name);
            source.upload(ctx, opt, dst, schema).await?;
        }
        Ok(())
    }
}
