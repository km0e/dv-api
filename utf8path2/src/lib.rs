use std::{
    borrow::{Borrow, Cow},
    cmp,
    ops::Deref,
    path::{Components, Path},
};

pub struct U8Path(str);

impl<'a> From<&'a str> for &'a U8Path {
    fn from(path: &'a str) -> Self {
        U8Path::new(path)
    }
}

impl<'a> From<&'a String> for &'a U8Path {
    fn from(path: &'a String) -> Self {
        U8Path::new(path)
    }
}

impl std::fmt::Display for U8Path {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<Path> for &U8Path {
    fn as_ref(&self) -> &Path {
        self.0.as_ref()
    }
}

impl<'a> From<&'a U8Path> for Cow<'a, U8Path> {
    fn from(path: &'a U8Path) -> Self {
        Cow::Borrowed(path)
    }
}

impl U8Path {
    pub fn metadata(&self) -> std::io::Result<std::fs::Metadata> {
        Path::new(&self.0).metadata()
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn has_root(&self) -> bool {
        Path::new(&self.0).has_root()
    }
    pub fn starts_with(&self, prefix: &str) -> bool {
        self.0.starts_with(prefix)
    }
    pub fn new<S: AsRef<str> + ?Sized>(s: &S) -> &U8Path {
        unsafe { &*(s.as_ref() as *const str as *const U8Path) }
    }
    pub fn components(&self) -> Components {
        Path::new(&self.0).components()
    }
    pub fn join<P: AsRef<str>>(&self, path: P) -> U8PathBuf {
        let mut path_buf = U8PathBuf(self.0.to_string());
        path_buf.push(path);
        path_buf
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Default, Debug, Clone)]
pub struct U8PathBuf(String);

impl U8PathBuf {
    /// just push a path to the end of the string
    pub fn push<P: AsRef<str>>(&mut self, path: P) {
        let path = path.as_ref();
        if !self.0.is_empty() && !self.0.ends_with('/') {
            self.0.push('/');
        }
        self.0.push_str(path);
    }
}

impl AsRef<str> for U8PathBuf {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for U8PathBuf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl Deref for U8PathBuf {
    type Target = U8Path;

    fn deref(&self) -> &Self::Target {
        U8Path::new(&self.0)
    }
}

impl Borrow<U8Path> for U8PathBuf {
    fn borrow(&self) -> &U8Path {
        self.deref()
    }
}

impl ToOwned for U8Path {
    type Owned = U8PathBuf;

    fn to_owned(&self) -> Self::Owned {
        U8PathBuf(self.0.to_string())
    }
}

impl From<&str> for U8PathBuf {
    fn from(path: &str) -> Self {
        Self(path.to_string())
    }
}

impl From<&U8Path> for U8PathBuf {
    fn from(path: &U8Path) -> Self {
        Self::from(&path.0)
    }
}

impl From<String> for U8PathBuf {
    fn from(path: String) -> Self {
        Self(path)
    }
}

impl From<U8PathBuf> for Cow<'_, U8Path> {
    fn from(path: U8PathBuf) -> Self {
        Cow::Owned(path)
    }
}

impl PartialEq for U8Path {
    #[inline]
    fn eq(&self, other: &U8Path) -> bool {
        Path::new(&self.0) == Path::new(&other.0)
    }
}

impl PartialOrd for U8Path {
    #[inline]
    fn partial_cmp(&self, other: &U8Path) -> Option<cmp::Ordering> {
        Path::new(&self.0).partial_cmp(Path::new(&other.0))
    }
}
