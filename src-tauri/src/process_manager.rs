use std::{
    net::{SocketAddr, TcpStream},
    process::Stdio,
    sync::Arc,
    time::Duration,
};

use tauri::{AppHandle, Emitter};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::{Child, Command as TokioCommand},
    sync::Mutex,
    time::{sleep, timeout},
};

use crate::{
    environment::{command, EnvironmentService},
    error_mapper::{map_process_failure, LauncherError},
    models::{DshLifecycleStatus, DshState, LauncherError as ModelError, OperationProgress},
};

pub type ChildHandle = Arc<Mutex<Child>>;

pub struct DshProcessManager {
    child: Arc<Mutex<Option<ChildHandle>>>,
    port: Arc<Mutex<Option<u16>>>,
    web_url: Arc<Mutex<Option<String>>>,
    stopping: Arc<Mutex<bool>>,
    runtime: Arc<Mutex<DshState>>,
    app: AppHandle,
}

impl DshProcessManager {
    pub fn new(app: AppHandle, runtime: Arc<Mutex<DshState>>) -> Self {
        Self {
            child: Arc::new(Mutex::new(None)),
            port: Arc::new(Mutex::new(None)),
            web_url: Arc::new(Mutex::new(None)),
            stopping: Arc::new(Mutex::new(false)),
            runtime,
            app,
        }
    }

    pub async fn start(
        &self,
        port: u16,
        current_version: Option<String>,
        lan_host: Option<&str>,
    ) -> Result<DshState, LauncherError> {
        if port == 0 {
            return Err(LauncherError::new(
                "invalidPort",
                "端口必须在 1 到 65535 之间。",
            ));
        }
        if let Some(existing) = self.child.lock().await.as_ref().cloned() {
            let mut process = existing.lock().await;
            if process
                .try_wait()
                .map_err(|error| {
                    LauncherError::new("processFailed", "无法读取 DSH 进程状态。")
                        .with_detail(error.to_string())
                })?
                .is_none()
            {
                return Ok(DshState {
                    status: DshLifecycleStatus::Running,
                    port: *self.port.lock().await,
                    url: self.web_url.lock().await.clone(),
                    lan_url: self
                        .web_url
                        .lock()
                        .await
                        .clone()
                        .filter(|url| !url.contains("127.0.0.1")),
                    current_version,
                    error: None,
                });
            }
            drop(process);
            self.child.lock().await.take();
            self.port.lock().await.take();
            self.web_url.lock().await.take();
        }
        let environment = EnvironmentService;
        let dsh_path = environment.dsh_path()?;
        let lan_host = lan_host
            .map(crate::network::selected_lan_host)
            .transpose()?
            .map(|host| host.address);
        let bind_host = lan_host.as_deref().unwrap_or("127.0.0.1");
        if port_is_in_use(bind_host, port) {
            return Err(
                LauncherError::new("portConflict", "目标端口已被其他程序占用。")
                    .with_action("请关闭占用该端口的 DSH 进程，或在设置中更换端口。")
                    .with_port(port),
            );
        }
        let args = command_args(port, lan_host.as_deref());
        let mut process = spawn_command(&dsh_path, &args)?;
        let stdout = process.stdout.take();
        let stderr = process.stderr.take();
        let log = Arc::new(Mutex::new(String::new()));
        spawn_reader(stdout, Arc::clone(&log));
        spawn_reader(stderr, Arc::clone(&log));
        let process = Arc::new(Mutex::new(process));
        let target_url = url(bind_host, port);
        self.publish_progress(OperationProgress {
            operation: "start".to_string(),
            phase: "healthCheck".to_string(),
            message: format!("正在等待 DSH Web 在 {target_url} 就绪"),
            percent: None,
        });
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(900))
            .build()
            .map_err(|error| {
                LauncherError::new("networkError", "无法执行本地健康检查。")
                    .with_detail(error.to_string())
            })?;
        let ready = timeout(Duration::from_secs(45), async {
            loop {
                {
                    let mut child = process.lock().await;
                    if let Some(exit) = child.try_wait().map_err(|error| {
                        LauncherError::new("processFailed", "无法读取 DSH 进程状态。")
                            .with_detail(error.to_string())
                    })? {
                        let output = log.lock().await.clone();
                        return Err(map_process_failure(
                            &format!("进程退出码：{exit}\n{output}"),
                            Some(port),
                        ));
                    }
                }
                if let Ok(response) = client.get(&target_url).send().await {
                    if response.status().is_success() {
                        return Ok(());
                    }
                }
                sleep(Duration::from_millis(500)).await;
            }
        })
        .await;
        match ready {
            Ok(Ok(())) => {
                *self.child.lock().await = Some(Arc::clone(&process));
                *self.port.lock().await = Some(port);
                *self.web_url.lock().await = Some(target_url.clone());
                *self.stopping.lock().await = false;
                self.watch(process, port, current_version.clone(), log);
                Ok(DshState {
                    status: DshLifecycleStatus::Running,
                    port: Some(port),
                    url: Some(target_url),
                    lan_url: lan_host.map(|host| url(&host, port)),
                    current_version,
                    error: None,
                })
            }
            Ok(Err(error)) => {
                terminate_child(&process).await;
                Err(error)
            }
            Err(_) => {
                terminate_child(&process).await;
                Err(
                    LauncherError::new("startupTimeout", "DSH 在 45 秒内没有响应。")
                        .with_action("检查端口、网络和 DSH 日志后重试。"),
                )
            }
        }
    }

    pub async fn stop(&self, current_version: Option<String>) -> Result<DshState, LauncherError> {
        *self.stopping.lock().await = true;
        let child = self.child.lock().await.take();
        let port = self.port.lock().await.take();
        self.web_url.lock().await.take();
        if let Some(child) = child {
            terminate_child(&child).await;
        }
        Ok(DshState {
            status: DshLifecycleStatus::Stopped,
            port,
            url: None,
            lan_url: None,
            current_version,
            error: None,
        })
    }

    fn watch(
        &self,
        process: ChildHandle,
        port: u16,
        current_version: Option<String>,
        log: Arc<Mutex<String>>,
    ) {
        let child_store = Arc::clone(&self.child);
        let port_store = Arc::clone(&self.port);
        let web_url_store = Arc::clone(&self.web_url);
        let stopping = Arc::clone(&self.stopping);
        let runtime = Arc::clone(&self.runtime);
        let app = self.app.clone();
        tauri::async_runtime::spawn(async move {
            let exit = loop {
                let result = process.lock().await.try_wait();
                match result {
                    Ok(Some(status)) => break Ok(status),
                    Ok(None) => sleep(Duration::from_millis(250)).await,
                    Err(error) => break Err(error),
                }
            };
            let was_stopping = *stopping.lock().await;
            let mut stored = child_store.lock().await;
            let was_owned = stored
                .as_ref()
                .is_some_and(|candidate| Arc::ptr_eq(candidate, &process));
            if was_owned {
                stored.take();
                *port_store.lock().await = None;
                web_url_store.lock().await.take();
            }
            drop(stored);
            if was_stopping || !was_owned {
                return;
            }
            let detail = match exit {
                Ok(value) => format!("进程退出码：{value}\n{}", log.lock().await.clone()),
                Err(error) => error.to_string(),
            };
            let error = ModelError::new("processExited", "DSH 进程意外退出。")
                .with_detail(crate::error_mapper::limit_text(&detail, 1200))
                .with_action("确认端口和 DSH 环境后重新启动。");
            let next = DshState {
                status: DshLifecycleStatus::StartFailed,
                port: Some(port),
                url: None,
                lan_url: None,
                current_version,
                error: Some(error),
            };
            *runtime.lock().await = next.clone();
            let _ = app.emit("launcher://dsh-state", next);
        });
    }

    fn publish_progress(&self, progress: OperationProgress) {
        let _ = self.app.emit("launcher://operation-progress", progress);
    }
}

