use crate::User;

use super::dev::*;

pub async fn detect(_: &User) -> Result<Pm> {
    Ok(Pm::apk())
}
