use strum::{AsRefStr, Display, EnumIs, EnumString};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "rune", derive(rune::Any))]
#[derive(
    Default, Hash, Eq, Debug, Clone, Copy, AsRefStr, Display, EnumIs, EnumString, PartialEq,
)]
#[strum(serialize_all = "snake_case")]
pub enum Linux {
    #[default]
    #[strum(serialize = "linux")]
    Unknown,
    #[strum(serialize = "manjaro")]
    Manjaro,
    #[strum(serialize = "alpine")]
    Alpine,
    #[strum(serialize = "debian")]
    Debian,
    #[strum(serialize = "ubuntu")]
    Ubuntu,
}
