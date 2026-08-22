use std::process::Stdio;
use std::sync::Arc;

use tauri::{AppHandle, Emitter};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command as TokioCommand,
    sync::Mutex,
    time::{sleep, Duration},
};

use crate::{
    environment::{command, EnvironmentService},
    error_mapper::{limit_text, map_npm_failure, LauncherError},
    models::{DshState, OperationProgress},
    process_manager::{request_terminate, ChildHandle, DshProcessManager},
};

pub struct DshPackageService;

impl DshPackageService {
    pub async fn install_or_switch(
        &self,
        app: &AppHandle,
        lock: &Arc<Mutex<()>>,
        process: &DshProcessManager,
        package_child: &Arc<Mutex<Option<ChildHandle>>>,
        package_cancelled: &Arc<Mutex<bool>>,
        version: &str,
        current_version: Option<String>,
    ) -> Result<DshState, LauncherError> {
        validate_version(version)?;
        let _operation = lock.lock().await;
        *package_cancelled.lock().await = false;
        if current_version.is_some() {
            let _ = process.stop(current_version.clone()).await?;
        }
        let npm_path = EnvironmentService.npm_path()?;
        let package = format!("@deepseek-ai/dsh@{version}");
        let args = vec![
            "install",
            "--global",
            "--no-audit",
            "--no-fund",
            package.as_str(),
        ];
        emit_progress(
            app,
            OperationProgress {
                operation: if current_version.as_deref() == Some(version) {
                    "install"
                } else {
                    "switch"
                }
                .to_string(),
                phase: "npm".to_string(),
                message: format!("正在通过本机 npm 安装 {version}"),
                percent: Some(25),
            },
        );
        let mut child = TokioCommand::from(command(&npm_path, &args));
        child
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null());
        let mut child = child.spawn().map_err(|error| {
            LauncherError::new("npmInstallFailed", "无法启动 npm 全局安装。")
                .with_detail(error.to_string())
        })?;
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let output_log = Arc::new(Mutex::new(String::new()));
        spawn_reader(stdout, Arc::clone(&output_log));
        spawn_reader(stderr, Arc::clone(&output_log));
        let child = Arc::new(Mutex::new(child));
        *package_child.lock().await = Some(Arc::clone(&child));
        let status = loop {
            match child.lock().await.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) => sleep(Duration::from_millis(250)).await,
                Err(error) => {
                    return Err(LauncherError::new(
                        "npmInstallFailed",
                        "无法读取 npm 全局安装状态。",
                    )
                    .with_detail(error.to_string()))
                }
            }
        };
        let cancelled = *package_cancelled.lock().await;
        *package_child.lock().await = None;
        *package_cancelled.lock().await = false;
        if cancelled {
            return Err(LauncherError::new(
                "operationCanceled",
                "已取消 npm 全局安装。",
            ));
        }
        if !status.success() {
            let text = output_log.lock().await.clone();
            return Err(map_npm_failure(&limit_text(&text, 1800)));
        }
        emit_progress(
            app,
            OperationProgress {
                operation: "switch".to_string(),
                phase: "verify".to_string(),
                message: "正在确认全局 dsh 版本".to_string(),
                percent: Some(90),
            },
        );
        let detected = EnvironmentService.detect();
        let current = detected.dsh.version.clone();
        if current.as_deref() != Some(version) {
            return Err(LauncherError::new(
                "verificationFailed",
                "npm 已完成，但 dsh --version 与目标版本不一致。",
            )
            .with_detail(format!(
                "检测到：{}；目标：{version}",
                current.as_deref().unwrap_or("未知")
            ))
            .with_action("重新检测环境，确认 PATH 指向本机 npm global prefix。"));
        }
        emit_progress(
            app,
            OperationProgress {
                operation: "switch".to_string(),
                phase: "done".to_string(),
                message: format!("已切换到 DSH {version}"),
                percent: Some(100),
            },
        );
        Ok(DshState::ready(current))
    }
}

pub async fn cancel(
    package_child: &Arc<Mutex<Option<ChildHandle>>>,
    package_cancelled: &Arc<Mutex<bool>>,
) {
    *package_cancelled.lock().await = true;
    let child = package_child.lock().await.as_ref().cloned();
    if let Some(child) = child {
        request_terminate(&child).await;
    }
}

fn validate_version(version: &str) -> Result<(), LauncherError> {
    if version.len() > 120 || semver::Version::parse(version).is_err() {
        return Err(LauncherError::new("invalidVersion", "版本号格式无效。"));
    }
    Ok(())
}

fn emit_progress(app: &AppHandle, progress: OperationProgress) {
    let _ = app.emit("launcher://operation-progress", progress);
}

fn spawn_reader<T>(pipe: Option<T>, log: Arc<Mutex<String>>)
where
    T: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    let Some(pipe) = pipe else {
        return;
    };
    tauri::async_runtime::spawn(async move {
        let mut lines = BufReader::new(pipe).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let mut output = log.lock().await;
            if output.len() < 4000 {
                output.push_str(&limit_text(&line, 500));
                output.push('\n');
            }
        }
    });
}
