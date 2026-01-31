use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const CONFIG_DIR: &str = "obsidianclidigit1024";
const CONFIG_FILE: &str = "config.json";

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub vault_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub daily_notes_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weekly_notes_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub monthly_notes_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub templates_path: Option<String>,
}

/// Returns the full path to the config file.
/// Uses dirs::config_dir() -> ~/.config on Linux, config dir on other platforms.
pub fn config_path() -> Result<PathBuf> {
    let config_dir = dirs::config_dir()
        .context("Could not determine config directory")?;
    Ok(config_dir.join(CONFIG_DIR).join(CONFIG_FILE))
}

pub fn load_config() -> Result<Config> {
    let path = config_path()?;
    let content = fs::read_to_string(&path)
        .with_context(|| format!("Config file not found: {}. Run: obsidian-cli set-vault-path /path/to/your/vault", path.display()))?;
    let config: Config = serde_json::from_str(&content)
        .context("Invalid config file format")?;
    if config.vault_path.is_empty() {
        anyhow::bail!(
            "Vault path not configured.\nRun: obsidian-cli set-vault-path /path/to/your/vault"
        );
    }
    Ok(config)
}

pub fn save_config(config: &Config) -> Result<()> {
    let path = config_path()?;
    let dir = path.parent().context("Invalid config path")?;
    fs::create_dir_all(dir)
        .with_context(|| format!("Could not create config directory: {}", dir.display()))?;
    let content = serde_json::to_string_pretty(config)?;
    fs::write(&path, content)
        .with_context(|| format!("Could not write config file: {}", path.display()))?;
    Ok(())
}
