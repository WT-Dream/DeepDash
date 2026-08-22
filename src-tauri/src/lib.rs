mod config;
mod environment;
mod error_mapper;
mod models;
mod network;
mod package_service;
mod process_manager;
mod versions;

use std::{fs, process::Command, sync::Arc};

use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::Mutex;

use config::{config_path_summary, data_root, LauncherConfigService};
use environment::EnvironmentService;
use error_mapper::LauncherError;
use models::{DshLifecycleStatus, DshState, EnvironmentInfo, LanHost, LauncherConfig};
use package_service::DshPackageService;
use process_manager::DshProcessManager;
use versions::DshVersionService;

pub struct AppState {
    pub config: LauncherConfigService,
    pub environment: EnvironmentService,
    pub process: Arc<DshProcessManager>,
    pub operation_lock: Arc<Mutex<()>>,
    pub package_child: Arc<Mutex<Option<process_manager::ChildHandle>>>,
    pub package_cancelled: Arc<Mutex<bool>>,
    pub runtime: Arc<Mutex<DshState>>,
}

impl AppState {
    fn new(app: &AppHandle) -> Result<Self, String> {
        let runtime = Arc::new(Mutex::new(DshState::stopped(None)));
        Ok(Self {
            config: LauncherConfigService::new(app)?,
            environment: EnvironmentService,
            process: Arc::new(DshProcessManager::new(app.clone(), Arc::clone(&runtime))),
            operation_lock: Arc::new(Mutex::new(())),
            package_child: Arc::new(Mutex::new(None)),
            package_cancelled: Arc::new(Mutex::new(false)),
            runtime,
        })
    }
}

#[tauri::command]
fn get_launcher_config(state: State<'_, AppState>) -> Result<LauncherConfig, LauncherError> {
    state.config.load().map_err(|message| {
        LauncherError::new("configReadFailed", message)
            .with_detail(config_path_summary(&state.config))
    })
}

#[tauri::command]
fn save_launcher_config(
    config: LauncherConfig,
    state: State<'_, AppState>,
) -> Result<LauncherConfig, LauncherError> {
    state.config.save(config).map_err(|message| {
        LauncherError::new("configWriteFailed", message)
            .with_detail(config_path_summary(&state.config))
    })
}

#[tauri::command]
fn open_data_directory(app: AppHandle) -> Result<(), LauncherError> {
    let path = data_root(&app)
        .map_err(|message| LauncherError::new("dataDirectoryUnavailable", message))?;
    fs::create_dir_all(&path).map_err(|error| {
        LauncherError::new(
            "dataDirectoryUnavailable",
            "无法创建 DeepDash 用户数据目录。",
        )
        .with_detail(error.to_string())
    })?;
    Command::new("explorer.exe")
        .arg(&path)
        .spawn()
        .map_err(|error| {
            LauncherError::new(
                "dataDirectoryOpenFailed",
                "无法打开 DeepDash 用户数据目录。",
            )
            .with_detail(error.to_string())
        })?;
    Ok(())
}

#[tauri::command]
fn detect_environment(state: State<'_, AppState>) -> EnvironmentInfo {
    state.environment.detect()
}

#[tauri::command]
fn get_lan_hosts() -> Result<Vec<LanHost>, LauncherError> {
    network::lan_hosts()
}

#[tauri::command]
async fn get_dsh_versions(
    state: State<'_, AppState>,
) -> Result<Vec<models::DshVersion>, LauncherError> {
    DshVersionService.list(&state.environment).await
}

#[tauri::command]
fn get_dsh_current_version(state: State<'_, AppState>) -> Result<Option<String>, LauncherError> {
    state.environment.current_dsh_version()
}

