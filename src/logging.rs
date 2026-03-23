use std::backtrace::Backtrace;
use std::fs::{self, OpenOptions};
use std::path::PathBuf;

use anyhow::Result;
use tracing::{error, info};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

pub struct LoggingGuard {
    _guard: WorkerGuard,
    pub log_path: PathBuf,
}

pub fn init() -> Result<LoggingGuard> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let log_dir = root.join("logs");
    fs::create_dir_all(&log_dir)?;
    let log_path = log_dir.join("riggy.log");

    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;
    let (writer, guard) = tracing_appender::non_blocking(log_file);

    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("riggy=trace,rig=debug,info"));

    tracing_subscriber::registry()
        .with(env_filter)
        .with(
            fmt::layer()
                .with_ansi(false)
                .with_writer(writer)
                .with_file(true)
                .with_line_number(true)
                .with_target(true)
                .with_thread_ids(true)
                .with_thread_names(true)
                .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE),
        )
        .try_init()?;

    install_panic_hook();
    info!(log_path = %log_path.display(), "logging initialized");

    Ok(LoggingGuard {
        _guard: guard,
        log_path,
    })
}

fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let location = panic_info
            .location()
            .map(|location| {
                format!(
                    "{}:{}:{}",
                    location.file(),
                    location.line(),
                    location.column()
                )
            })
            .unwrap_or_else(|| "<unknown>".to_string());

        let payload = if let Some(message) = panic_info.payload().downcast_ref::<&str>() {
            (*message).to_string()
        } else if let Some(message) = panic_info.payload().downcast_ref::<String>() {
            message.clone()
        } else {
            "non-string panic payload".to_string()
        };

        error!(
            %location,
            %payload,
            backtrace = %Backtrace::force_capture(),
            "application panicked"
        );

        default_hook(panic_info);
    }));
}
