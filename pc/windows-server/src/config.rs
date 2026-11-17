//! 持久化配置：%APPDATA%\VoiceRemoteControl\config.json
//! 内容：{ "token": "<hex>", "auth_required": true, "device_name": "<COMPUTERNAME>" }
//! 开机自启不入此文件——以注册表 HKCU\...\Run 为准。

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Serialize, Deserialize)]
pub struct Config {
    /// 配对令牌（WS 认证用），首次启动随机生成
    pub token: String,
    /// 是否强制未配对设备认证（默认 true，管理页可关闭）
    #[serde(default = "default_true")]
    pub auth_required: bool,
    /// 设备名称：写入配对链接/二维码，手机端展示用（默认取 COMPUTERNAME）
    /// serde 缺省为空串，由 load_or_create 归一化为默认值并落盘
    #[serde(default)]
    pub device_name: String,
}

fn default_true() -> bool {
    true
}

pub(crate) fn default_device_name() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .map(|s| s.trim().to_string())
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Windows 电脑".to_string())
}

impl Default for Config {
    fn default() -> Self {
        Config {
            token: generate_token(),
            auth_required: true,
            device_name: default_device_name(),
        }
    }
}

pub fn generate_token() -> String {
    let mut buf = [0u8; 12];
    rand::Rng::fill(&mut rand::thread_rng(), &mut buf);
    buf.iter().map(|b| format!("{b:02x}")).collect()
}

fn config_dir() -> Result<PathBuf> {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .context("APPDATA not set")?
        .join("VoiceRemoteControl");
    std::fs::create_dir_all(&base)?;
    Ok(base)
}

pub fn load_or_create() -> Config {
    let path = config_dir().map(|d| d.join("config.json"));
    let path = match path {
        Ok(p) => p,
        Err(e) => {
            log::warn!("config dir unavailable ({e:#}); using in-memory config");
            return Config::default();
        }
    };
    if let Ok(text) = std::fs::read_to_string(&path) {
        match serde_json::from_str::<Config>(&text) {
            Ok(mut cfg) => {
                // 旧版本配置没有 device_name，或被清空过：用默认值补上并落盘
                if cfg.device_name.trim().is_empty() {
                    cfg.device_name = default_device_name();
                    save(&cfg);
                }
                return cfg;
            }
            Err(e) => log::warn!("config.json corrupt ({e}); regenerating"),
        }
    }
    let cfg = Config::default();
    if let Err(e) = std::fs::write(&path, serde_json::to_string_pretty(&cfg).unwrap_or_default()) {
        log::error!("failed to write config: {e:#}");
    } else {
        log::info!("config created at {}", path.display());
    }
    cfg
}

pub fn save(cfg: &Config) {
    let Ok(dir) = config_dir() else { return };
    let path = dir.join("config.json");
    if let Err(e) = std::fs::write(&path, serde_json::to_string_pretty(cfg).unwrap_or_default()) {
        log::error!("failed to save config: {e:#}");
    }
}
