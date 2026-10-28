use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct Config {
    pub ndi: NdiConfig,
    pub mute: MuteConfig,
    pub log: LogConfig,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct NdiConfig {
    pub source_name: String,
    pub output_name: String,
    pub output_groups: Option<String>,
    pub bandwidth: Bandwidth,
    pub color_format: ColorFormat,
    pub clock_video: bool,
    pub clock_audio: bool,
    pub capture_timeout_ms: u64,
    pub reconnect_delay_ms: u64,
    pub status_interval_ms: u64,
}

impl Default for NdiConfig {
    fn default() -> Self {
        Self {
            source_name: "OBS".to_string(),
            output_name: "OTOME-OUT".to_string(),
            output_groups: Some("Public".to_string()),
            bandwidth: Bandwidth::Highest,
            color_format: ColorFormat::UyvyBgra,
            clock_video: true,
            clock_audio: false,
            capture_timeout_ms: 500,
            reconnect_delay_ms: 1000,
            status_interval_ms: 5000,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Bandwidth {
    #[default]
    Highest,
    Lowest,
    AudioOnly,
    MetadataOnly,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ColorFormat {
    BgrxBgra,
    #[default]
    UyvyBgra,
    RgbxRgba,
    UyvyRgba,
    Fastest,
    Best,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct MuteConfig {
    pub mode: MuteMode,
    pub listen: String,
    pub remote_url: String,
    pub forward_to_remote: bool,
    pub poll_interval_ms: u64,
    pub audio_mode: AudioMode,
}

impl Default for MuteConfig {
    fn default() -> Self {
        Self {
            mode: MuteMode::Api,
            listen: "0.0.0.0:7878".to_string(),
            remote_url: "http://127.0.0.1:7878".to_string(),
            forward_to_remote: false,
            poll_interval_ms: 200,
            audio_mode: AudioMode::Drop,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MuteMode {
    #[default]
    Api,
    Poll,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AudioMode {
    #[default]
    Drop,
    Silence,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct LogConfig {
    pub level: String,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
        }
    }
}

pub fn load(path: &Path) -> Result<Config> {
    if !path.exists() {
        return Ok(Config::default());
    }

    let text = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read config {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("failed to parse config {}", path.display()))
}

impl Config {
    pub fn validate(&self) -> Result<()> {
        if self.ndi.source_name.trim().is_empty() {
            anyhow::bail!("ndi.source_name must not be empty");
        }
        if self.ndi.output_name.trim().is_empty() {
            anyhow::bail!("ndi.output_name must not be empty");
        }
        if self.mute.poll_interval_ms < 50 {
            anyhow::bail!("mute.poll_interval_ms must be at least 50");
        }
        if self.mute.mode == MuteMode::Api && self.mute.listen.trim().is_empty() {
            anyhow::bail!("mute.listen must not be empty");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_valid() {
        Config::default().validate().unwrap();
    }

    #[test]
    fn parses_modes_and_bandwidth() {
        let config: Config = toml::from_str(
            r#"
            [ndi]
            bandwidth = "lowest"
            color_format = "fastest"

            [mute]
            mode = "poll"
            audio_mode = "drop"
            "#,
        )
        .unwrap();

        assert_eq!(config.ndi.bandwidth, Bandwidth::Lowest);
        assert_eq!(config.ndi.color_format, ColorFormat::Fastest);
        assert_eq!(config.mute.mode, MuteMode::Poll);
        assert_eq!(config.mute.audio_mode, AudioMode::Drop);
    }
}
