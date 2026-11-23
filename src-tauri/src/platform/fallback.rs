//! 非 Windows 平台的占位实现。
//!
//! 窗口和复制操作使用 Tauri/arboard，macOS 通过系统剪贴板变更计数记录历史。

use std::{
    borrow::Cow,
    path::Path,
    sync::{Arc, Mutex},
};

use tauri::{AppHandle, Emitter, Manager};

use crate::AppState;

#[cfg(target_os = "macos")]
use objc2::{rc::autoreleasepool, runtime::NSObjectProtocol, sel, MainThreadMarker};
#[cfg(target_os = "macos")]
use objc2_app_kit::{
    NSApplication, NSApplicationActivationOptions, NSPasteboard, NSPasteboardTypePNG,
    NSRunningApplication, NSWorkspace,
};
#[cfg(target_os = "macos")]
use tauri_plugin_global_shortcut::{
    Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState,
};

#[cfg(target_os = "macos")]
fn clipboard_shortcut() -> Shortcut {
    Shortcut::new(Some(Modifiers::CONTROL), Code::KeyV)
}

#[cfg(target_os = "macos")]
pub fn register_clipboard_shortcut(app: &AppHandle) {
    let result = app.global_shortcut().on_shortcut(clipboard_shortcut(), |app, _, event| {
        if event.state == ShortcutState::Pressed {
            if let Some(state) = app.try_state::<AppState>() {
                let _ = show_main_window(app, &state.last_foreground_window);
            }
        }
    });
    if let Err(error) = result {
        eprintln!("Could not register the Smart Clipboard shortcut: {error}");
    }
}

#[cfg(target_os = "macos")]
pub fn show_menu_title(app: &AppHandle) -> &'static str {
    if app.global_shortcut().is_registered(clipboard_shortcut()) {
        "Show Smart Clipboard (Ctrl+V)"
    } else {
        "Show Smart Clipboard"
    }
}

#[cfg(target_os = "macos")]
static LAST_CLIPBOARD_CHANGE: Mutex<isize> = Mutex::new(0);

#[cfg(target_os = "macos")]
pub fn start_system_integrations(app: AppHandle, state: &AppState) {
    *LAST_CLIPBOARD_CHANGE.lock().unwrap() =
        autoreleasepool(|_| NSPasteboard::generalPasteboard().changeCount());

    let state = state.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_millis(150));
        autoreleasepool(|_| {
            let mut last_change = LAST_CLIPBOARD_CHANGE.lock().unwrap();
            let pasteboard = NSPasteboard::generalPasteboard();
            let change = pasteboard.changeCount();
            if change == *last_change {
                return;
            }
            *last_change = change;

            let capture_enabled = state
                .settings
                .lock()
                .map(|settings| settings.app_enabled && settings.capture_enabled)
                .unwrap_or(false);
            if capture_enabled {
                super::capture::record_clipboard(&app, &state.db);
            }
        });
    });
}

#[cfg(not(target_os = "macos"))]
pub fn start_system_integrations(_app: AppHandle, _state: &AppState) {}

#[cfg(target_os = "macos")]
pub fn runs_at_startup() -> bool {
    use objc2_service_management::{SMAppService, SMAppServiceStatus};

    autoreleasepool(|_| unsafe {
        SMAppService::mainAppService().status() == SMAppServiceStatus::Enabled
    })
}

#[cfg(target_os = "macos")]
pub fn set_run_at_startup(_app: &AppHandle, enable: bool) -> Result<(), String> {
    use objc2_service_management::{SMAppService, SMAppServiceStatus};

    autoreleasepool(|_| unsafe {
        let service = SMAppService::mainAppService();
        let status = service.status();
        if enable && matches!(status, SMAppServiceStatus::NotRegistered | SMAppServiceStatus::NotFound) {
            service.registerAndReturnError().map_err(|error| error.localizedDescription().to_string())?;
        } else if !enable
            && matches!(status, SMAppServiceStatus::Enabled | SMAppServiceStatus::RequiresApproval)
        {
            service.unregisterAndReturnError().map_err(|error| error.localizedDescription().to_string())?;
        }
        if enable && service.status() != SMAppServiceStatus::Enabled {
            return Err("请在系统设置 → 通用 → 登录项中允许 Smart Clipboard。".to_string());
        }
        Ok(())
    })
}

#[cfg(not(target_os = "macos"))]
pub fn set_run_at_startup(_app: &AppHandle, _enable: bool) -> Result<(), String> {
    Ok(())
}

pub fn set_hide_console_window(_enable: bool) -> Result<(), String> {
    Ok(())
}

pub fn hide_main_window(app: &AppHandle) -> Result<(), String> {
    finish_panel(app, None)
}

