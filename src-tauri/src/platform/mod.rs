#[cfg(target_os = "windows")]
mod windows_impl;

#[cfg(not(target_os = "windows"))]
mod fallback;

#[cfg(target_os = "windows")]
pub use windows_impl::*;

#[cfg(not(target_os = "windows"))]
pub use fallback::*;
