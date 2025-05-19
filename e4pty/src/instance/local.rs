#[cfg(not(windows))]
mod linux;

#[cfg(not(windows))]
pub use linux::openpty;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::openpty;
