use super::dev::*;

pub async fn detect(_u: &BoxedUser) -> Result<Pm> {
    Ok(Pm::WinGet)
}
