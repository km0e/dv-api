use super::dev::*;
use os2::{LinuxOs, Os};
use tracing::info;

mod platform;

#[derive(Debug, Clone)]
pub struct Pm {
    pub name: &'static str,
    pub action: &'static [&'static [&'static [&'static str]; 2]],
}

macro_rules! generate_pm_structs {
    (
        $(
            $id:ident {
                name = $name:expr,
                install = [$($install_args:expr),* $(,)?], [$($install_confirm:expr),* $(,)?]
                update  = [$($update_args:expr),* $(,)?],  [$($update_confirm:expr),* $(,)?]
                upgrade = [$($upgrade_args:expr),* $(,)?], [$($upgrade_confirm:expr),* $(,)?]
            }
        )*
    ) => {


        $(
            pub const fn $id() -> Pm {
                Pm {
                    name: $name,
                    action: &[
                        &[
                            &[$($install_args),*],
                            &[$($install_confirm),*]
                        ],
                        &[
                            &[$($update_args),*],
                            &[$($update_confirm),*]
                        ],
                        &[
                            &[$($upgrade_args),*],
                            &[$($upgrade_confirm),*]
                        ],
                    ],
                }
            }
        )*
    };
}
impl Pm {
    const INSTALL: usize = 0;
    const UPDATE: usize = 1;
    const UPGRADE: usize = 2;
    generate_pm_structs! {
        apk {
            name = "apk",
            install = ["add"], []
            update  = ["update"], []
            upgrade = ["upgrade"], []
        }
        apt {
            name = "apt",
            install = ["install"], ["-y"]
            update  = ["update"], []
            upgrade = ["upgrade"], ["-y"]
        }
        pacman {
            name = "pacman",
            install = ["-S" ], ["--noconfirm"]
            update  = ["-Sy"], []
            upgrade = ["-Su"], []
        }
        yay {
            name = "yay",
            install = ["-S" ], ["--noconfirm"]
            update  = ["-Sy"], []
            upgrade = ["-Su"], []
        }
        paru {
            name = "paru",
            install = ["-S"], ["--noconfirm"]
            update  = ["-Sy"], []
            upgrade = ["-Su"], []
        }
        winget {
            name = "winget",
            install = ["install"],
                [
                    "--accept-source-agreements",
                    "--accept-package-agreements"
                ]
            update  = [], []
            upgrade = [], []
        }
        unknown {
            name = "unknown",
            install = [], []
            update  = [], []
            upgrade = [], []
        }
    }
}

impl Pm {
    pub async fn detect(u: &User, os: &Os) -> Result<Self> {
        info!("new_am os:{:?}", os);
        match os {
            Os::Linux(os) => match os {
                LinuxOs::Arch => platform::arch::detect(u).await,
                LinuxOs::Manjaro => platform::arch::detect(u).await,
                LinuxOs::Debian => platform::debian::detect(u).await,
                LinuxOs::Alpine => platform::alpine::detect(u).await,
                LinuxOs::Ubuntu => platform::ubuntu::detect(u).await,
                _ => bail!("Unknown LinuxOs {:?}", os),
            },
            Os::Windows => platform::windows::detect(u).await,
            _ => Ok(Self::unknown()),
        }
    }
}

impl Pm {
    async fn action(
        &self,
        ctx: &Context,
        uid: &str,
        idx: usize,
        packages: impl Iterator<Item = &str> + Send,
        confirm: bool,
    ) -> Result<bool> {
        let action = self.action[idx];
        let args = action[0].iter().copied().chain(if confirm {
            action[1].iter().copied()
        } else {
            <&[&str]>::default().iter().copied()
        });
        let script = Script::Split {
            program: self.name,
            args: Box::new(args.chain(packages)),
        };
        if ctx.pty(uid, script).await? != 0 {
            bail!("{} failed", idx);
        }
        Ok(true)
    }
    //TODO:better generate this code
    pub async fn install(&self, ctx: &Context, uid: &str, packages: &str, y: bool) -> Result<bool> {
        info!("{} packages: {}", self.name, packages);
        let packages = packages.split_whitespace();
        self.action(ctx, uid, Pm::INSTALL, packages, y).await
    }

    pub async fn update(&self, ctx: &Context, uid: &str, confirm: bool) -> Result<bool> {
        info!("{} update", self.name);
        self.action(ctx, uid, Pm::UPDATE, std::iter::empty::<&str>(), confirm)
            .await
    }
    pub async fn upgrade(&self, ctx: &Context, uid: &str, packages: &str, y: bool) -> Result<bool> {
        info!("{} upgrade packages: {}", self.name, packages);
        let packages = packages.split_whitespace();
        self.action(ctx, uid, Pm::UPGRADE, packages, y).await
    }
}
