use super::dev::*;

pub async fn detect(_: &BoxedUser) -> Result<Pm> {
    Ok(Pm::Apk)
}
