use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use crate::domain::QariId;

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub home: PathBuf,
    pub config_path: PathBuf,
    pub db_path: PathBuf,
    pub notice_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncConfig {
    pub enabled: bool,
    pub interval_hours: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConfig {
    pub default_qari: String,
    pub cache_enabled: bool,
    pub cache_max_mb: u64,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            default_qari: "05".to_string(),
            cache_enabled: true,
            cache_max_mb: 512,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    pub theme: String,
    pub output: String,
    pub keymap: String,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            theme: "auto".to_string(),
            output: "rich".to_string(),
            keymap: "modern".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub default_lang: String,
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_show_translation")]
    pub show_translation: bool,
    pub sync: SyncConfig,
    #[serde(default)]
    pub audio: AudioConfig,
    #[serde(default)]
    pub ui: UiConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            default_lang: "id".to_string(),
            theme: default_theme(),
            show_translation: default_show_translation(),
            sync: SyncConfig {
                enabled: true,
                interval_hours: 24,
            },
            audio: AudioConfig::default(),
            ui: UiConfig::default(),
        }
    }
}

impl AppConfig {
    pub fn set_key(&mut self, key: &str, value: &str) -> Result<()> {
        match key {
            "default_lang" => {
                self.default_lang = value.to_string();
            }
            "theme" => {
                self.theme = value.to_string();
                self.ui.theme = value.to_string();
            }
            "ui.theme" => {
                self.ui.theme = value.to_string();
            }
            "show_translation" => {
                self.show_translation = parse_bool(value)?;
            }
            "ui.output" => {
                self.ui.output = parse_output(value)?;
            }
            "ui.keymap" => {
                self.ui.keymap = parse_keymap(value)?;
            }
            "sync.enabled" => {
                self.sync.enabled = parse_bool(value)?;
            }
            "sync.interval_hours" => {
                self.sync.interval_hours = value
                    .parse::<u64>()
                    .context("sync.interval_hours harus angka positif")?;
            }
            "audio.default_qari" => {
                self.audio.default_qari = parse_qari(value)?;
            }
            "audio.cache_enabled" => {
                self.audio.cache_enabled = parse_bool(value)?;
            }
            "audio.cache_max_mb" => {
                self.audio.cache_max_mb = value
                    .parse::<u64>()
                    .context("audio.cache_max_mb harus angka positif")?;
            }
            _ => return Err(anyhow!("Key config tidak dikenal: {key}")),
        }
        Ok(())
    }

    pub fn ui_theme(&self) -> &str {
        if self.ui.theme.trim().is_empty() {
            &self.theme
        } else {
            &self.ui.theme
        }
    }

    pub fn ui_output(&self) -> &str {
        if self.ui.output.trim().is_empty() {
            "rich"
        } else {
            &self.ui.output
        }
    }
}

pub fn resolve_paths(data_dir: Option<&Path>) -> Result<AppPaths> {
    let home = if let Some(path) = data_dir {
        path.to_path_buf()
    } else if let Ok(from_env) = std::env::var("TADA_HOME") {
        PathBuf::from(from_env)
    } else {
        let dirs = ProjectDirs::from("dev", "tada", "tada-rust")
            .ok_or_else(|| anyhow!("Gagal menentukan direktori aplikasi"))?;
        dirs.data_local_dir().to_path_buf()
    };

    fs::create_dir_all(&home).context("Gagal membuat direktori data")?;

    Ok(AppPaths {
        config_path: home.join("config.toml"),
        db_path: home.join("tada.db"),
        notice_path: home.join("NOTICE"),
        home,
    })
}

pub fn load_or_create(paths: &AppPaths) -> Result<AppConfig> {
    if paths.config_path.exists() {
        let raw = fs::read_to_string(&paths.config_path)
            .with_context(|| format!("Gagal membaca {}", paths.config_path.display()))?;
        let mut cfg = toml::from_str::<AppConfig>(&raw)
            .with_context(|| format!("Gagal parse {}", paths.config_path.display()))?;
        if cfg.ui.theme.trim().is_empty() {
            cfg.ui.theme = cfg.theme.clone();
        }
        if cfg.ui.output.trim().is_empty() {
            cfg.ui.output = "rich".to_string();
        }
        if cfg.ui.keymap.trim().is_empty() {
            cfg.ui.keymap = "modern".to_string();
        }
        if cfg.audio.default_qari.trim().is_empty() {
            cfg.audio.default_qari = "05".to_string();
        }
        if cfg.audio.cache_max_mb == 0 {
            cfg.audio.cache_max_mb = 512;
        }
        Ok(cfg)
    } else {
        let cfg = AppConfig::default();
        save(paths, &cfg)?;
        Ok(cfg)
    }
}

pub fn save(paths: &AppPaths, config: &AppConfig) -> Result<()> {
    let raw = toml::to_string_pretty(config).context("Gagal serialize config TOML")?;
    fs::write(&paths.config_path, raw)
        .with_context(|| format!("Gagal menulis {}", paths.config_path.display()))?;
    Ok(())
}

pub fn ensure_notice(paths: &AppPaths) -> Result<()> {
    if !paths.notice_path.exists() {
        fs::write(&paths.notice_path, attribution_notice()).with_context(|| {
            format!(
                "Gagal menulis attribution notice di {}",
                paths.notice_path.display()
            )
        })?;
    }
    Ok(())
}

fn attribution_notice() -> &'static str {
    "Quran content is fetched live from equran.id API v2. Ensure API provider terms and attribution requirements are followed before public release."
}

fn parse_bool(value: &str) -> Result<bool> {
    match value {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        _ => Err(anyhow!(
            "Nilai boolean tidak valid: {value}. Gunakan true/false."
        )),
    }
}

fn parse_output(value: &str) -> Result<String> {
    match value {
        "rich" | "plain" | "json" => Ok(value.to_string()),
        _ => Err(anyhow!(
            "Nilai ui.output tidak valid: {value}. Gunakan rich/plain/json."
        )),
    }
}

fn parse_keymap(value: &str) -> Result<String> {
    match value {
        "modern" => Ok(value.to_string()),
        _ => Err(anyhow!(
            "Nilai ui.keymap tidak valid: {value}. Saat ini hanya modern didukung."
        )),
    }
}

fn parse_qari(value: &str) -> Result<String> {
    QariId::new(value)
        .map(|qari| qari.as_str().to_string())
        .map_err(|_| anyhow!("Nilai audio.default_qari tidak valid: {value}. Gunakan 01..06."))
}

fn default_theme() -> String {
    "auto".to_string()
}

fn default_show_translation() -> bool {
    true
}
