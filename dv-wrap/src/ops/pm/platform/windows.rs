use super::dev::*;

pub async fn detect(_u: &User) -> Result<Pm> {
    Ok(Pm::WinGet)
}
