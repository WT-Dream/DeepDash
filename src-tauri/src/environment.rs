use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
    time::SystemTime,
};

use semver::Version;

use crate::{
    error_mapper::{limit_text, LauncherError},
    models::{EnvironmentInfo, EnvironmentStatus, ToolInfo},
};

pub struct EnvironmentService;

impl EnvironmentService {
    pub fn detect(&self) -> EnvironmentInfo {
        let node_path = resolve_command("node", &[]);
        let node = tool_info(node_path.clone(), "--version");
        if !node.found {
            return EnvironmentInfo {
                node,
                npm: missing_tool(),
                prefix: None,
                dsh: missing_tool(),
                status: EnvironmentStatus::MissingNode,
                checked_at: now(),
            };
        }
        if node.version.is_none() {
            return EnvironmentInfo {
                node,
                npm: missing_tool(),
                prefix: None,
                dsh: missing_tool(),
                status: EnvironmentStatus::BrokenNode,
                checked_at: now(),
            };
        }

        let npm_path = node_path
            .as_ref()
            .and_then(|path| path.parent())
            .and_then(|parent| resolve_command("npm", &[parent.to_path_buf()]))
            .or_else(|| resolve_command("npm", &[]));
        let npm = tool_info(npm_path.clone(), "--version");
        if !npm.found {
            return EnvironmentInfo {
                node,
                npm,
                prefix: None,
                dsh: missing_tool(),
                status: EnvironmentStatus::MissingNpm,
                checked_at: now(),
            };
        }
        if npm.version.is_none() {
            return EnvironmentInfo {
                node,
                npm,
                prefix: None,
                dsh: missing_tool(),
                status: EnvironmentStatus::BrokenNpm,
                checked_at: now(),
            };
        }

        let prefix = if let Some(path) = npm_path.as_ref() {
            match run_capture(path, &["config", "get", "prefix"]) {
                Ok(value) => Some(value.trim().to_string()).filter(|value| !value.is_empty()),
                Err(error) => {
                    return EnvironmentInfo {
                        node,
                        npm: ToolInfo {
                            error: Some(error.message),
                            ..npm
                        },
                        prefix: None,
                        dsh: missing_tool(),
                        status: EnvironmentStatus::BrokenNpm,
                        checked_at: now(),
                    }
                }
            }
        } else {
            None
        };
        let extra = prefix
            .as_ref()
            .map(|value| prefix_bin(Path::new(value)))
            .into_iter()
            .collect::<Vec<_>>();
        let dsh_path = resolve_command("dsh", &extra);
        let dsh = tool_info(dsh_path, "--version");
        let status = if !dsh.found {
            EnvironmentStatus::MissingDsh
        } else if dsh.version.is_none() {
            EnvironmentStatus::BrokenDsh
        } else {
            EnvironmentStatus::Ready
        };
        EnvironmentInfo {
            node,
            npm,
            prefix,
            dsh,
            status,
            checked_at: now(),
        }
    }

    pub fn current_dsh_version(&self) -> Result<Option<String>, LauncherError> {
        let detected = self.detect();
        if !detected.dsh.found {
            return Ok(None);
        }
        Ok(detected.dsh.version)
    }

    pub fn npm_path(&self) -> Result<PathBuf, LauncherError> {
        let node_path = resolve_command("node", &[]);
        node_path
            .as_ref()
            .and_then(|path| path.parent())
            .and_then(|parent| resolve_command("npm", &[parent.to_path_buf()]))
            .or_else(|| resolve_command("npm", &[]))
            .ok_or_else(|| {
                LauncherError::new("npmMissing", "未检测到 npm 可执行文件。")
                    .with_action("修复 Node.js/npm 安装或检查 PATH，然后重新启动启动器。")
            })
    }

    pub fn dsh_path(&self) -> Result<PathBuf, LauncherError> {
        let detected = self.detect();
        detected.dsh.path.map(PathBuf::from).ok_or_else(|| {
            LauncherError::new("dshMissing", "未检测到全局 dsh 命令。")
                .with_action("前往版本管理页安装 @deepseek-ai/dsh。")
        })
    }
}

pub fn resolve_command(name: &str, extra_paths: &[PathBuf]) -> Option<PathBuf> {
    let raw_name = Path::new(name);
    if raw_name.is_absolute() && raw_name.exists() {
        return Some(raw_name.to_path_buf());
    }
    let mut roots = extra_paths.to_vec();
    if let Some(path_var) = env::var_os("PATH") {
        roots.extend(env::split_paths(&path_var));
    }
    let candidates = command_candidates(name);
    roots.into_iter().find_map(|root| {
        candidates
            .iter()
            .map(|candidate| root.join(candidate))
            .find(|candidate| candidate.is_file())
    })
}

fn command_candidates(name: &str) -> Vec<String> {
    if name.ends_with(".exe") || name.ends_with(".cmd") || name.ends_with(".bat") {
        return vec![name.to_string()];
    }
    vec![
        format!("{name}.exe"),
        format!("{name}.cmd"),
        format!("{name}.bat"),
        name.to_string(),
    ]
}

pub fn prefix_bin(prefix: &Path) -> PathBuf {
    prefix.to_path_buf()
}

pub fn run_capture(path: &Path, args: &[&str]) -> Result<String, LauncherError> {
    let output = command(path, args).output().map_err(|error| {
        LauncherError::new("commandFailed", format!("无法执行 {}。", path.display()))
            .with_detail(error.to_string())
    })?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}\n{stderr}");
    if !output.status.success() {
        return Err(LauncherError::new(
            "commandFailed",
            format!("命令 {} 执行失败。", path.display()),
        )
        .with_detail(limit_text(&combined, 1200)));
    }
    Ok(limit_text(stdout.trim(), 300))
}

pub fn command(path: &Path, args: &[&str]) -> Command {
    if path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("cmd") || extension.eq_ignore_ascii_case("bat")
        })
    {
        let mut command = Command::new("cmd.exe");
        command.arg("/d").arg("/s").arg("/c").arg(path);
        command.args(args);
        hide_console_window(&mut command);
        return command;
    }
    let mut command = Command::new(path);
    command.args(args);
    hide_console_window(&mut command);
    command
}

fn hide_console_window(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    command.creation_flags(CREATE_NO_WINDOW);
}

fn tool_info(path: Option<PathBuf>, version_arg: &str) -> ToolInfo {
    let Some(path) = path else {
        return missing_tool();
    };
    match run_capture(&path, &[version_arg]) {
        Ok(version) => ToolInfo {
            found: true,
            path: Some(path.display().to_string()),
            version: Some(normalize_version(&version)),
            error: None,
        },
        Err(error) => ToolInfo {
            found: true,
            path: Some(path.display().to_string()),
            version: None,
            error: Some(error.message),
        },
    }
}

fn missing_tool() -> ToolInfo {
    ToolInfo {
        found: false,
        path: None,
        version: None,
        error: None,
    }
}

fn normalize_version(value: &str) -> String {
    value
        .split_whitespace()
        .map(|token| {
            token
                .trim_matches(|character: char| {
                    !character.is_ascii_alphanumeric()
                        && character != '.'
                        && character != '-'
                        && character != '+'
                })
                .trim_start_matches('v')
        })
        .find(|token| Version::parse(token).is_ok())
        .unwrap_or_else(|| {
            value
                .lines()
                .next()
                .unwrap_or(value)
                .trim()
                .trim_start_matches('v')
        })
        .to_string()
}

fn now() -> String {
    let seconds = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    seconds.to_string()
}
