#[cfg(test)]
pub mod tests {

    use crate::{cache::SqliteCache, context::Context, dev::User, interactor::TermInteractor};
    use std::collections::HashMap;

    pub struct TestDv {
        pub dry_run: bool,
        pub users: HashMap<String, User>,
        pub cache: SqliteCache,
        pub interactor: TermInteractor,
    }
    impl TestDv {
        pub fn context(&self) -> Context<'_> {
            Context::new(self.dry_run, &self.cache, &self.interactor, &self.users)
        }
    }
}