fn finish_panel(app: &AppHandle, _pasteboard_change: Option<isize>) -> Result<(), String> {
    let Some(window) = app.get_webview_window("main") else {
        return Ok(());
    };
    #[cfg(target_os = "macos")]
    if let Some(state) = app.try_state::<AppState>() {
        let target_pid = state
            .last_foreground_window
            .lock()
            .map_err(|error| error.to_string())?
            .take();
        if let Some(target_pid) = target_pid {
            let panel = window.clone();
            return window
                .run_on_main_thread(move || {
                    if let Some(main_thread) = MainThreadMarker::new() {
                        restore_foreground_application(main_thread, target_pid as i32);
                    }
                    if panel.hide().is_ok() {
                        if let Some(change) = _pasteboard_change {
                            super::macos_paste::paste_when_focused(target_pid as i32, change);
                        }
                    }
                })
                .map_err(|error| error.to_string());
        }
    }
    window.hide().map_err(|error| error.to_string())
}

#[cfg(target_os = "macos")]
fn restore_foreground_application(main_thread: MainThreadMarker, pid: i32) {
    let current = NSRunningApplication::currentApplication();
    if !current.isActive() {
        return;
    }
    let Some(target) = NSRunningApplication::runningApplicationWithProcessIdentifier(pid) else {
        return;
    };
    let app = NSApplication::sharedApplication(main_thread);
    if app.respondsToSelector(sel!(yieldActivationToApplication:)) {
        app.yieldActivationToApplication(&target);
        target.activateFromApplication_options(&current, NSApplicationActivationOptions::empty());
    } else {
        #[allow(deprecated)]
        target.activateWithOptions(NSApplicationActivationOptions::ActivateIgnoringOtherApps);
    }
}

pub fn show_main_window(
    app: &AppHandle,
    _last_foreground_window: &Arc<Mutex<Option<isize>>>,
) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        #[cfg(target_os = "macos")]
        if let Some(frontmost) = NSWorkspace::sharedWorkspace().frontmostApplication() {
            let frontmost_pid = frontmost.processIdentifier();
            if frontmost_pid > 0 && frontmost_pid as u32 != std::process::id() {
                *_last_foreground_window
                    .lock()
                    .map_err(|error| error.to_string())? = Some(frontmost_pid as isize);
            }
        }
        let was_visible = window.is_visible().map_err(|error| error.to_string())?;
        #[cfg(target_os = "macos")]
        window.unminimize().map_err(|error| error.to_string())?;
        window.show().map_err(|error| error.to_string())?;
        window.set_focus().map_err(|error| error.to_string())?;
        if !was_visible {
            let _ = app.emit("panel-shown", ());
        }
    }
    Ok(())
}

pub fn paste_text_and_hide(
    app: &AppHandle,
    text: &str,
    _last_foreground_window: &Arc<Mutex<Option<isize>>>,
) -> Result<(), String> {
    let change = write_clipboard(|clipboard| clipboard.set_text(text))?;
    finish_panel(app, change)
}

pub fn paste_image_and_hide(
    app: &AppHandle,
    image_path: &str,
    _last_foreground_window: &Arc<Mutex<Option<isize>>>,
) -> Result<(), String> {
    let image = image::open(Path::new(image_path))
        .map_err(|error| error.to_string())?
        .to_rgba8();
    let (width, height) = image.dimensions();
    let change = write_clipboard(|clipboard| {
        clipboard.set_image(arboard::ImageData {
            width: width as usize,
            height: height as usize,
            bytes: Cow::Owned(image.into_raw()),
        })
    })?;
    finish_panel(app, change)
}

fn write_clipboard(
    write: impl FnOnce(&mut arboard::Clipboard) -> Result<(), arboard::Error>,
) -> Result<Option<isize>, String> {
    #[cfg(target_os = "macos")]
    let mut last_change = LAST_CLIPBOARD_CHANGE.lock().map_err(|error| error.to_string())?;

    let mut clipboard = arboard::Clipboard::new().map_err(|error| error.to_string())?;
    write(&mut clipboard).map_err(|error| error.to_string())?;

    #[cfg(target_os = "macos")]
    {
        let change = autoreleasepool(|_| NSPasteboard::generalPasteboard().changeCount());
        *last_change = change;
        Ok(Some(change))
    }
    #[cfg(not(target_os = "macos"))]
    Ok(None)
}

// arboard reads TIFF on macOS. Screenshots and other apps can offer PNG only.
#[cfg(target_os = "macos")]
pub(super) fn read_png() -> Option<arboard::ImageData<'static>> {
    let pasteboard = NSPasteboard::generalPasteboard();
    let data = pasteboard.dataForType(unsafe { NSPasteboardTypePNG })?;
    let pixels = image::load_from_memory_with_format(&data.to_vec(), image::ImageFormat::Png)
        .ok()?
        .into_rgba8();
    let (width, height) = pixels.dimensions();
    Some(arboard::ImageData {
        width: width as usize,
        height: height as usize,
        bytes: Cow::Owned(pixels.into_raw()),
    })
}
