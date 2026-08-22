use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LauncherConfig {
    pub port: u16,
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default)]
    pub lan_enabled: bool,
    #[serde(default)]
    pub lan_host: Option<String>,
}

impl Default for LauncherConfig {
    fn default() -> Self {
        Self {
            port: 3080,
            theme: default_theme(),
            lan_enabled: false,
            lan_host: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LanHost {
    pub name: String,
    pub address: String,
}

fn default_theme() -> String {
    "system".to_string()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolInfo {
    pub found: bool,
    pub path: Option<String>,
    pub version: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentInfo {
    pub node: ToolInfo,
    pub npm: ToolInfo,
    pub prefix: Option<String>,
    pub dsh: ToolInfo,
    pub status: EnvironmentStatus,
    pub checked_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum EnvironmentStatus {
    Ready,
    MissingNode,
    BrokenNode,
    MissingNpm,
    BrokenNpm,
    MissingDsh,
    BrokenDsh,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DshVersion {
    pub version: String,
    pub tags: Vec<String>,
    pub prerelease: bool,
    pub stable: bool,
    pub current: bool,
    pub installed: bool,
    pub published_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LauncherError {
    pub kind: String,
    pub message: String,
    pub detail: Option<String>,
    pub action: Option<String>,
    pub port: Option<u16>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DshLifecycleStatus {
    NotInstalled,
    ReadyToStart,
    Installing,
    SwitchingVersion,
    Starting,
    Running,
    Stopping,
    Stopped,
    PortConflict,
    StartFailed,
    StartupTimeout,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DshState {
    pub status: DshLifecycleStatus,
    pub port: Option<u16>,
    pub url: Option<String>,
    pub lan_url: Option<String>,
    pub lan_connected: bool,
    pub current_version: Option<String>,
    pub error: Option<LauncherError>,
}

impl DshState {
    pub fn ready(version: Option<String>) -> Self {
        Self {
            status: DshLifecycleStatus::ReadyToStart,
            port: None,
            url: None,
            lan_url: None,
            lan_connected: false,
            current_version: version,
            error: None,
        }
    }

    pub fn stopped(version: Option<String>) -> Self {
        Self {
            status: DshLifecycleStatus::Stopped,
            port: None,
            url: None,
            lan_url: None,
            lan_connected: false,
            current_version: version,
            error: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationProgress {
    pub operation: String,
    pub phase: String,
    pub message: String,
    pub percent: Option<u8>,
}
