mod dev {
    pub use super::super::Pm;
    pub use crate::User;
    pub use crate::error::Result;
}

pub mod alpine;
pub mod debian;
pub mod manjaro;
pub mod ubuntu;
pub mod windows;
