//! Tauri 后端总入口。
//!
//! 这个文件负责把各个后端模块拼起来：初始化数据库、加载设置、注册 IPC 命令、
//! 启动系统集成（剪贴板监听、Win+V 热键、开机启动、托盘）并控制首屏显示逻辑。

mod ai;
mod commands;
mod db;
mod models;
mod platform;
mod quick_pool;

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

use db::Database;
use models::AppSettings;
use serde::{Deserialize, Serialize};
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager,
};

const STARTUP_ARG: &str = "--startup";

static APP_EXITING: AtomicBool = AtomicBool::new(false);

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Mutex<Database>>,
    pub settings: Arc<Mutex<AppSettings>>,
    pub current_data_dir: Arc<Mutex<PathBuf>>,
    pub ignore_next_clipboard: Arc<AtomicBool>,
    pub last_foreground_window: Arc<Mutex<Option<isize>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct BootstrapConfig {
    // 数据库打开前就要知道数据目录，所以这个小配置必须放在默认 AppData 中。
    custom_data_dir: Option<String>,
    // 目录切换需要重启后在数据库初始化前完成，pending 状态让迁移过程可恢复。
    pending_migration: Option<PendingMigration>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PendingMigration {
    from: String,
    to: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataDirectoryChangeResult {
    pub settings: AppSettings,
    pub message: String,
    pub restart_required: bool,
}

pub fn run() {
    install_auto_restart_hook();

    let mut builder = tauri::Builder::default();

    // 单实例插件必须先注册，后续 setup/窗口逻辑才不会让第二个进程继续初始化托盘和系统钩子。
    builder = builder.plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
        if args.iter().any(|arg| arg == STARTUP_ARG) {
            return;
        }

        if let Some(state) = app.try_state::<AppState>() {
            let _ = platform::show_main_window(app, &state.last_foreground_window);
            return;
        }

        if let Some(window) = app.get_webview_window("main") {
            let _ = window.show();
            let _ = window.unminimize();
            let _ = window.set_focus();
        }
    }));

    builder
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            commands::hide_window,
            commands::execute_paste,
            commands::get_history,
            commands::get_history_light,
            commands::get_item,
            commands::get_items_by_ids,
            commands::update_item_text,
            commands::delete_item,
            commands::get_image_data_url,
            commands::toggle_star,
            commands::get_folders,
            commands::create_folder,
            commands::delete_folder,
            commands::move_to_folder,
            commands::get_quick_pool,
            commands::get_quick_suggestions,
            commands::update_quick_item,
            commands::accept_quick_suggestion,
            commands::dismiss_quick_suggestion,
            commands::delete_quick_item,
            commands::star_quick_item,
            commands::get_app_settings,
            commands::save_app_settings,
            commands::change_data_directory,
            commands::restart_application,
            commands::test_ai_connection,
            commands::list_ai_models,
            commands::search_local,
            commands::search_local_light,
            commands::search_ai_semantic,
            commands::trigger_ai_categorize,
            commands::trigger_ocr
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" && !APP_EXITING.load(Ordering::SeqCst) {
                    // 用户点 X 时隐藏到托盘；真正退出/重启/系统关机时放行，避免拖住 Windows 关机流程。
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .setup(|app| {
            // setup 是后端真正的装配点：数据库、设置缓存、系统集成都在这里建立。
            let app_handle = app.handle().clone();
            let config_path = bootstrap_config_path(&app_handle)?;
            let mut bootstrap = load_bootstrap_config(&config_path)?;
            run_pending_migration(&mut bootstrap)?;
            save_bootstrap_config(&config_path, &bootstrap)?;

            let data_dir = resolve_effective_data_dir(&app_handle, &bootstrap)?;
            std::fs::create_dir_all(&data_dir)?;
            let db_path = data_dir.join("smart_clipboard.sqlite");
            let mut database = Database::open(db_path, data_dir.join("images"))?;
            database.cleanup_retention()?;
            let mut settings = database.get_app_settings()?;
            settings.data_directory = bootstrap.custom_data_dir.clone().unwrap_or_default();
            settings.resolved_data_directory = data_dir.to_string_lossy().to_string();
            if !settings.app_enabled {
                settings.app_enabled = true;
                settings = database.save_app_settings(settings)?;
                settings.resolved_data_directory = data_dir.to_string_lossy().to_string();
            }
            let run_at_startup = settings.run_at_startup;
            let hide_console_window = settings.hide_console_window;

            let state = AppState {
                db: Arc::new(Mutex::new(database)),
                settings: Arc::new(Mutex::new(settings)),
                current_data_dir: Arc::new(Mutex::new(data_dir)),
                ignore_next_clipboard: Arc::new(AtomicBool::new(false)),
                last_foreground_window: Arc::new(Mutex::new(None)),
            };

            let integrations_state = state.clone();
            app.manage(state);
            platform::start_system_integrations(app.handle().clone(), &integrations_state);
            platform::set_run_at_startup(app.handle(), run_at_startup)?;
            platform::set_hide_console_window(hide_console_window)?;
            setup_tray(app.handle())?;
            sync_initial_window_visibility(app.handle());
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("failed to build Smart Clipboard")
        .run(|_app, event| match event {
            tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit => {
                APP_EXITING.store(true, Ordering::SeqCst);
            }
            _ => {}
        });
}

fn bootstrap_config_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let config_dir = app
        .path()
        .app_config_dir()
        .map_err(|error| error.to_string())?;
    fs::create_dir_all(&config_dir).map_err(|error| error.to_string())?;
    Ok(config_dir.join("storage-bootstrap.json"))
}

