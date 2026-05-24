use std::{
  sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex, OnceLock},
  thread,
  time::Duration,
};

use tauri::{AppHandle, Emitter, Manager};
use windows::{
  core::w,
  Win32::{
    Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM},
    System::{DataExchange::AddClipboardFormatListener, LibraryLoader::GetModuleHandleW},
    UI::{
      Input::KeyboardAndMouse::{SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP, VIRTUAL_KEY, VK_CONTROL, VK_LWIN, VK_RWIN},
      WindowsAndMessaging::{CallNextHookEx, CreateWindowExW, DefWindowProcW, DispatchMessageW, GetForegroundWindow, GetMessageW, PostQuitMessage, RegisterClassW, SetForegroundWindow, SetWindowsHookExW, ShowWindow, TranslateMessage, CS_HREDRAW, CS_VREDRAW, HHOOK, HWND_MESSAGE, KBDLLHOOKSTRUCT, MSG, SW_HIDE, SW_SHOWNORMAL, WH_KEYBOARD_LL, WINDOW_EX_STYLE, WINDOW_STYLE, WM_CLIPBOARDUPDATE, WM_DESTROY, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP, WNDCLASSW},
    },
  },
};

use crate::{models::ClipboardItem, AppState};

static CLIPBOARD_RUNTIME: OnceLock<ClipboardRuntime> = OnceLock::new();
static HOTKEY_RUNTIME: OnceLock<HotkeyRuntime> = OnceLock::new();

#[derive(Clone)]
struct ClipboardRuntime {
  app: AppHandle,
  db: Arc<Mutex<crate::db::Database>>,
  ignore_next: Arc<AtomicBool>,
}

#[derive(Clone)]
struct HotkeyRuntime {
  app: AppHandle,
  win_down: Arc<AtomicBool>,
  last_foreground_window: Arc<Mutex<Option<isize>>>,
}

pub fn start_system_integrations(app: AppHandle, state: &AppState) {
  let clipboard_runtime = ClipboardRuntime { app: app.clone(), db: state.db.clone(), ignore_next: state.ignore_next_clipboard.clone() };
  let hotkey_runtime = HotkeyRuntime { app, win_down: Arc::new(AtomicBool::new(false)), last_foreground_window: state.last_foreground_window.clone() };

  let _ = CLIPBOARD_RUNTIME.set(clipboard_runtime);
  let _ = HOTKEY_RUNTIME.set(hotkey_runtime);

  thread::spawn(start_clipboard_message_window);
  thread::spawn(start_keyboard_hook);
}

pub fn hide_main_window(app: &AppHandle) -> Result<(), String> {
  if let Some(window) = app.get_webview_window("main") {
    window.hide().map_err(|error| error.to_string())?;
  }
  Ok(())
}

pub fn paste_text_and_hide(app: &AppHandle, text: &str, last_foreground_window: &Arc<Mutex<Option<isize>>>) -> Result<(), String> {
  let mut clipboard = arboard::Clipboard::new().map_err(|error| error.to_string())?;
  clipboard.set_text(text.to_string()).map_err(|error| error.to_string())?;
  hide_main_window(app)?;

  if let Some(hwnd) = *last_foreground_window.lock().map_err(|_| "foreground window lock poisoned".to_string())? {
    unsafe {
      let target = HWND(hwnd);
      ShowWindow(target, SW_SHOWNORMAL);
      let _ = SetForegroundWindow(target);
    }
  }

  thread::sleep(Duration::from_millis(70));
  send_ctrl_v();
  Ok(())
}

fn start_clipboard_message_window() {
  unsafe {
    let Ok(module) = GetModuleHandleW(None) else { return; };
    let class_name = w!("SmartClipboardMessageWindow");
    let window_class = WNDCLASSW {
      hInstance: HINSTANCE(module.0),
      lpszClassName: class_name,
      lpfnWndProc: Some(clipboard_wnd_proc),
      style: CS_HREDRAW | CS_VREDRAW,
      ..Default::default()
    };
    RegisterClassW(&window_class);
    let hwnd = CreateWindowExW(WINDOW_EX_STYLE(0), class_name, w!(""), WINDOW_STYLE(0), 0, 0, 0, 0, HWND_MESSAGE, None, HINSTANCE(module.0), None);
    if hwnd.0 == 0 {
      return;
    }
    let _ = AddClipboardFormatListener(hwnd);

    let mut message = MSG::default();
    while GetMessageW(&mut message, HWND(0), 0, 0).into() {
      TranslateMessage(&message);
      DispatchMessageW(&message);
    }
  }
}

unsafe extern "system" fn clipboard_wnd_proc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
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
  let Some(runtime) = CLIPBOARD_RUNTIME.get() else { return; };
  if runtime.ignore_next.swap(false, Ordering::SeqCst) {
    return;
  }

  let Ok(mut clipboard) = arboard::Clipboard::new() else { return; };

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
      if let Ok(item) = db.insert_image_item(image.width, image.height, image.bytes.as_ref()) {
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
    let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), HINSTANCE(0), 0);
    if hook.0 == 0 {
      return;
    }

    let mut message = MSG::default();
    while GetMessageW(&mut message, HWND(0), 0, 0).into() {
      TranslateMessage(&message);
      DispatchMessageW(&message);
    }
  }
}

unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
  if code < 0 {
    return CallNextHookEx(HHOOK(0), code, wparam, lparam);
  }

  let Some(runtime) = HOTKEY_RUNTIME.get() else {
    return CallNextHookEx(HHOOK(0), code, wparam, lparam);
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
    show_main_window(runtime);
    return LRESULT(1);
  }

  CallNextHookEx(HHOOK(0), code, wparam, lparam)
}

fn show_main_window(runtime: &HotkeyRuntime) {
  unsafe {
    let foreground = GetForegroundWindow();
    if foreground.0 != 0 {
      if let Ok(mut last) = runtime.last_foreground_window.lock() {
        *last = Some(foreground.0);
      }
    }
  }

  if let Some(window) = runtime.app.get_webview_window("main") {
    let _ = window.show();
    let _ = window.set_focus();
  }
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
