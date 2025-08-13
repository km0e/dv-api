mod dev {
    pub use super::super::dev::*;
    pub use super::super::schema::*;
}
use dev::*;

mod fs;
pub use fs::FileSystemSource;
use os2::Os;

pub trait Source {
    fn search<'a>(&'a self, name: &str, os: Os) -> Option<Box<dyn 'a + SourceAction>>;
}

#[async_trait::async_trait]
pub trait SourceAction {
    async fn sync(
        &self,
        ctx: &Context,
        opt: DotConfig,
        dst: &str,
        schema: &AppSchema,
    ) -> Result<()>;
    async fn upload(
        &self,
        ctx: &Context,
        opt: DotConfig,
        dst: &str,
        schema: &AppSchema,
    ) -> Result<()>;
}