fn load_bootstrap_config(path: &Path) -> Result<BootstrapConfig, String> {
    if !path.exists() {
        return Ok(BootstrapConfig::default());
    }
    let content = fs::read_to_string(path).map_err(|error| error.to_string())?;
    // PowerShell 5.1 的 UTF8 写入可能带 BOM；配置损坏时回退默认目录，避免启动崩溃。
    let content = content.trim_start_matches('\u{feff}');
    match serde_json::from_str(content) {
        Ok(config) => Ok(config),
        Err(_) => Ok(BootstrapConfig::default()),
    }
}

fn save_bootstrap_config(path: &Path, config: &BootstrapConfig) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let content = serde_json::to_string_pretty(config).map_err(|error| error.to_string())?;
    fs::write(path, content).map_err(|error| error.to_string())
}

fn resolve_effective_data_dir(
    app: &tauri::AppHandle,
    bootstrap: &BootstrapConfig,
) -> Result<PathBuf, String> {
    if let Some(custom) = bootstrap.custom_data_dir.as_deref() {
        let trimmed = custom.trim();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed));
        }
    }
    app.path().app_data_dir().map_err(|error| error.to_string())
}

fn run_pending_migration(bootstrap: &mut BootstrapConfig) -> Result<(), String> {
    let Some(pending) = bootstrap.pending_migration.clone() else {
        return Ok(());
    };

    let from = PathBuf::from(&pending.from);
    let to = PathBuf::from(&pending.to);
    if from == to {
        bootstrap.pending_migration = None;
        return Ok(());
    }

    if !from.exists() {
        fs::create_dir_all(&to).map_err(|error| error.to_string())?;
        bootstrap.pending_migration = None;
        return Ok(());
    }

    if to.exists() && target_dir_has_conflicting_app_data(&to)? {
        // 如果上次重启已经把数据库复制过去，但还没来得及清 pending，这里直接收敛。
        if to.join("smart_clipboard.sqlite").exists() {
            bootstrap.pending_migration = None;
            return Ok(());
        }
        return Err(target_dir_conflict_message(&to));
    }

    copy_app_data_files(&from, &to)?;
    rewrite_migrated_image_paths(&to, &from, &to)?;
    remove_app_data_files(&from);
    remove_dir_if_empty(&from);
    bootstrap.pending_migration = None;
    Ok(())
}

