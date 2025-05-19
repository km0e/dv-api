mod system;
mod user;

pub enum AutoX {
    System(system::AutoX),
    User(user::AutoX),
}

impl AutoX {
    pub async fn new(is_system: bool) -> Result<Self, Error> {
        let x = if is_system {
            system::AutoX::new().map(Self::System)?
        } else {
            user::AutoX::new().map(Self::User)?
        };
        Ok(x)
    }
    pub async fn setup(&self, name: impl AsRef<str>, cmd: impl AsRef<str>) -> Result<(), Error> {
        match self {
            Self::System(auto) => auto.setup(name, cmd)?,
            Self::User(auto) => auto.setup(name, cmd)?,
        }
        Ok(())
    }
    pub async fn destroy(&self, name: impl AsRef<str>) -> Result<(), Error> {
        match self {
            Self::System(auto) => auto.destroy(name)?,
            Self::User(auto) => auto.destroy(name)?,
        }
        Ok(())
    }
    pub async fn reload(&self, name: impl AsRef<str>) -> Result<(), Error> {
        match self {
            Self::System(auto) => auto.reload(name)?,
            Self::User(auto) => auto.reload(name)?,
        }
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    User(#[from] auto_launch::Error),
    #[error(transparent)]
    System(#[from] windows::core::Error),
}
