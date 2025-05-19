use os2::Os;
use tracing::debug;

use crate::{User, ops::Pm};

#[derive(Debug, Clone, Copy)]
pub struct DeviceInfo {
    pub os: Os,
    pub pm: Pm,
}

impl DeviceInfo {
    pub async fn detect(user: &User, os: impl Into<Os>) -> crate::error::Result<Self> {
        let os = os.into();
        let pm = Pm::detect(user, &os).await?;
        debug!("Detected device: os: {:?}, pm: {:?}", os, pm);
        Ok(Self { os, pm })
    }
}
