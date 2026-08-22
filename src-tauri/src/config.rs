use std::{fs, io, path::PathBuf};

use tauri::{AppHandle, Manager};

use crate::{error_mapper::limit_text, models::LauncherConfig};

const CONFIG_FILE: &str = "launcher-config.json";

pub struct LauncherConfigService {
    path: PathBuf,
}

impl LauncherConfigService {
    pub fn new(app: &AppHandle) -> Result<Self, String> {
        Ok(Self {
            path: data_root(app)?.join(CONFIG_FILE),
        })
    }

    pub fn load(&self) -> Result<LauncherConfig, String> {
        if !self.path.exists() {
            return Ok(LauncherConfig::default());
        }
        let content = fs::read_to_string(&self.path)
            .map_err(|error| format!("无法读取启动器配置：{error}"))?;
        let config = serde_json::from_str::<LauncherConfig>(&content)
            .map_err(|error| format!("启动器配置格式无效：{error}"))?;
        validate(&config).map_err(|error| error.message)?;
        Ok(config)
    }

    pub fn save(&self, config: LauncherConfig) -> Result<LauncherConfig, String> {
        validate(&config).map_err(|error| error.message)?;
        let content = serde_json::to_string_pretty(&config)
            .map_err(|error| format!("无法编码启动器配置：{error}"))?;
        atomic_write(&self.path, &content)
            .map_err(|error| format!("无法保存启动器配置：{error}"))?;
        Ok(config)
    }

    pub fn path(&self) -> PathBuf {
        self.path.clone()
    }
}

pub fn data_root(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_local_data_dir()
        .map_err(|error| format!("无法定位 DeepDash 用户数据目录：{error}"))
}

fn validate(config: &LauncherConfig) -> Result<(), crate::models::LauncherError> {
    if config.port == 0 {
        return Err(crate::models::LauncherError::new(
            "invalidPort",
            "端口必须在 1 到 65535 之间。",
        ));
    }
    if !matches!(config.theme.as_str(), "system" | "light" | "dark") {
        return Err(crate::models::LauncherError::new(
            "invalidTheme",
            "主题模式无效。",
        ));
    }
    Ok(())
}

fn atomic_write(path: &PathBuf, content: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temp = path.with_extension("json.tmp");
    fs::write(&temp, content.as_bytes())?;
    match fs::rename(&temp, path) {
        Ok(()) => Ok(()),
        Err(rename_error) => {
            let _ = fs::remove_file(&temp);
            Err(rename_error)
        }
    }
}

pub fn config_path_summary(service: &LauncherConfigService) -> String {
    limit_text(&format!("路径：{}", service.path().display()), 800)
}
