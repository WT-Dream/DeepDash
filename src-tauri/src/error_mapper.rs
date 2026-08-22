use std::fmt::{Display, Formatter};

pub use crate::models::LauncherError;

impl Display for LauncherError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for LauncherError {}

impl LauncherError {
    pub fn new(kind: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            message: message.into(),
            detail: None,
            action: None,
            port: None,
        }
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(limit_text(&detail.into(), 1200));
        self
    }

    pub fn with_action(mut self, action: impl Into<String>) -> Self {
        self.action = Some(action.into());
        self
    }

    pub fn with_port(mut self, port: u16) -> Self {
        self.port = Some(port);
        self
    }
}

pub fn limit_text(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

pub fn map_process_failure(output: &str, port: Option<u16>) -> LauncherError {
    let lower = output.to_ascii_lowercase();
    if lower.contains("eaddrinuse")
        || lower.contains("address already in use")
        || lower.contains("only one usage of each socket address")
    {
        return LauncherError::new("portConflict", "目标端口已被其他程序占用。")
            .with_action("请在设置中更换端口，或关闭占用该端口的程序。")
            .with_port(port.unwrap_or_default());
    }
    if lower.contains("permission denied") || lower.contains("access is denied") {
        return LauncherError::new("permissionDenied", "系统拒绝了 npm 或 DSH 操作。")
            .with_detail(limit_text(output, 1200))
            .with_action("检查 npm global prefix 的写入权限，不要修改 DSH 数据目录。")
            .with_port(port.unwrap_or_default());
    }
    LauncherError::new("processExited", "DSH 进程在就绪前退出。")
        .with_detail(limit_text(output, 1200))
        .with_action("检查 Node.js、npm 和 DSH 安装后重试。")
}

pub fn map_npm_failure(output: &str) -> LauncherError {
    let lower = output.to_ascii_lowercase();
    let kind = if lower.contains("eacces")
        || lower.contains("permission denied")
        || lower.contains("access is denied")
    {
        "permissionDenied"
    } else {
        "npmInstallFailed"
    };
    LauncherError::new(kind, "npm 全局安装未完成。")
        .with_detail(limit_text(output, 1600))
        .with_action("确认 npm 默认 global prefix 可写，并检查网络后重试。")
}
