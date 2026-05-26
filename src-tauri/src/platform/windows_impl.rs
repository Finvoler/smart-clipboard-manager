use std::{
    borrow::Cow,
    fs, io,
    path::{Path, PathBuf},
    ptr::null_mut,
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        Arc, Mutex, OnceLock,
    },
    thread,
    time::Duration,
};

use tauri::{AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, Position, Size};
use windows::{
    core::{w, Interface, HSTRING},
    Win32::{
        Foundation::{BOOL, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM},
        Graphics::Gdi::{
            ClientToScreen, GetMonitorInfoW, MonitorFromRect, MONITORINFO, MONITOR_DEFAULTTONEAREST,
        },
        System::{
            Com::{
                CoCreateInstance, CoInitializeEx, CoUninitialize, IPersistFile,
                CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
            },
            Console::GetConsoleWindow,
            DataExchange::AddClipboardFormatListener,
            LibraryLoader::GetModuleHandleW,
            Threading::{AttachThreadInput, GetCurrentThreadId},
        },
        UI::Shell::{IShellLinkW, ShellLink},
        UI::{
            Input::KeyboardAndMouse::{
                SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS,
                KEYEVENTF_KEYUP, VIRTUAL_KEY, VK_CONTROL, VK_LWIN, VK_RWIN,
            },
            WindowsAndMessaging::{
                BringWindowToTop, CallNextHookEx, CreateWindowExW, DefWindowProcW,
                DispatchMessageW, GetCursorPos, GetForegroundWindow, GetGUIThreadInfo, GetMessageW,
                GetWindowRect, GetWindowThreadProcessId, IsIconic, PostQuitMessage, RegisterClassW,
                SetForegroundWindow, SetWindowsHookExW, ShowWindow, TranslateMessage, CS_HREDRAW,
                CS_VREDRAW, GUITHREADINFO, HHOOK, HWND_MESSAGE, KBDLLHOOKSTRUCT, LLKHF_INJECTED,
                MSG, SW_HIDE, SW_RESTORE, SW_SHOWNORMAL, WH_KEYBOARD_LL,
                WINDOW_EX_STYLE, WINDOW_STYLE, WM_CLIPBOARDUPDATE, WM_DESTROY, WM_KEYDOWN,
                WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP, WNDCLASSW,
            },
        },
    },
};
use winreg::{enums::HKEY_CURRENT_USER, RegKey};

use crate::{models::ClipboardItem, AppState};

const WINDOW_WIDTH: f64 = 960.0;
const WINDOW_HEIGHT: f64 = 640.0;
const WINDOW_MARGIN: i32 = 10;
const STARTUP_VALUE_NAME: &str = "SmartClipboardManager";
const STARTUP_SHORTCUT_NAME: &str = "Smart Clipboard Manager.lnk";
const STARTUP_ARG: &str = "--startup";

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
    win_forwarded: Arc<AtomicBool>,
    smart_win_v_active: Arc<AtomicBool>,
    held_win_vk: Arc<AtomicU32>,
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
        win_forwarded: Arc::new(AtomicBool::new(false)),
        smart_win_v_active: Arc::new(AtomicBool::new(false)),
        held_win_vk: Arc::new(AtomicU32::new(0)),
        settings: state.settings.clone(),
        last_foreground_window: state.last_foreground_window.clone(),
    };

    let _ = CLIPBOARD_RUNTIME.set(clipboard_runtime);
    let _ = HOTKEY_RUNTIME.set(hotkey_runtime);

    thread::spawn(start_clipboard_message_window);
    thread::spawn(start_keyboard_hook);
}

pub fn set_run_at_startup(_app: &AppHandle, enable: bool) -> Result<(), String> {
    // Always remove any legacy registry Run value left by older builds.
    let _ = remove_startup_registry_value();
    if enable {
        let exe = std::env::current_exe().map_err(|error| error.to_string())?;
        ensure_startup_shortcut(&exe)
    } else {
        remove_startup_shortcut()
    }
}

