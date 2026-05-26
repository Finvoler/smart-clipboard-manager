//! 平台抽象层入口。
//!
//! Windows 真正实现放在 windows_impl.rs；其它平台用 fallback.rs 保持可编译。

#[cfg(target_os = "windows")]
mod windows_impl;

#[cfg(not(target_os = "windows"))]
mod fallback;

#[cfg(target_os = "windows")]
pub use windows_impl::*;

#[cfg(not(target_os = "windows"))]
pub use fallback::*;
