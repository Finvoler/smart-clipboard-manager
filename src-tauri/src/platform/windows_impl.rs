use std::{
    ptr::null_mut,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, OnceLock,
    },
    thread,
    time::Duration,
};

use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, Position, Size};
use windows::{
    core::w,
    Win32::{
        Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM},
        Graphics::Gdi::{
            ClientToScreen, GetMonitorInfoW, MonitorFromRect, MONITORINFO, MONITOR_DEFAULTTONEAREST,
        },
        System::{
            Console::GetConsoleWindow, DataExchange::AddClipboardFormatListener,
            LibraryLoader::GetModuleHandleW,
        },
        UI::{
            Input::KeyboardAndMouse::{
                SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS,
                KEYEVENTF_KEYUP, VIRTUAL_KEY, VK_CONTROL, VK_LWIN, VK_RWIN,
            },
            WindowsAndMessaging::{
                CallNextHookEx, CreateWindowExW, DefWindowProcW, DispatchMessageW,
                GetForegroundWindow, GetGUIThreadInfo, GetMessageW, GetWindowRect,
                GetWindowThreadProcessId, PostQuitMessage, RegisterClassW, SetForegroundWindow,
                SetWindowsHookExW, ShowWindow, TranslateMessage, CS_HREDRAW, CS_VREDRAW,
                GUITHREADINFO, HHOOK, HWND_MESSAGE, KBDLLHOOKSTRUCT, MSG, SW_HIDE, SW_SHOWNORMAL,
                WH_KEYBOARD_LL, WINDOW_EX_STYLE, WINDOW_STYLE, WM_CLIPBOARDUPDATE, WM_DESTROY,
                WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP, WNDCLASSW,
            },
        },
    },
};
use winreg::{enums::HKEY_CURRENT_USER, RegKey};

use crate::{models::ClipboardItem, AppState};

const WINDOW_WIDTH: u32 = 760;
const WINDOW_HEIGHT: u32 = 540;
const WINDOW_MARGIN: i32 = 10;
const STARTUP_VALUE_NAME: &str = "SmartClipboardManager";

static CLIPBOARD_RUNTIME: OnceLock<ClipboardRuntime> = OnceLock::new();
static HOTKEY_RUNTIME: OnceLock<HotkeyRuntime> = OnceLock::new();

#[derive(Clone)]
struct ClipboardRuntime {
    app: AppHandle,
    db: Arc<Mutex<crate::db::Database>>,
    settings: Arc<Mutex<crate::models::AppSettings>>,
    ignore_next: Arc<AtomicBool>,
}

#[derive(Clone)]
struct HotkeyRuntime {
    app: AppHandle,
    win_down: Arc<AtomicBool>,
    settings: Arc<Mutex<crate::models::AppSettings>>,
    last_foreground_window: Arc<Mutex<Option<isize>>>,
}

pub fn start_system_integrations(app: AppHandle, state: &AppState) {
    let clipboard_runtime = ClipboardRuntime {
        app: app.clone(),
        db: state.db.clone(),
        settings: state.settings.clone(),
        ignore_next: state.ignore_next_clipboard.clone(),
    };
    let hotkey_runtime = HotkeyRuntime {
        app,
        win_down: Arc::new(AtomicBool::new(false)),
        settings: state.settings.clone(),
        last_foreground_window: state.last_foreground_window.clone(),
    };

    let _ = CLIPBOARD_RUNTIME.set(clipboard_runtime);
    let _ = HOTKEY_RUNTIME.set(hotkey_runtime);

    thread::spawn(start_clipboard_message_window);
    thread::spawn(start_keyboard_hook);
}

pub fn set_run_at_startup(_app: &AppHandle, enable: bool) -> Result<(), String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (run_key, _) = hkcu
        .create_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Run")
        .map_err(|error| error.to_string())?;
    if enable {
        let exe = std::env::current_exe().map_err(|error| error.to_string())?;
        run_key
            .set_value(STARTUP_VALUE_NAME, &format!("\"{}\"", exe.display()))
            .map_err(|error| error.to_string())?;
    } else {
        let _ = run_key.delete_value(STARTUP_VALUE_NAME);
    }
    Ok(())
}

    pub fn set_hide_console_window(enable: bool) -> Result<(), String> {
        unsafe {
            let hwnd = GetConsoleWindow();
            if !hwnd.0.is_null() {
                let command = if enable { SW_HIDE } else { SW_SHOWNORMAL };
                let _ = ShowWindow(hwnd, command);
            }
        }
        Ok(())
    }