fn startup_shortcut_path() -> Result<PathBuf, String> {
    let appdata =
        std::env::var_os("APPDATA").ok_or_else(|| "APPDATA is not available".to_string())?;
    Ok(PathBuf::from(appdata)
        .join("Microsoft")
        .join("Windows")
        .join("Start Menu")
        .join("Programs")
        .join("Startup")
        .join(STARTUP_SHORTCUT_NAME))
}

fn ensure_startup_shortcut(exe: &Path) -> Result<(), String> {
    let shortcut_path = startup_shortcut_path()?;
    if let Some(parent) = shortcut_path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    if shortcut_path.exists() {
        let _ = fs::remove_file(&shortcut_path);
    }
    create_startup_shortcut(exe, &shortcut_path)
}

fn remove_startup_shortcut() -> Result<(), String> {
    match fs::remove_file(startup_shortcut_path()?) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

fn remove_startup_registry_value() -> Result<(), String> {
    let run_key = startup_registry_key()?;
    match run_key.delete_value(STARTUP_VALUE_NAME) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

fn startup_registry_key() -> Result<RegKey, String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    hkcu.create_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Run")
        .map(|(key, _)| key)
        .map_err(|error| error.to_string())
}

fn create_startup_shortcut(exe: &Path, shortcut_path: &Path) -> Result<(), String> {
    unsafe {
        let com_initialized = CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_ok();
        let result = (|| -> windows::core::Result<()> {
            let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
            link.SetPath(&HSTRING::from(exe.to_string_lossy().as_ref()))?;
            link.SetArguments(&HSTRING::from(STARTUP_ARG))?;
            if let Some(parent) = exe.parent() {
                link.SetWorkingDirectory(&HSTRING::from(parent.to_string_lossy().as_ref()))?;
            }
            link.SetDescription(&HSTRING::from("Smart Clipboard Manager"))?;
            link.SetIconLocation(&HSTRING::from(exe.to_string_lossy().as_ref()), 0)?;
            link.SetShowCmd(SW_HIDE)?;
            let persist_file: IPersistFile = link.cast()?;
            persist_file.Save(
                &HSTRING::from(shortcut_path.to_string_lossy().as_ref()),
                true,
            )
        })();
        if com_initialized {
            CoUninitialize();
        }
        result.map_err(|error| error.to_string())
    }
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
    clear_hotkey_state();
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
    clear_hotkey_state();
    hide_main_window(app)?;

    if let Some(hwnd) = *last_foreground_window
        .lock()
        .map_err(|_| "foreground window lock poisoned".to_string())?
    {
        unsafe {
            let target = HWND(hwnd as *mut _);
            restore_target_window(target);
        }
    }

    thread::sleep(Duration::from_millis(140));
    send_ctrl_v();
    Ok(())
}

pub fn paste_image_and_hide(
    app: &AppHandle,
    image_path: &str,
    last_foreground_window: &Arc<Mutex<Option<isize>>>,
) -> Result<(), String> {
    let image = image::open(Path::new(image_path))
        .map_err(|error| error.to_string())?
        .to_rgba8();
    let (width, height) = image.dimensions();
    let mut clipboard = arboard::Clipboard::new().map_err(|error| error.to_string())?;
    clipboard
        .set_image(arboard::ImageData {
            width: width as usize,
            height: height as usize,
            bytes: Cow::Owned(image.into_raw()),
        })
        .map_err(|error| error.to_string())?;

    clear_hotkey_state();
    hide_main_window(app)?;

    if let Some(hwnd) = *last_foreground_window
        .lock()
        .map_err(|_| "foreground window lock poisoned".to_string())?
    {
        unsafe {
            let target = HWND(hwnd as *mut _);
            restore_target_window(target);
        }
    }

    thread::sleep(Duration::from_millis(140));
    send_ctrl_v();
    Ok(())
}

unsafe fn restore_target_window(target: HWND) {
    if target.0.is_null() {
        return;
    }

    let current_thread = GetCurrentThreadId();
    let target_thread = GetWindowThreadProcessId(target, None);
    let attached = target_thread != 0
        && target_thread != current_thread
        && AttachThreadInput(current_thread, target_thread, BOOL(1)).as_bool();

    if IsIconic(target).as_bool() {
        let _ = ShowWindow(target, SW_RESTORE);
    }
    let _ = BringWindowToTop(target);
    let _ = SetForegroundWindow(target);

    if attached {
        let _ = AttachThreadInput(current_thread, target_thread, BOOL(0));
    }
}

pub fn show_main_window(
    app: &AppHandle,
    last_foreground_window: &Arc<Mutex<Option<isize>>>,
) -> Result<(), String> {
    let target_rect = capture_target_rect(last_foreground_window);
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_size(Size::Logical(LogicalSize::new(WINDOW_WIDTH, WINDOW_HEIGHT)));
        let scale_factor = window.scale_factor().unwrap_or(1.0);
        let physical_width = (WINDOW_WIDTH * scale_factor).round() as i32;
        let physical_height = (WINDOW_HEIGHT * scale_factor).round() as i32;
        if let Some((x, y)) =
            target_rect.and_then(|rect| place_near_rect(rect, physical_width, physical_height))
        {
            let _ = window.set_position(Position::Physical(PhysicalPosition::new(x, y)));
        }
        window.show().map_err(|error| error.to_string())?;
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
            if let Ok((item, quick_suggestions)) = db.insert_text_item(&text) {
                emit_new_item(&runtime.app, item);
                for quick_suggestion in quick_suggestions {
                    let _ = runtime
                        .app
                        .emit("on_quick_suggestion_detected", quick_suggestion);
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
    let is_win_key = vk == VK_LWIN.0 as u32 || vk == VK_RWIN.0 as u32;
    let is_v_key = vk == 0x56;

    if (keyboard.flags.0 & LLKHF_INJECTED.0) != 0 {
        return CallNextHookEx(HHOOK(null_mut()), code, wparam, lparam);
    }

    let should_intercept = runtime
        .settings
        .lock()
        .map(|settings| settings.app_enabled && settings.intercept_win_v)
        .unwrap_or(true);

    if !should_intercept {
        reset_win_hotkey_state(runtime);
        return CallNextHookEx(HHOOK(null_mut()), code, wparam, lparam);
    }

    if is_win_key {
        if is_down {
            runtime.win_down.store(true, Ordering::SeqCst);
            runtime.win_forwarded.store(false, Ordering::SeqCst);
            runtime.smart_win_v_active.store(false, Ordering::SeqCst);
            runtime.held_win_vk.store(vk, Ordering::SeqCst);
            return LRESULT(1);
        }

        if is_up {
            let was_down = runtime.win_down.swap(false, Ordering::SeqCst);
            let was_forwarded = runtime.win_forwarded.swap(false, Ordering::SeqCst);
            let smart_active = runtime.smart_win_v_active.swap(false, Ordering::SeqCst);
            let held_vk = runtime.held_win_vk.swap(0, Ordering::SeqCst);
            let win_key = VIRTUAL_KEY((if held_vk == 0 { vk } else { held_vk }) as u16);

            if was_forwarded {
                send_key_event(win_key, KEYEVENTF_KEYUP);
            } else if was_down && !smart_active {
                send_key_event(win_key, KEYBD_EVENT_FLAGS(0));
                send_key_event(win_key, KEYEVENTF_KEYUP);
            }
            return LRESULT(1);
        }
    }

    if runtime.smart_win_v_active.load(Ordering::SeqCst) {
        if is_v_key {
            if is_up {
                reset_win_hotkey_state(runtime);
            }
            return LRESULT(1);
        }
        reset_win_hotkey_state(runtime);
        return CallNextHookEx(HHOOK(null_mut()), code, wparam, lparam);
    }

    if runtime.win_down.load(Ordering::SeqCst) && is_down {
        if is_v_key && !runtime.win_forwarded.load(Ordering::SeqCst) {
            runtime.smart_win_v_active.store(true, Ordering::SeqCst);
            show_main_window_from_hotkey(runtime);
            return LRESULT(1);
        }

        if !runtime.win_forwarded.load(Ordering::SeqCst) {
            let held_vk = runtime.held_win_vk.load(Ordering::SeqCst);
            let win_key = VIRTUAL_KEY(
                (if held_vk == 0 {
                    VK_LWIN.0 as u32
                } else {
                    held_vk
                }) as u16,
            );
            send_key_event(win_key, KEYBD_EVENT_FLAGS(0));
            runtime.win_forwarded.store(true, Ordering::SeqCst);
        }
    }

    CallNextHookEx(HHOOK(null_mut()), code, wparam, lparam)
}

fn reset_win_hotkey_state(runtime: &HotkeyRuntime) {
    runtime.win_down.store(false, Ordering::SeqCst);
    runtime.win_forwarded.store(false, Ordering::SeqCst);
    runtime.smart_win_v_active.store(false, Ordering::SeqCst);
    runtime.held_win_vk.store(0, Ordering::SeqCst);
}

fn clear_hotkey_state() {
    if let Some(runtime) = HOTKEY_RUNTIME.get() {
        reset_win_hotkey_state(runtime);
    }
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
                let mut cursor = POINT::default();
                if GetCursorPos(&mut cursor).is_ok() && rect_contains_point(rect, cursor) {
                    return Some(rect_from_point(cursor));
                }
                return Some(rect);
            }
        }
    }
    None
}