fn copy_app_data_files(from: &Path, to: &Path) -> Result<(), String> {
    fs::create_dir_all(to).map_err(|error| error.to_string())?;
    // 只迁移应用数据文件，允许 exe 与数据库位于同一目录而不误动程序本体。
    copy_if_exists(
        &from.join("smart_clipboard.sqlite"),
        &to.join("smart_clipboard.sqlite"),
    )?;
    copy_if_exists(
        &from.join("smart_clipboard.sqlite-wal"),
        &to.join("smart_clipboard.sqlite-wal"),
    )?;
    copy_if_exists(
        &from.join("smart_clipboard.sqlite-shm"),
        &to.join("smart_clipboard.sqlite-shm"),
    )?;

    let images_dir = from.join("images");
    if images_dir.is_dir() {
        copy_dir_recursive(&images_dir, &to.join("images"))?;
    }
    Ok(())
}

fn copy_if_exists(from: &Path, to: &Path) -> Result<(), String> {
    if !from.exists() {
        return Ok(());
    }
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::copy(from, to)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn copy_dir_recursive(from: &Path, to: &Path) -> Result<(), String> {
    fs::create_dir_all(to).map_err(|error| error.to_string())?;
    for entry in fs::read_dir(from).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let source = entry.path();
        let target = to.join(entry.file_name());
        let file_type = entry.file_type().map_err(|error| error.to_string())?;
        if file_type.is_dir() {
            copy_dir_recursive(&source, &target)?;
        } else {
            fs::copy(&source, &target).map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

fn remove_app_data_files(from: &Path) {
    // 清理时同样只删应用数据，保留目标目录里可能共存的 SmartClipboard.exe 或其他用户文件。
    let _ = fs::remove_file(from.join("smart_clipboard.sqlite"));
    let _ = fs::remove_file(from.join("smart_clipboard.sqlite-wal"));
    let _ = fs::remove_file(from.join("smart_clipboard.sqlite-shm"));
    let _ = fs::remove_dir_all(from.join("images"));
}

fn remove_dir_if_empty(path: &Path) {
    let Ok(mut entries) = fs::read_dir(path) else {
        return;
    };
    if entries.next().is_none() {
        let _ = fs::remove_dir(path);
    }
}

fn rewrite_migrated_image_paths(
    target_dir: &Path,
    old_root: &Path,
    new_root: &Path,
) -> Result<(), String> {
    let db_path = target_dir.join("smart_clipboard.sqlite");
    if !db_path.exists() {
        return Ok(());
    }
    let old_prefix = old_root.join("images").to_string_lossy().to_string();
    let new_prefix = new_root.join("images").to_string_lossy().to_string();
    let connection = rusqlite::Connection::open(&db_path).map_err(|error| error.to_string())?;
    connection
        .execute(
            "UPDATE items SET image_path = REPLACE(image_path, ?1, ?2) WHERE image_path LIKE ?3",
            rusqlite::params![
                old_prefix,
                new_prefix,
                format!("{}%", old_root.to_string_lossy())
            ],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn sync_initial_window_visibility(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    // 登录自启时只保留托盘常驻，不抢焦点；手动启动时再显示主窗口。
    if is_startup_launch() {
        let _ = window.hide();
    } else {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn is_startup_launch() -> bool {
    std::env::args_os().any(|arg| arg == std::ffi::OsStr::new(STARTUP_ARG))
}

fn setup_tray(app: &tauri::AppHandle) -> tauri::Result<()> {
    let menu = build_tray_menu(app, current_intercept_win_v(app))?;
    let mut tray = TrayIconBuilder::with_id("main")
        .tooltip("Smart Clipboard")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| {
            let id = event.id().as_ref();
            if id == "show" {
                if let Some(state) = app.try_state::<AppState>() {
                    let _ = platform::show_main_window(app, &state.last_foreground_window);
                }
            } else if id == "toggle_win_v" {
                if let Some(state) = app.try_state::<AppState>() {
                    if let Ok(saved) = toggle_runtime_setting(app, &state, |settings| {
                        settings.intercept_win_v = !settings.intercept_win_v;
                    }) {
                        let _ = sync_tray_menu(app, saved.intercept_win_v);
                    }
                }
            } else if id == "restart" {
                let _ = restart_app(app, "manual");
            } else if id == "quit" {
                APP_EXITING.store(true, Ordering::SeqCst);
                app.exit(0);
            }
        });

    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

fn build_tray_menu(
    app: &tauri::AppHandle,
    intercept_win_v: bool,
) -> tauri::Result<Menu<tauri::Wry>> {
    let show = MenuItem::with_id(app, "show", "Show Smart Clipboard", true, None::<&str>)?;
    let native = MenuItem::with_id(
        app,
        "toggle_win_v",
        win_v_tray_label(intercept_win_v),
        true,
        None::<&str>,
    )?;
    let restart = MenuItem::with_id(
        app,
        "restart",
        "Restart Smart Clipboard",
        true,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    Menu::with_items(app, &[&show, &native, &restart, &quit])
}

fn current_intercept_win_v(app: &tauri::AppHandle) -> bool {
    app.try_state::<AppState>()
        .and_then(|state| {
            state
                .settings
                .lock()
                .ok()
                .map(|settings| settings.intercept_win_v)
        })
        .unwrap_or(true)
}

pub(crate) fn sync_tray_menu(app: &tauri::AppHandle, intercept_win_v: bool) -> Result<(), String> {
    if let Some(tray) = app.tray_by_id("main") {
        let menu = build_tray_menu(app, intercept_win_v).map_err(|error| error.to_string())?;
        tray.set_menu(Some(menu))
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn win_v_tray_label(intercept_win_v: bool) -> &'static str {
    if intercept_win_v {
        "Use Native Win+V"
    } else {
        "Use Smart Clipboard Win+V"
    }
}

fn restart_app(app: &tauri::AppHandle, reason: &str) -> Result<(), String> {
    spawn_replacement_process(reason)?;
    APP_EXITING.store(true, Ordering::SeqCst);
    app.exit(0);
    Ok(())
}

pub(crate) fn schedule_data_dir_change(
    app: &tauri::AppHandle,
    next_data_dir: Option<&str>,
) -> Result<(), String> {
    let config_path = bootstrap_config_path(app)?;
    let mut bootstrap = load_bootstrap_config(&config_path)?;
    let current_dir = resolve_effective_data_dir(app, &bootstrap)?;
    let target_dir = if let Some(path) = next_data_dir {
        let trimmed = path.trim();
        if trimmed.is_empty() {
            app.path()
                .app_data_dir()
                .map_err(|error| error.to_string())?
        } else {
            PathBuf::from(trimmed)
        }
    } else {
        app.path()
            .app_data_dir()
            .map_err(|error| error.to_string())?
    };

    if current_dir == target_dir {
        bootstrap.custom_data_dir = next_data_dir
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned);
        bootstrap.pending_migration = None;
        return save_bootstrap_config(&config_path, &bootstrap);
    }

    bootstrap.custom_data_dir = next_data_dir
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    bootstrap.pending_migration = Some(PendingMigration {
        from: current_dir.to_string_lossy().to_string(),
        to: target_dir.to_string_lossy().to_string(),
    });
    save_bootstrap_config(&config_path, &bootstrap)
}

pub(crate) fn validate_data_dir_change(
    app: &tauri::AppHandle,
    next_data_dir: Option<&str>,
) -> Result<(), String> {
    let config_path = bootstrap_config_path(app)?;
    let bootstrap = load_bootstrap_config(&config_path)?;
    let current_dir = resolve_effective_data_dir(app, &bootstrap)?;
    let target_dir = if let Some(path) = next_data_dir {
        let trimmed = path.trim();
        if trimmed.is_empty() {
            app.path()
                .app_data_dir()
                .map_err(|error| error.to_string())?
        } else {
            PathBuf::from(trimmed)
        }
    } else {
        app.path()
            .app_data_dir()
            .map_err(|error| error.to_string())?
    };

    if current_dir == target_dir {
        return Ok(());
    }
    if target_dir.exists() && target_dir_has_conflicting_app_data(&target_dir)? {
        return Err(target_dir_conflict_message(&target_dir));
    }
    Ok(())
}

fn target_dir_has_conflicting_app_data(target_dir: &Path) -> Result<bool, String> {
    let db_path = target_dir.join("smart_clipboard.sqlite");
    if db_path.exists() {
        return Ok(true);
    }

    // 目录里存在 exe、README 或其他无关文件都允许；真正冲突的是旧数据库或非空 images 数据目录。
    let images_dir = target_dir.join("images");
    if !images_dir.exists() {
        return Ok(false);
    }

    if !images_dir.is_dir() {
        return Ok(true);
    }

    let mut entries = fs::read_dir(&images_dir).map_err(|error| error.to_string())?;
    Ok(entries.next().is_some())
}

fn target_dir_conflict_message(target_dir: &Path) -> String {
    format!(
        "Target data directory already contains Smart Clipboard data: {}",
        target_dir.to_string_lossy()
    )
}

pub(crate) fn current_data_dir_string(state: &AppState) -> Result<String, String> {
    state
        .current_data_dir
        .lock()
        .map_err(|_| "data directory lock poisoned".to_string())
        .map(|path| path.to_string_lossy().to_string())
}

#[cfg(target_os = "windows")]
fn spawn_replacement_process(reason: &str) -> Result<(), String> {
    use std::os::windows::process::CommandExt;

    let exe_path = std::env::current_exe().map_err(|error| error.to_string())?;
    let exe = windows_restart_path(exe_path.clone());
    let working_dir = exe_path
        .parent()
        .map(|path| windows_restart_path(path.to_path_buf()))
        .unwrap_or_else(|| ".".to_string());
    let script = format!(
        "Start-Sleep -Milliseconds 1500; Start-Process -FilePath '{}' -WorkingDirectory '{}'",
        windows_powershell_literal(&exe),
        windows_powershell_literal(&working_dir)
    );
    // 单实例保护开启后，新进程必须等旧进程退出后再启动，否则会被当成重复实例立即关闭。
    // 这里故意不用 cmd /c start，避免 Windows 对引号和 \\?\ 前缀的解析把重启路径打坏。
    Command::new("powershell")
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-WindowStyle",
            "Hidden",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &script,
        ])
        .env("SMART_CLIPBOARD_RESTART_REASON", reason)
        .env("SMART_CLIPBOARD_RESTARTED", "1")
        .creation_flags(0x08000000 /* CREATE_NO_WINDOW */)
        .spawn()
        .map(|_| ())
        .map_err(|error| error.to_string())
}

#[cfg(target_os = "windows")]
fn windows_restart_path(path: PathBuf) -> String {
    let raw = path.to_string_lossy();
    if let Some(stripped) = raw.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{}", stripped);
    }
    if let Some(stripped) = raw.strip_prefix(r"\\?\") {
        return stripped.to_string();
    }
    raw.into_owned()
}

#[cfg(target_os = "windows")]
fn windows_powershell_literal(value: &str) -> String {
    value.replace('\'', "''")
}

#[cfg(not(target_os = "windows"))]
fn spawn_replacement_process(reason: &str) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|error| error.to_string())?;
    let mut cmd = Command::new(exe);
    cmd.env("SMART_CLIPBOARD_RESTART_REASON", reason)
        .env("SMART_CLIPBOARD_RESTARTED", "1");
    cmd.spawn().map(|_| ()).map_err(|error| error.to_string())
}

fn install_auto_restart_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        previous(panic_info);
        if std::env::var_os("SMART_CLIPBOARD_RESTARTED").is_none()
            && !APP_EXITING.load(Ordering::SeqCst)
        {
            let _ = spawn_replacement_process("panic");
        }
    }));
}

