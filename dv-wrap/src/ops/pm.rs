use std::collections::HashMap;

use os2::{LinuxOs, Os};
use strum::{Display, EnumIs, EnumString};
mod dev {
    pub use super::super::dev::*;
    pub use super::Pm;
    pub use super::support::*;
}
use dev::*;
use tracing::{info, warn};

mod platform;
mod support;

#[derive(Debug, Clone, Copy, Display, Hash, PartialEq, Eq, EnumIs, EnumString)]
#[strum(serialize_all = "snake_case")]
pub enum Pm {
    #[strum(serialize = "apk")]
    Apk,
    #[strum(serialize = "apt")]
    Apt,
    #[strum(serialize = "pacman")]
    Pacman,
    #[strum(serialize = "yay")]
    Yay,
    #[strum(serialize = "paru")]
    Paru,
    #[strum(serialize = "winget")]
    WinGet,
    Unknown,
}

impl Pm {
    pub async fn detect(u: &User, os: &Os) -> Result<Self> {
        info!("new_am os:{:?}", os);
        match os {
            Os::Linux(os) => match os {
                LinuxOs::Manjaro => platform::manjaro::detect(u).await,
                LinuxOs::Debian => platform::debian::detect(u).await,
                LinuxOs::Alpine => platform::alpine::detect(u).await,
                LinuxOs::Ubuntu => platform::ubuntu::detect(u).await,
                LinuxOs::Unknown => whatever!("Unknown LinuxOs"),
            },
            Os::Windows => platform::windows::detect(u).await,
            _ => Ok(Self::Unknown),
        }
    }
    pub async fn install(
        &self,
        u: &User,
        interactor: &DynInteractor,
        package: &str,
    ) -> Result<bool> {
        match self {
            Pm::Apk => apk::install(u, interactor, package).await,
            Pm::Apt => apt::install(u, interactor, package).await,
            Pm::Pacman => pacman::install(u, interactor, package).await,
            Pm::Yay => yay::install(u, interactor, package).await,
            Pm::Paru => paru::install(u, interactor, package).await,
            Pm::WinGet => winget::install(u, interactor, package).await,
            Pm::Unknown => whatever!("Unknown Pm"),
        }
    }
}

#[derive(Debug, Default)]
pub struct Package<'a> {
    pub pm: HashMap<Pm, &'a str>,
}

impl std::fmt::Display for Package<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            self.pm
                .iter()
                .map(|(k, v)| format!("{}: {}", k, v))
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

impl Package<'_> {
    pub async fn install(&self, ctx: Context<'_>, uid: &str, pm: &Pm) -> Result<bool> {
        let user = ctx.get_user(uid)?;
        let res = ctx.dry_run || {
            if let Some(package) = self.pm.get(pm) {
                pm.install(user, ctx.interactor, package).await
            } else {
                warn!("No package found for {:?}", pm);
                Ok(false)
            }
        }?;
        action!(ctx, res, "install {}", self);
        Ok(res)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn pm_from_str() {
        let pm: Pm = "apk".parse().unwrap();
        assert_eq!(pm, Pm::Apk);
        let pm: Pm = "apt".parse().unwrap();
        assert_eq!(pm, Pm::Apt);
        let pm: Pm = "pacman".parse().unwrap();
        assert_eq!(pm, Pm::Pacman);
        let pm: Pm = "yay".parse().unwrap();
        assert_eq!(pm, Pm::Yay);
        let pm: Pm = "paru".parse().unwrap();
        assert_eq!(pm, Pm::Paru);
        let pm: Pm = "winget".parse().unwrap();
        assert_eq!(pm, Pm::WinGet);
    }
}