fn place_near_rect(target: RECT, width: i32, height: i32) -> Option<(i32, i32)> {
    let work = monitor_work_area(target)?;
    let centered_x = target.left + ((target.right - target.left) / 2) - (width / 2);
    let above_y = target.top - height - WINDOW_MARGIN;
    let below_y = target.bottom + WINDOW_MARGIN;

    if above_y >= work.top + WINDOW_MARGIN {
        return Some((
            clamp(
                centered_x,
                work.left + WINDOW_MARGIN,
                work.right - width - WINDOW_MARGIN,
            ),
            above_y,
        ));
    }

    if below_y + height <= work.bottom - WINDOW_MARGIN {
        return Some((
            clamp(
                centered_x,
                work.left + WINDOW_MARGIN,
                work.right - width - WINDOW_MARGIN,
            ),
            below_y,
        ));
    }

    let centered_y = target.top + ((target.bottom - target.top) / 2) - (height / 2);
    let right_x = target.right + WINDOW_MARGIN;
    if right_x + width <= work.right - WINDOW_MARGIN {
        return Some((
            right_x,
            clamp(
                centered_y,
                work.top + WINDOW_MARGIN,
                work.bottom - height - WINDOW_MARGIN,
            ),
        ));
    }

    let left_x = target.left - width - WINDOW_MARGIN;
    if left_x >= work.left + WINDOW_MARGIN {
        return Some((
            left_x,
            clamp(
                centered_y,
                work.top + WINDOW_MARGIN,
                work.bottom - height - WINDOW_MARGIN,
            ),
        ));
    }

    Some((
        clamp(
            centered_x,
            work.left + WINDOW_MARGIN,
            work.right - width - WINDOW_MARGIN,
        ),
        clamp(
            target.top,
            work.top + WINDOW_MARGIN,
            work.bottom - height - WINDOW_MARGIN,
        ),
    ))
}

fn rect_contains_point(rect: RECT, point: POINT) -> bool {
    point.x >= rect.left && point.x <= rect.right && point.y >= rect.top && point.y <= rect.bottom
}

fn rect_from_point(point: POINT) -> RECT {
    RECT {
        left: point.x - 4,
        top: point.y - 4,
        right: point.x + 4,
        bottom: point.y + 18,
    }
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

fn send_key_event(key: VIRTUAL_KEY, flags: KEYBD_EVENT_FLAGS) {
    let inputs = [key_input(key, flags)];
    unsafe {
        let _ = SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
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
