use std::collections::HashMap;

use reqwest::Client;
use semver::Version;
use serde::Deserialize;

use crate::{environment::EnvironmentService, error_mapper::LauncherError, models::DshVersion};

const REGISTRY_URL: &str = "https://registry.npmjs.org/@deepseek-ai%2fdsh";

#[derive(Debug, Deserialize)]
struct RegistryMetadata {
    #[serde(rename = "dist-tags")]
    dist_tags: HashMap<String, String>,
    versions: HashMap<String, serde_json::Value>,
    #[serde(default)]
    time: HashMap<String, String>,
}

pub struct DshVersionService;

impl DshVersionService {
    pub async fn list(
        &self,
        environment: &EnvironmentService,
    ) -> Result<Vec<DshVersion>, LauncherError> {
        let client = Client::builder()
            .user_agent("DeepDash/1.0.0")
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .map_err(|error| {
                LauncherError::new("networkError", "无法创建 npm registry 请求。")
                    .with_detail(error.to_string())
            })?;
        let metadata = client
            .get(REGISTRY_URL)
            .send()
            .await
            .map_err(|error| {
                LauncherError::new("networkError", "无法读取 npm registry 版本信息。")
                    .with_detail(error.to_string())
                    .with_action("检查网络连接后重试。")
            })?
            .error_for_status()
            .map_err(|error| {
                LauncherError::new("networkError", "npm registry 返回了错误响应。")
                    .with_detail(error.to_string())
                    .with_action("检查网络连接后重试。")
            })?
            .json::<RegistryMetadata>()
            .await
            .map_err(|error| {
                LauncherError::new("networkError", "npm registry 返回的数据无法解析。")
                    .with_detail(error.to_string())
            })?;
        let current = environment.current_dsh_version().ok().flatten();
        let latest = metadata.dist_tags.get("latest").cloned();
        let latest_version = latest
            .as_deref()
            .and_then(|value| Version::parse(value).ok());
        let next = parsed_next_version(&metadata.versions, latest_version.as_ref());
        let mut parsed = metadata
            .versions
            .keys()
            .filter_map(|raw| {
                Version::parse(raw)
                    .ok()
                    .map(|version| (version, raw.clone()))
            })
            .collect::<Vec<_>>();
        parsed.sort_by(|left, right| right.0.cmp(&left.0));
        let latest_semver = latest
            .as_deref()
            .and_then(|value| Version::parse(value).ok());
        let mut selected = match latest_semver.as_ref() {
            Some(latest_version) => {
                let mut newer = parsed
                    .iter()
                    .filter(|(version, _)| version > latest_version)
                    .cloned()
                    .collect::<Vec<_>>();
                let mut older = parsed
                    .iter()
                    .filter(|(version, _)| version <= latest_version)
                    .take(6)
                    .cloned()
                    .collect::<Vec<_>>();
                newer.append(&mut older);
                newer
            }
            None => parsed.into_iter().take(80).collect(),
        };
        for required in [current.clone(), latest.clone(), next.clone()]
            .into_iter()
            .flatten()
        {
            if !selected.iter().any(|(_, raw)| raw == &required) {
                if let Some(version) = Version::parse(required.as_str()).ok() {
                    selected.push((version, required));
                }
            }
        }
        selected.sort_by(|left, right| right.0.cmp(&left.0));
        Ok(selected
            .into_iter()
            .map(|(version, raw)| {
                let mut tags = Vec::new();
                if current.as_deref() == Some(raw.as_str()) {
                    tags.push("当前".to_string());
                }
                if latest.as_deref() == Some(raw.as_str()) {
                    tags.push("latest".to_string());
                }
                if next.as_deref() == Some(raw.as_str()) {
                    tags.push("next".to_string());
                }
                let prerelease = !version.pre.is_empty();
                DshVersion {
                    version: raw.clone(),
                    tags,
                    prerelease,
                    stable: !prerelease,
                    current: current.as_deref() == Some(raw.as_str()),
                    installed: current.as_deref() == Some(raw.as_str()),
                    published_at: metadata.time.get(&raw).cloned(),
                }
            })
            .collect())
    }
}

fn parsed_next_version(
    versions: &HashMap<String, serde_json::Value>,
    latest: Option<&Version>,
) -> Option<String> {
    versions
        .keys()
        .filter_map(|raw| Version::parse(raw).ok().map(|version| (version, raw)))
        .filter(|(version, _)| latest.map(|current| version > current).unwrap_or(true))
        .max_by(|left, right| left.0.cmp(&right.0))
        .map(|(_, raw)| raw.clone())
}
