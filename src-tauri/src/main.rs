#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

//! 原生可执行入口。
//!
//! 这里保持极薄，只负责把 Windows 构建切到 GUI 子系统，然后交给 lib.rs 里的 run。

fn main() {
    smart_clipboard_lib::run();
}