fn toggle_runtime_setting<F>(
    app: &tauri::AppHandle,
    state: &AppState,
    update: F,
) -> Result<AppSettings, String>
where
    F: FnOnce(&mut AppSettings),
{
    let saved = {
        let mut settings = state
            .settings
            .lock()
            .map_err(|_| "settings lock poisoned".to_string())?
            .clone();
        update(&mut settings);
        let db = state
            .db
            .lock()
            .map_err(|_| "database lock poisoned".to_string())?;
        db.save_app_settings(settings).map_err(String::from)?
    };
    platform::set_run_at_startup(app, saved.run_at_startup)?;
    platform::set_hide_console_window(saved.hide_console_window)?;
    if let Ok(mut cached) = state.settings.lock() {
        *cached = saved.clone();
    }
    Ok(saved)
}

#[cfg(test)]
mod tests {
    use super::{
        copy_app_data_files, remove_app_data_files, remove_dir_if_empty,
        target_dir_has_conflicting_app_data,
    };
    use std::{env, fs, path::PathBuf};
    use uuid::Uuid;

    #[cfg(target_os = "windows")]
    use super::windows_restart_path;

    struct TempTestDir {
        path: PathBuf,
    }

    impl TempTestDir {
        fn new() -> Self {
            let path = env::temp_dir().join(format!("smart-clipboard-test-{}", Uuid::new_v4()));
            fs::create_dir_all(&path).expect("failed to create temp test dir");
            Self { path }
        }
    }

