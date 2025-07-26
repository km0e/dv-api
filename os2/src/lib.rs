#![doc = include_str!("../README.md")]

mod linux;
pub use linux::Linux as LinuxOs;

use std::str::FromStr;
use strum::{AsRefStr, Display, EnumIs};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "rune", derive(rune::Any))]
#[derive(Debug, Hash, Eq, Clone, Copy, Default, Display, EnumIs, AsRefStr, PartialEq)]
#[strum(serialize_all = "snake_case")]
pub enum Os {
    #[default]
    #[strum(serialize = "unknown")]
    Unknown,
    #[strum(transparent)]
    Linux(LinuxOs),
    #[strum(serialize = "windows")]
    Windows,
    #[strum(serialize = "macos")]
    MacOs,
    #[strum(serialize = "unix")]
    Unix,
}

impl Os {
    pub fn linux() -> Self {
        Os::Linux(LinuxOs::default())
    }
    /// Confirms if the current operating system is compatible with another.
    /// # Examples
    /// ```
    /// use os2::Os;
    /// let os = Os::Linux(os2::LinuxOs::Ubuntu);
    /// assert!(os.compatible(&Os::Linux(os2::LinuxOs::Ubuntu)));
    /// assert!(!os.compatible(&Os::Linux(os2::LinuxOs::Arch)));
    /// assert!(os.compatible(&Os::Linux(os2::LinuxOs::Unknown)));
    /// assert!(os.compatible(&Os::Unix));
    /// ```
    pub fn compatible(&self, other: &Os) -> bool {
        match other {
            Os::Unknown => true,
            Os::Linux(LinuxOs::Unknown) => matches!(self, Os::Linux(_)),
            Os::Linux(linux) => match self {
                Os::Linux(other_linux) => linux == other_linux,
                _ => false,
            },
            Os::Windows => self == &Os::Windows,
            Os::MacOs => self == &Os::MacOs,
            Os::Unix => matches!(self, Os::Linux(_) | Os::Unix | Os::MacOs),
        }
    }
    /// Returns the next compatible operating system.
    /// # Examples
    /// ```
    /// use os2::Os;
    /// let os = Os::MacOs;
    /// assert_eq!(os.next_compatible(), Some(Os::Unix));
    /// let os = Os::Linux(os2::LinuxOs::Ubuntu);
    /// assert_eq!(os.next_compatible(), Some(Os::Linux(os2::LinuxOs::Unknown)));
    /// let os = Os::Unknown;
    /// assert_eq!(os.next_compatible(), None);
    /// ```
    pub fn next_compatible(&self) -> Option<Os> {
        match self {
            Os::Unknown => None,
            Os::Linux(linux) if linux.is_unknown() => Some(Os::Unix),
            Os::Linux(_) => Some(Os::linux()),
            Os::Windows => Some(Os::Unknown), //NOTE:Need to be confirmed
            Os::MacOs => Some(Os::Unix),      //NOTE:Need to be confirmed
            Os::Unix => Some(Os::Unknown),    //NOTE:Need to be confirmed
        }
    }
}

impl From<&str> for Os {
    fn from(s: &str) -> Self {
        if let Ok(os) = LinuxOs::from_str(s) {
            Os::Linux(os)
        } else {
            match s {
                "windows" => Os::Windows,
                "macos" => Os::MacOs,
                "unix" => Os::Unix,
                _ => Os::Unknown,
            }
        }
    }
}

impl From<&String> for Os {
    fn from(s: &String) -> Self {
        Os::from(s.as_str())
    }
}

impl From<String> for Os {
    fn from(s: String) -> Self {
        Os::from(s.as_str())
    }
}
/// Detect the current operating system.
pub fn detect() -> Os {
    if cfg!(target_os = "linux") {
        Os::Linux(LinuxOs::detect())
    } else if cfg!(target_os = "windows") {
        Os::Windows
    } else if cfg!(target_os = "macos") {
        Os::MacOs
    } else {
        Os::Unknown
    }
}

#[test]
fn test_os_convert() {
    assert_eq!(Os::Unknown.as_ref(), "unknown");
    assert_eq!(Os::Linux(LinuxOs::Unknown).as_ref(), "linux");
    assert_eq!(Os::from("linux"), Os::Linux(LinuxOs::Unknown));
    assert_eq!(Os::from("manjaro"), Os::Linux(LinuxOs::Manjaro));
    assert_eq!(Os::from("unknown"), Os::Unknown);
}