pub fn hide_main_window(app: &AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window.hide().map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub fn paste_text_and_hide(
    app: &AppHandle,
    text: &str,
    last_foreground_window: &Arc<Mutex<Option<isize>>>,
) -> Result<(), String> {
    let mut clipboard = arboard::Clipboard::new().map_err(|error| error.to_string())?;
    clipboard
        .set_text(text.to_string())
        .map_err(|error| error.to_string())?;
    hide_main_window(app)?;

    if let Some(hwnd) = *last_foreground_window
        .lock()
        .map_err(|_| "foreground window lock poisoned".to_string())?
    {
        unsafe {
            let target = HWND(hwnd as *mut _);
            let _ = ShowWindow(target, SW_SHOWNORMAL);
            let _ = SetForegroundWindow(target);
        }
    }

    thread::sleep(Duration::from_millis(70));
    send_ctrl_v();
    Ok(())
}

pub fn show_main_window(
    app: &AppHandle,
    last_foreground_window: &Arc<Mutex<Option<isize>>>,
) -> Result<(), String> {
    let target_rect = capture_target_rect(last_foreground_window);
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_size(Size::Physical(PhysicalSize::new(
            WINDOW_WIDTH,
            WINDOW_HEIGHT,
        )));
        if let Some((x, y)) = target_rect
            .and_then(|rect| place_near_rect(rect, WINDOW_WIDTH as i32, WINDOW_HEIGHT as i32))
        {
            let _ = window.set_position(Position::Physical(PhysicalPosition::new(x, y)));
        }
        window.show().map_err(|error| error.to_string())?;
        window.set_focus().map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn start_clipboard_message_window() {
    unsafe {
        let Ok(module) = GetModuleHandleW(None) else {
            return;
        };
        let class_name = w!("SmartClipboardMessageWindow");
        let window_class = WNDCLASSW {
            hInstance: HINSTANCE(module.0),
            lpszClassName: class_name,
            lpfnWndProc: Some(clipboard_wnd_proc),
            style: CS_HREDRAW | CS_VREDRAW,
            ..Default::default()
        };
        RegisterClassW(&window_class);
        let Ok(hwnd) = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class_name,
            w!(""),
            WINDOW_STYLE(0),
            0,
            0,
            0,
            0,
            HWND_MESSAGE,
            None,
            HINSTANCE(module.0),
            None,
        ) else {
            return;
        };
        if hwnd.0.is_null() {
            return;
        }
        let _ = AddClipboardFormatListener(hwnd);

        let mut message = MSG::default();
        while GetMessageW(&mut message, HWND(null_mut()), 0, 0).into() {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

unsafe extern "system" fn clipboard_wnd_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_CLIPBOARDUPDATE => {
            capture_clipboard();
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}

fn capture_clipboard() {
    let Some(runtime) = CLIPBOARD_RUNTIME.get() else {
        return;
    };
    if !runtime
        .settings
        .lock()
        .map(|settings| settings.app_enabled && settings.capture_enabled)
        .unwrap_or(true)
    {
        return;
    }
    if runtime.ignore_next.swap(false, Ordering::SeqCst) {
        return;
    }

    let Ok(mut clipboard) = arboard::Clipboard::new() else {
        return;
    };

    if let Ok(text) = clipboard.get_text() {
        if text.trim().is_empty() {
            return;
        }
        if let Ok(mut db) = runtime.db.lock() {
            if let Ok((item, quick_items)) = db.insert_text_item(&text) {
                emit_new_item(&runtime.app, item);
                for quick_item in quick_items {
                    let _ = runtime.app.emit("on_quick_pool_extracted", quick_item);
                }
            }
        }
        return;
    }

    if let Ok(image) = clipboard.get_image() {
        if let Ok(mut db) = runtime.db.lock() {
            if let Ok(item) = db.insert_image_item(image.width, image.height, image.bytes.as_ref())
            {
                emit_new_item(&runtime.app, item);
            }
        }
    }
}

fn emit_new_item(app: &AppHandle, item: ClipboardItem) {
    let _ = app.emit("on_new_item", item);
}

fn start_keyboard_hook() {
    unsafe {
        let Ok(hook) = SetWindowsHookExW(
            WH_KEYBOARD_LL,
            Some(keyboard_proc),
            HINSTANCE(null_mut()),
            0,
        ) else {
            return;
        };
        if hook.0.is_null() {
            return;
        }

        let mut message = MSG::default();
        while GetMessageW(&mut message, HWND(null_mut()), 0, 0).into() {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code < 0 {
        return CallNextHookEx(HHOOK(null_mut()), code, wparam, lparam);
    }

    let Some(runtime) = HOTKEY_RUNTIME.get() else {
        return CallNextHookEx(HHOOK(null_mut()), code, wparam, lparam);
    };

    let keyboard = *(lparam.0 as *const KBDLLHOOKSTRUCT);
    let message = wparam.0 as u32;
    let is_down = message == WM_KEYDOWN || message == WM_SYSKEYDOWN;
    let is_up = message == WM_KEYUP || message == WM_SYSKEYUP;
    let vk = keyboard.vkCode;

    if vk == VK_LWIN.0 as u32 || vk == VK_RWIN.0 as u32 {
        runtime.win_down.store(is_down && !is_up, Ordering::SeqCst);
    }

    if is_down && vk == 0x56 && runtime.win_down.load(Ordering::SeqCst) {
        let should_intercept = runtime
            .settings
            .lock()
            .map(|settings| settings.app_enabled && settings.intercept_win_v)
            .unwrap_or(true);
        if should_intercept {
            show_main_window_from_hotkey(runtime);
            return LRESULT(1);
        }
    }

    CallNextHookEx(HHOOK(null_mut()), code, wparam, lparam)
}

fn show_main_window_from_hotkey(runtime: &HotkeyRuntime) {
    unsafe {
        let foreground = GetForegroundWindow();
        if !foreground.0.is_null() {
            if let Ok(mut last) = runtime.last_foreground_window.lock() {
                *last = Some(foreground.0 as isize);
            }
        }
    }

    let _ = show_main_window(&runtime.app, &runtime.last_foreground_window);
}

fn capture_target_rect(last_foreground_window: &Arc<Mutex<Option<isize>>>) -> Option<RECT> {
    unsafe {
        let foreground = GetForegroundWindow();
        if !foreground.0.is_null() {
            if let Ok(mut last) = last_foreground_window.lock() {
                *last = Some(foreground.0 as isize);
            }

            let thread_id = GetWindowThreadProcessId(foreground, None);
            let mut gui = GUITHREADINFO {
                cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
                ..Default::default()
            };
            if thread_id != 0
                && GetGUIThreadInfo(thread_id, &mut gui).is_ok()
                && !gui.hwndCaret.0.is_null()
            {
                let mut top_left = POINT {
                    x: gui.rcCaret.left,
                    y: gui.rcCaret.top,
                };
                let mut bottom_right = POINT {
                    x: gui.rcCaret.right.max(gui.rcCaret.left + 1),
                    y: gui.rcCaret.bottom.max(gui.rcCaret.top + 18),
                };
                let top_ok = ClientToScreen(gui.hwndCaret, &mut top_left).as_bool();
                let bottom_ok = ClientToScreen(gui.hwndCaret, &mut bottom_right).as_bool();
                if top_ok && bottom_ok {
                    return Some(RECT {
                        left: top_left.x,
                        top: top_left.y,
                        right: bottom_right.x,
                        bottom: bottom_right.y,
                    });
                }
            }

            let mut rect = RECT::default();
            if GetWindowRect(foreground, &mut rect).is_ok() {
                return Some(rect);
            }
        }
    }
    None
}

fn place_near_rect(target: RECT, width: i32, height: i32) -> Option<(i32, i32)> {
    let work = monitor_work_area(target)?;
    let centered_x = target.left + ((target.right - target.left) / 2) - (width / 2);
    let x = clamp(
        centered_x,
        work.left + WINDOW_MARGIN,
        work.right - width - WINDOW_MARGIN,
    );
    let above_y = target.top - height - WINDOW_MARGIN;
    let below_y = target.bottom + WINDOW_MARGIN;
    let y = if above_y >= work.top + WINDOW_MARGIN {
        above_y
    } else if below_y + height <= work.bottom - WINDOW_MARGIN {
        below_y
    } else {
        clamp(
            target.top,
            work.top + WINDOW_MARGIN,
            work.bottom - height - WINDOW_MARGIN,
        )
    };
    Some((x, y))
}

fn monitor_work_area(target: RECT) -> Option<RECT> {
    unsafe {
        let monitor = MonitorFromRect(&target, MONITOR_DEFAULTTONEAREST);
        if monitor.0.is_null() {
            return None;
        }
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if GetMonitorInfoW(monitor, &mut info).as_bool() {
            Some(info.rcWork)
        } else {
            None
        }
    }
}

fn clamp(value: i32, min: i32, max: i32) -> i32 {
    value.max(min).min(max)
}

fn key_input(key: VIRTUAL_KEY, flags: KEYBD_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: key,
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn send_ctrl_v() {
    let inputs = [
        key_input(VK_CONTROL, KEYBD_EVENT_FLAGS(0)),
        key_input(VIRTUAL_KEY(0x56), KEYBD_EVENT_FLAGS(0)),
        key_input(VIRTUAL_KEY(0x56), KEYEVENTF_KEYUP),
        key_input(VK_CONTROL, KEYEVENTF_KEYUP),
    ];
    unsafe {
        let _ = SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
    }
}