#[tauri::command]
async fn install_or_switch_dsh_version(
    version: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<DshState, LauncherError> {
    let current = state.environment.current_dsh_version()?;
    let next_status = if current.is_some() {
        DshLifecycleStatus::SwitchingVersion
    } else {
        DshLifecycleStatus::Installing
    };
    publish(
        &app,
        &state,
        DshState {
            status: next_status,
            port: None,
            url: None,
            lan_url: None,
            lan_connected: false,
            current_version: current.clone(),
            error: None,
        },
    )
    .await;
    match DshPackageService
        .install_or_switch(
            &app,
            &state.operation_lock,
            &state.process,
            &state.package_child,
            &state.package_cancelled,
            &version,
            current,
        )
        .await
    {
        Ok(next) => {
            publish(&app, &state, next.clone()).await;
            Ok(next)
        }
        Err(error) => {
            if error.kind == "operationCanceled" {
                let next = DshState::ready(state.environment.current_dsh_version().ok().flatten());
                publish(&app, &state, next).await;
                return Err(error);
            }
            let status = if error.kind == "portConflict" {
                DshLifecycleStatus::PortConflict
            } else {
                DshLifecycleStatus::StartFailed
            };
            let next = DshState {
                status,
                port: error.port,
                url: None,
                lan_url: None,
                lan_connected: false,
                current_version: state.environment.current_dsh_version().ok().flatten(),
                error: Some(error.clone()),
            };
            publish(&app, &state, next).await;
            Err(error)
        }
    }
}

#[tauri::command]
async fn cancel_package_operation(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<DshState, LauncherError> {
    package_service::cancel(&state.package_child, &state.package_cancelled).await;
    let next = DshState::ready(state.environment.current_dsh_version().ok().flatten());
    publish(&app, &state, next.clone()).await;
    Ok(next)
}

#[tauri::command]
async fn start_dsh(
    port: u16,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<DshState, LauncherError> {
    let _operation = state.operation_lock.lock().await;
    let current = state.environment.current_dsh_version()?;
    let config = state.config.load().map_err(|message| {
        LauncherError::new("configReadFailed", message)
            .with_detail(config_path_summary(&state.config))
    })?;
    let lan_host = if config.lan_enabled {
        config.lan_host.as_deref()
    } else {
        None
    };
    publish(
        &app,
        &state,
        DshState {
            status: DshLifecycleStatus::Starting,
            port: Some(port),
            url: None,
            lan_url: None,
            lan_connected: false,
            current_version: current.clone(),
            error: None,
        },
    )
    .await;
    match state.process.start(port, current.clone(), lan_host).await {
        Ok(next) => {
            publish(&app, &state, next.clone()).await;
            Ok(next)
        }
        Err(error) => {
            let status = match error.kind.as_str() {
                "portConflict" => DshLifecycleStatus::PortConflict,
                "startupTimeout" => DshLifecycleStatus::StartupTimeout,
                _ => DshLifecycleStatus::StartFailed,
            };
            let next = DshState {
                status,
                port: error.port.or(Some(port)),
                url: None,
                lan_url: None,
                lan_connected: false,
                current_version: current,
                error: Some(error.clone()),
            };
            publish(&app, &state, next).await;
            Err(error)
        }
    }
}

#[tauri::command]
async fn stop_dsh(app: AppHandle, state: State<'_, AppState>) -> Result<DshState, LauncherError> {
    let _operation = state.operation_lock.lock().await;
    let current = state.environment.current_dsh_version().ok().flatten();
    publish(
        &app,
        &state,
        DshState {
            status: DshLifecycleStatus::Stopping,
            port: None,
            url: None,
            lan_url: None,
            lan_connected: false,
            current_version: current.clone(),
            error: None,
        },
    )
    .await;
    let next = state.process.stop(current).await?;
    publish(&app, &state, next.clone()).await;
    Ok(next)
}

#[tauri::command]
async fn get_dsh_status(state: State<'_, AppState>) -> Result<DshState, LauncherError> {
    Ok(state.runtime.lock().await.clone())
}

async fn publish(app: &AppHandle, state: &AppState, value: DshState) {
    *state.runtime.lock().await = value.clone();
    let _ = app.emit("launcher://dsh-state", value);
}

fn tray_image() -> tauri::image::Image<'static> {
    tauri::image::Image::new(include_bytes!("../icons/tray-icon.rgba"), 32, 32)
}

fn build_tray(app: &tauri::AppHandle) -> tauri::Result<()> {
    use tauri::menu::{MenuBuilder, MenuItemBuilder};
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
    let show = MenuItemBuilder::with_id("show", "显示 DeepDash").build(app)?;
    let restart = MenuItemBuilder::with_id("restart", "重启 DSH 服务").build(app)?;
    let quit = MenuItemBuilder::with_id("quit", "退出").build(app)?;
    let menu = MenuBuilder::new(app)
        .items(&[&show, &restart, &quit])
        .build()?;
    TrayIconBuilder::new()
        .icon(tray_image())
        .menu(&menu)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "restart" => {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    restart_dsh(&app).await;
                });
            }
            "quit" => {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    let state = app.state::<AppState>();
                    let _ = state
                        .process
                        .stop(state.environment.current_dsh_version().ok().flatten())
                        .await;
                    app.exit(0);
                });
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                if let Some(window) = tray.app_handle().get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        })
        .build(app)?;
    Ok(())
}

