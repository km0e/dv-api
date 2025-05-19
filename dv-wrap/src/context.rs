use super::dev::*;
use crate::{cache::SqliteCache, interactor::TermInteractor};
use std::collections::HashMap;

#[derive(Clone, Copy)]
pub struct Context<'a> {
    pub dry_run: bool,
    pub cache: &'a SqliteCache,
    pub interactor: &'a TermInteractor,
    users: &'a HashMap<String, User>,
}

macro_rules! action {
    ($ctx:expr, $suc:expr, $fmt:expr, $($arg:tt)*) => {
        $ctx.interactor.log(format!(concat!("[{}] {} ",$fmt), if $ctx.dry_run { "n" } else { "a" }, if $suc { "exec" } else { "skip" }, $($arg)*)).await;
    };
}

pub(crate) use action;
use dv_api::whatever;

impl<'s> Context<'s> {
    pub fn new<'a>(
        dry_run: bool,
        cache: &'a SqliteCache,
        interactor: &'a TermInteractor,
        users: &'a HashMap<String, User>,
    ) -> Context<'a> {
        Context {
            dry_run,
            cache,
            interactor,
            users,
        }
    }
    pub fn get_user(&self, uid: impl AsRef<str>) -> Result<&'s User> {
        let uid = uid.as_ref();
        match self.users.get(uid) {
            Some(user) => Ok(user),
            None => {
                whatever!("user {} not found", uid)
            }
        }
    }
}
