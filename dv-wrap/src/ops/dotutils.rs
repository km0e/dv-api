mod dev {
    pub use super::super::dev::*;
    pub use super::DotConfig;
    pub use os2::Os;
}
use dev::*;

use schema::{SchemaStorage, SerdeSchemaStorage};
use source::{FileSystemSource, Source};
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
    copy_action: String,
    schema: SchemaStorage,
    source: HashMap<String, Box<dyn Source>>,
}

impl DotUtil {
    pub fn new(copy_action: Option<String>) -> Self {
        Self {
            copy_action: copy_action.unwrap_or_default(),
            schema: SchemaStorage::default(),
            source: HashMap::new(),
        }
    }
    pub async fn add_schema(&mut self, ctx: Context<'_>, user: &str, path: &str) -> Result<()> {
        let user = ctx.get_user(user)?;
        let mut file = user.open(path.into(), OpenFlags::READ).await?;
        let mut content = String::new();
        file.read_to_string(&mut content).await?;
        let schema: SerdeSchemaStorage = toml::from_str(&content)?;
        self.schema = schema.into_schema_storage();
        Ok(())
    }
    pub async fn add_source(&mut self, ctx: Context<'_>, user: &str, path: &str) -> Result<()> {
        let u = ctx.get_user(user)?;
        let cfg_path = U8Path::new(path).join("config.toml");
        let mut file = u.open(&cfg_path, OpenFlags::READ).await?;
        let mut content = String::new();
        file.read_to_string(&mut content).await?;
        let schema: SerdeSchemaStorage = toml::from_str(&content)?;
        for (name, schema) in &schema.schemas {
            trace!("schema {}: {:?}", name, schema);
        }
        self.source.insert(
            schema.name.clone(),
            Box::new(FileSystemSource::new(
                user,
                path,
                schema.into_schema_storage(),
            )),
        );
        Ok(())
    }
    pub async fn sync(&self, ctx: Context<'_>, apps: Vec<DotConfig>, dst: &str) -> Result<()> {
        let dst_u = ctx.get_user(dst)?;
        for mut opt in apps {
            debug!("sync app {}", opt.name);
            let Some(source) = self
                .source
                .iter()
                .find_map(|(_, source)| source.try_sync(&opt.name, dst_u.os()))
            else {
                whatever!("app {} not found in source config", opt.name)
            };
            let Some(schema) = self.schema.search_compatible(dst_u.os(), &opt.name) else {
                whatever!("app {} not found in schema", opt.name)
            };
            opt.copy_action = opt.copy_action + &self.copy_action;
            source.sync(ctx, &opt, dst.as_ref(), schema).await?;
        }
        Ok(())
    }
}
