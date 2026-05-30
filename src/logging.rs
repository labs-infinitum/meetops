use tracing_subscriber::{EnvFilter, fmt};

use crate::config::LoggingConfig;

pub fn init(logging: &LoggingConfig) {
    let filter = EnvFilter::new(resolve_filter(logging));

    fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_thread_ids(false)
        .with_file(logging.log_file)
        .with_line_number(logging.log_line)
        .init();
}

fn resolve_filter(logging: &LoggingConfig) -> String {
    let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| logging.filter.clone());

    if filter.contains("chromiumoxide::handler") {
        filter
    } else {
        format!("{filter},chromiumoxide::handler=error")
    }
}
