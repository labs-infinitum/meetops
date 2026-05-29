use std::{net::SocketAddr, sync::Arc};

use tokio::net::TcpListener;
use tracing_subscriber::{EnvFilter, fmt};

mod bot;
mod config;
mod db;
mod error;
mod google;
mod meet_url;
mod web;
mod workers;

use config::Config;
use web::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    load_dev_env();
    init_tracing();

    let config = Arc::new(Config::load()?);
    if let Some(command) = CliCommand::parse(std::env::args().skip(1))? {
        return match command {
            CliCommand::Join { meet_url } => bot::runner::join(meet_url, &config).await,
            CliCommand::Record { meet_url } => bot::runner::record(meet_url, &config).await,
            CliCommand::Profile => bot::runner::open_profile(&config).await,
            CliCommand::ProfileStatus => bot::runner::profile_status(&config),
        };
    }

    tokio::fs::create_dir_all(&config.storage.data_dir).await?;

    let pool = db::connect(&config).await?;
    db::migrate(&pool).await?;
    let http = reqwest::Client::new();

    workers::calendar_sync::spawn(Arc::clone(&config), pool.clone(), http.clone());

    let chrome_sessions = Arc::new(tokio::sync::Semaphore::new(1));
    workers::scheduler::spawn(
        Arc::clone(&config),
        pool.clone(),
        Arc::clone(&chrome_sessions),
    );

    let addr = SocketAddr::from((config.server.host, config.server.port));
    let state = AppState {
        config,
        db: pool,
        http,
    };
    let app = web::router(state);
    let listener = TcpListener::bind(addr).await?;

    tracing::info!(%addr, "server listening");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

enum CliCommand {
    Join { meet_url: String },
    Record { meet_url: String },
    Profile,
    ProfileStatus,
}

impl CliCommand {
    fn parse(mut args: impl Iterator<Item = String>) -> anyhow::Result<Option<Self>> {
        let Some(command) = args.next() else {
            return Ok(None);
        };

        match command.as_str() {
            "join" => {
                let Some(meet_url) = args.next() else {
                    anyhow::bail!("usage: cargo run -- join <meet_url>");
                };
                if args.next().is_some() {
                    anyhow::bail!("usage: cargo run -- join <meet_url>");
                }
                Ok(Some(Self::Join { meet_url }))
            }
            "record" => {
                let Some(meet_url) = args.next() else {
                    anyhow::bail!("usage: cargo run -- record <meet_url>");
                };
                if args.next().is_some() {
                    anyhow::bail!("usage: cargo run -- record <meet_url>");
                }
                Ok(Some(Self::Record { meet_url }))
            }
            "profile" => {
                if args.next().is_some() {
                    anyhow::bail!("usage: cargo run -- profile");
                }
                Ok(Some(Self::Profile))
            }
            "profile-status" => {
                if args.next().is_some() {
                    anyhow::bail!("usage: cargo run -- profile-status");
                }
                Ok(Some(Self::ProfileStatus))
            }
            "-h" | "--help" => {
                println!(
                    "usage:\n  google-meeting-recorder\n  google-meeting-recorder profile\n  google-meeting-recorder profile-status\n  google-meeting-recorder join <meet_url>\n  google-meeting-recorder record <meet_url>"
                );
                std::process::exit(0);
            }
            other => anyhow::bail!("unknown command '{other}'. Try --help."),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::CliCommand;

    #[test]
    fn parses_join_command() {
        let command = CliCommand::parse(
            ["join", "https://meet.google.com/abc-defg-hij"]
                .into_iter()
                .map(str::to_owned),
        )
        .expect("join command should parse");

        match command {
            Some(CliCommand::Join { meet_url }) => {
                assert_eq!(meet_url, "https://meet.google.com/abc-defg-hij");
            }
            _ => panic!("expected join command"),
        }
    }

    #[test]
    fn rejects_extra_join_args() {
        let result = CliCommand::parse(
            ["join", "https://meet.google.com/abc-defg-hij", "extra"]
                .into_iter()
                .map(str::to_owned),
        );

        assert!(result.is_err());
    }

    #[test]
    fn parses_profile_command() {
        let command = CliCommand::parse(["profile"].into_iter().map(str::to_owned))
            .expect("profile command should parse");

        assert!(matches!(command, Some(CliCommand::Profile)));
    }

    #[test]
    fn parses_profile_status_command() {
        let command = CliCommand::parse(["profile-status"].into_iter().map(str::to_owned))
            .expect("profile-status command should parse");

        assert!(matches!(command, Some(CliCommand::ProfileStatus)));
    }

    #[test]
    fn parses_record_command() {
        let command = CliCommand::parse(
            ["record", "https://meet.google.com/abc-defg-hij"]
                .into_iter()
                .map(str::to_owned),
        )
        .expect("record command should parse");

        match command {
            Some(CliCommand::Record { meet_url }) => {
                assert_eq!(meet_url, "https://meet.google.com/abc-defg-hij");
            }
            _ => panic!("expected record command"),
        }
    }
}

fn load_dev_env() {
    let _ = dotenvy::dotenv();
}

fn init_tracing() {
    let filter = EnvFilter::new(log_filter());

    fmt().with_env_filter(filter).init();
}

fn log_filter() -> String {
    let filter = std::env::var("RUST_LOG")
        .unwrap_or_else(|_| "info,google_meeting_recorder=debug,tower_http=info".to_owned());

    if filter.contains("chromiumoxide::handler") {
        filter
    } else {
        format!("{filter},chromiumoxide::handler=error")
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        match tokio::signal::ctrl_c().await {
            Ok(()) => {}
            Err(error) => {
                tracing::error!(
                    error = %error,
                    "failed to install Ctrl+C handler; graceful shutdown requires SIGTERM"
                );
                std::future::pending::<()>().await
            }
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(error) => {
                tracing::error!(
                    error = %error,
                    "failed to install SIGTERM handler; shutting down requires Ctrl+C"
                );
                std::future::pending::<()>().await
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
