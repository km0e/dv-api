use std::str::FromStr;

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
    #[strum(serialize = "alpine")]
    Alpine,
    #[strum(serialize = "arch")]
    Arch,
    #[strum(serialize = "debian")]
    Debian,
    #[strum(serialize = "fedora")]
    Fedora,
    #[strum(serialize = "manjaro")]
    Manjaro,
    #[strum(serialize = "ubuntu")]
    Ubuntu,
}

impl Linux {
    pub fn detect() -> Self {
        for path in ["/etc/os-release", "/usr/lib/os-release"] {
            let Ok(content) = std::fs::read_to_string(path) else {
                continue;
            };
            for line in content.lines() {
                let Some((key, value)) = line.split_once('=') else {
                    continue;
                };
                match key.trim() {
                    "ID" => {
                        return Linux::from_str(value.trim().trim_matches('"'))
                            .unwrap_or(Linux::Unknown);
                    }
                    _ => continue,
                }
            }
        }
        Linux::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_str() {
        assert_eq!(Linux::from_str("manjaro").unwrap(), Linux::Manjaro);
        assert_eq!(Linux::from_str("alpine").unwrap(), Linux::Alpine);
        assert_eq!(Linux::from_str("debian").unwrap(), Linux::Debian);
        assert_eq!(Linux::from_str("ubuntu").unwrap(), Linux::Ubuntu);
        assert_eq!(Linux::from_str("linux").unwrap(), Linux::Unknown);
    }
}