async fn restart_dsh(app: &AppHandle) {
    let state = app.state::<AppState>();
    let _operation = state.operation_lock.lock().await;
    let current = state.environment.current_dsh_version().ok().flatten();
    publish(
        app,
        &state,
        DshState {
            status: DshLifecycleStatus::Stopping,
            port: None,
            url: None,
            lan_url: None,
            lan_connected: false,
            current_version: current.clone(),
            error: None,
        },
    )
    .await;
    if let Err(error) = state.process.stop(current.clone()).await {
        let next = DshState {
            status: DshLifecycleStatus::StartFailed,
            port: None,
            url: None,
            lan_url: None,
            lan_connected: false,
            current_version: current,
            error: Some(error),
        };
        publish(app, &state, next).await;
        return;
    }
    let config = match state.config.load() {
        Ok(config) => config,
        Err(message) => {
            let error = LauncherError::new("configReadFailed", message)
                .with_detail(config_path_summary(&state.config));
            let next = DshState {
                status: DshLifecycleStatus::StartFailed,
                port: None,
                url: None,
                lan_url: None,
                lan_connected: false,
                current_version: current,
                error: Some(error),
            };
            publish(app, &state, next).await;
            return;
        }
    };
    publish(
        app,
        &state,
        DshState {
            status: DshLifecycleStatus::Starting,
            port: Some(config.port),
            url: None,
            lan_url: None,
            lan_connected: false,
            current_version: current.clone(),
            error: None,
        },
    )
    .await;
    let lan_host = if config.lan_enabled {
        config.lan_host.as_deref()
    } else {
        None
    };
    match state
        .process
        .start(config.port, current.clone(), lan_host)
        .await
    {
        Ok(next) => publish(app, &state, next).await,
        Err(error) => {
            let status = match error.kind.as_str() {
                "portConflict" => DshLifecycleStatus::PortConflict,
                "startupTimeout" => DshLifecycleStatus::StartupTimeout,
                _ => DshLifecycleStatus::StartFailed,
            };
            let next = DshState {
                status,
                port: error.port.or(Some(config.port)),
                url: None,
                lan_url: None,
                lan_connected: false,
                current_version: current,
                error: Some(error),
            };
            publish(app, &state, next).await;
        }
    }
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .setup(|app| {
            let state = AppState::new(app.handle())?;
            app.manage(state);
            build_tray(app.handle())?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let app = window.app_handle();
                api.prevent_close();
                let window = window.clone();
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    let state = app.state::<AppState>();
                    let current_status = state.runtime.lock().await.clone().status;
                    let running = matches!(
                        current_status,
                        DshLifecycleStatus::Starting
                            | DshLifecycleStatus::Running
                            | DshLifecycleStatus::Stopping
                    );
                    if running {
                        let current = state.environment.current_dsh_version().ok().flatten();
                        if let Ok(next) = state.process.stop(current).await {
                            publish(&app, &state, next).await;
                        }
                        let _ = window.show();
                        let _ = window.set_focus();
                    } else {
                        let _ = window.hide();
                    }
                });
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_launcher_config,
            save_launcher_config,
            open_data_directory,
            detect_environment,
            get_lan_hosts,
            get_dsh_versions,
            get_dsh_current_version,
            install_or_switch_dsh_version,
            cancel_package_operation,
            start_dsh,
            stop_dsh,
            get_dsh_status
        ])
        .run(tauri::generate_context!())
        .expect("error while running DeepDash");
}