fn spawn_command(path: &std::path::Path, args: &[String]) -> Result<Child, LauncherError> {
    let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    let base = command(path, &refs);
    let mut process = TokioCommand::from(base);
    process
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null());
    process.spawn().map_err(|error| {
        LauncherError::new("startFailed", "无法启动 dsh web。").with_detail(error.to_string())
    })
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
                output.push_str(&crate::error_mapper::limit_text(&line, 500));
                output.push('\n');
            }
        }
    });
}

pub async fn terminate_child(process: &ChildHandle) {
    request_terminate(process).await;
    let mut child = process.lock().await;
    if child.try_wait().ok().flatten().is_some() {
        return;
    }
    if timeout(Duration::from_secs(3), child.wait()).await.is_err() {
        let _ = child.kill().await;
        let _ = child.wait().await;
    }
}

pub async fn request_terminate(process: &ChildHandle) {
    let mut child = process.lock().await;
    let pid = child.id();
    if let Some(pid) = pid {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let mut taskkill = std::process::Command::new("taskkill");
        taskkill.creation_flags(CREATE_NO_WINDOW);
        let _ = taskkill
            .arg("/PID")
            .arg(pid.to_string())
            .arg("/T")
            .arg("/F")
            .output();
    }
    let _ = child.start_kill();
}

fn command_args(port: u16, lan_host: Option<&str>) -> Vec<String> {
    let mut args = vec!["web".to_string(), "--no-open".to_string()];
    if let Some(host) = lan_host {
        args.push("--host".to_string());
        args.push(host.to_string());
        args.push("--trusted-host".to_string());
        args.push(format!("{host}:{port}"));
    }
    args.push("--port".to_string());
    args.push(port.to_string());
    args
}

fn url(host: &str, port: u16) -> String {
    format!("http://{host}:{port}")
}

fn port_is_in_use(host: &str, port: u16) -> bool {
    let Ok(address) = format!("{host}:{port}").parse::<SocketAddr>() else {
        return false;
    };
    TcpStream::connect_timeout(&address, Duration::from_millis(100)).is_ok()
}

#[cfg(test)]
mod tests {
    use super::command_args;

    #[test]
    fn builds_loopback_start_arguments_by_default() {
        assert_eq!(
            command_args(3080, None),
            ["web", "--no-open", "--port", "3080"]
        );
    }

    #[test]
    fn builds_lan_start_arguments_with_trusted_host() {
        assert_eq!(
            command_args(3080, Some("192.168.2.9")),
            [
                "web",
                "--no-open",
                "--host",
                "192.168.2.9",
                "--trusted-host",
                "192.168.2.9:3080",
                "--port",
                "3080"
            ]
        );
    }
}
