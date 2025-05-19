pub mod prelude {
    pub use super::core::*;
    pub use super::instance::openpty_local;
}

mod core;
mod error;
mod instance;
pub use error::{Error, ErrorChain, Result};
