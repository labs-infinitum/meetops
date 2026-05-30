use std::{net::IpAddr, path::PathBuf};

use figment::{
    Figment,
    providers::{Env, Format, Serialized, Toml},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub server: ServerConfig,
    pub grpc: GrpcConfig,
    pub auth: AuthConfig,
    pub logging: LoggingConfig,
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
pub struct GrpcConfig {
    pub host: IpAddr,
    pub port: u16,
    /// CORS allow-origin for grpc-web (browser frontend).
    pub allow_origin: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AuthConfig {
    /// Session lifetime in seconds (default 30 days).
    pub session_ttl_secs: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LoggingConfig {
    /// tracing-subscriber filter (used when `RUST_LOG` is unset). Env: `GMR_LOGGING__FILTER`.
    #[serde(default = "default_log_filter")]
    pub filter: String,
    /// Include source file paths in log output.
    #[serde(default)]
    pub log_file: bool,
    /// Include source line numbers in log output.
    #[serde(default)]
    pub log_line: bool,
}

fn default_log_filter() -> String {
    "info,google_meeting_recorder=debug,tower_http=debug,tonic=info".to_owned()
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
    /// Where to redirect the browser after OAuth completes (React app).
    #[serde(default = "default_frontend_redirect_url")]
    pub frontend_redirect_url: String,
    pub calendar_sync_interval_secs: u64,
}

fn default_frontend_redirect_url() -> String {
    "http://localhost:5173/app/calendars".to_owned()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BotConfig {
    pub display_name: String,
    pub chromium_path: PathBuf,
    pub chrome_user_data_dir: PathBuf,
    #[serde(default)]
    /// Optional fixed X11 display (e.g. `":1"`). When unset on Linux, the bot discovers a working
    /// display or starts Xvfb automatically (see `x11_auto_virtual`).
    pub x11_display: Option<String>,
    #[serde(default = "default_true")]
    /// On Linux, probe `/tmp/.X11-unix` and start Xvfb on a free `:10+` display when needed.
    pub x11_auto_virtual: bool,
    #[serde(default = "default_true")]
    /// After resolving or starting a display, spawn `openbox` when missing (xdotool `windowactivate`).
    pub x11_start_openbox: bool,
    #[serde(default)]
    /// Small PNG crop of the blue Chrome tab-share Allow pill for grayscale template correlation when color-blob detection misses.
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

fn default_true() -> bool {
    true
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                host: "127.0.0.1".parse().expect("static IP address is valid"),
                port: 8080,
            },
            grpc: GrpcConfig {
                host: "127.0.0.1".parse().expect("static IP address is valid"),
                port: 50051,
                allow_origin: "http://localhost:5173".to_owned(),
            },
            auth: AuthConfig {
                session_ttl_secs: 60 * 60 * 24 * 30,
            },
            logging: LoggingConfig {
                filter: default_log_filter(),
                log_file: true,
                log_line: true,
            },
            storage: StorageConfig {
                data_dir: PathBuf::from("./data"),
            },
            google: GoogleConfig {
                client_id: String::new(),
                client_secret: String::new(),
                redirect_uri: "http://localhost:8080/auth/google/callback".to_owned(),
                frontend_redirect_url: default_frontend_redirect_url(),
                calendar_sync_interval_secs: 300,
            },
            bot: BotConfig {
                display_name: "AI Notetaker".to_owned(),
                chromium_path: PathBuf::from("/usr/bin/google-chrome"),
                chrome_user_data_dir: PathBuf::from("./data/chrome-profile"),
                x11_display: None,
                x11_auto_virtual: true,
                x11_start_openbox: true,
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