    impl Drop for TempTestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn allows_target_dir_with_unrelated_files() {
        let temp = TempTestDir::new();
        fs::write(temp.path.join("SmartClipboard.exe"), b"exe").expect("failed to write test file");

        assert!(!target_dir_has_conflicting_app_data(&temp.path)
            .expect("unexpected validation failure"));
    }

    #[test]
    fn rejects_target_dir_with_existing_database() {
        let temp = TempTestDir::new();
        fs::write(temp.path.join("smart_clipboard.sqlite"), b"db")
            .expect("failed to write db file");

        assert!(
            target_dir_has_conflicting_app_data(&temp.path).expect("unexpected validation failure")
        );
    }

    #[test]
    fn migrates_only_app_data_files() {
        let source = TempTestDir::new();
        let target = TempTestDir::new();
        fs::write(source.path.join("SmartClipboard.exe"), b"exe").expect("failed to write exe");
        fs::write(source.path.join("smart_clipboard.sqlite"), b"db").expect("failed to write db");
        fs::create_dir_all(source.path.join("images")).expect("failed to create images dir");
        fs::write(source.path.join("images").join("item.png"), b"png")
            .expect("failed to write image");

        copy_app_data_files(&source.path, &target.path).expect("failed to copy app data");
        remove_app_data_files(&source.path);
        remove_dir_if_empty(&source.path);

        assert!(source.path.join("SmartClipboard.exe").exists());
        assert!(!source.path.join("smart_clipboard.sqlite").exists());
        assert!(target.path.join("smart_clipboard.sqlite").exists());
        assert!(target.path.join("images").join("item.png").exists());
        assert!(!target.path.join("SmartClipboard.exe").exists());
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn strips_verbatim_drive_prefix_for_restart() {
        assert_eq!(
            windows_restart_path(PathBuf::from(r"\\?\H:\Clipboard\SmartClipboard.exe")),
            r"H:\Clipboard\SmartClipboard.exe"
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn strips_verbatim_unc_prefix_for_restart() {
        assert_eq!(
            windows_restart_path(PathBuf::from(r"\\?\UNC\server\share\SmartClipboard.exe")),
            r"\\server\share\SmartClipboard.exe"
        );
    }
}
