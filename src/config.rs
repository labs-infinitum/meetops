use std::{net::IpAddr, path::PathBuf};

use figment::{
    Figment,
    providers::{Env, Format, Serialized, Toml},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub server: ServerConfig,
    pub storage: StorageConfig,
    pub google: GoogleConfig,
    pub bot: BotConfig,
    pub groq: GroqConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    pub host: IpAddr,
    pub port: u16,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StorageConfig {
    pub data_dir: PathBuf,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GoogleConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub calendar_sync_interval_secs: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BotConfig {
    pub display_name: String,
    pub chromium_path: PathBuf,
    pub chrome_user_data_dir: PathBuf,
    #[serde(default)]
    /// Virtual / headless X server (xdotool target + Chrome DISPLAY), e.g. `":1"` for Xorg display `:1`.
    pub x11_display: Option<String>,
    #[serde(default)]
    /// Small PNG crop of the blue Chrome tab-share Allow pill (`allow_button_detect` grayscale fallback when chroma misses).
    /// Env: `GMR_BOT__ALLOW_BUTTON_TEMPLATE_PNG`
    pub allow_button_template_png: Option<PathBuf>,
    pub join_lead_time_secs: u64,
    pub admission_timeout_secs: u64,
    /// How often the scheduler checks for meetings to auto-join.
    pub scheduler_poll_interval_secs: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GroqConfig {
    pub api_key_env: String,
    pub transcription_model: String,
    pub summarization_model: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                host: "127.0.0.1".parse().expect("static IP address is valid"),
                port: 8080,
            },
            storage: StorageConfig {
                data_dir: PathBuf::from("./data"),
            },
            google: GoogleConfig {
                client_id: String::new(),
                client_secret: String::new(),
                redirect_uri: "http://localhost:8080/auth/google/callback".to_owned(),
                calendar_sync_interval_secs: 300,
            },
            bot: BotConfig {
                display_name: "AI Notetaker".to_owned(),
                chromium_path: PathBuf::from("/usr/bin/google-chrome"),
                chrome_user_data_dir: PathBuf::from("./data/chrome-profile"),
                x11_display: None,
                allow_button_template_png: None,
                join_lead_time_secs: 60,
                admission_timeout_secs: 300,
                scheduler_poll_interval_secs: 15,
            },
            groq: GroqConfig {
                api_key_env: "GROQ_API_KEY".to_owned(),
                transcription_model: "whisper-large-v3-turbo".to_owned(),
                summarization_model: "qwen3-32b-131k".to_owned(),
            },
        }
    }
}

impl Config {
    pub fn load() -> Result<Self, figment::Error> {
        Figment::from(Serialized::defaults(Self::default()))
            .merge(Toml::file("config.toml"))
            .merge(Env::prefixed("GMR_").split("__"))
            .extract()
    }

    pub fn database_path(&self) -> PathBuf {
        self.storage.data_dir.join("meetings.db")
    }
}
