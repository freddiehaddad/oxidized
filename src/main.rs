use anyhow::Result;
use crossterm::tty::IsTty;
use oxidized::{Editor, EventDrivenEditor};
use std::env;
use std::io;
use std::path::PathBuf;
use std::sync::OnceLock;

fn init_logging() {
    use tracing_subscriber::filter::LevelFilter;
    use tracing_subscriber::prelude::*;
    use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

    // Determine default level: debug in debug builds, info otherwise
    let default_level = if cfg!(debug_assertions) {
        "debug"
    } else {
        "info"
    };
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_level));

    // Destination selection
    let dest = env::var("OXY_LOG_DEST").unwrap_or_else(|_| {
        // If stdout is a TTY (interactive editor), prefer file to avoid corrupting UI.
        if io::stdout().is_tty() {
            "file".to_string()
        } else {
            "stderr".to_string()
        }
    });

    match dest.as_str() {
        "off" => {
            // No logging subscriber; logs will be dropped.
        }
        "stderr" => {
            // Log to stderr with env-controlled filtering.
            tracing_subscriber::registry()
                .with(env_filter)
                .with(fmt::layer().with_target(true).with_ansi(true))
                .init();
        }
        // Default and "file": write to a log file, mirror warnings+ to stderr.
        _ => {
            let path = env::var("OXY_LOG_FILE").unwrap_or_else(|_| "oxidized.log".to_string());
            let file = match std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
            {
                Ok(f) => f,
                Err(e) => {
                    // Fallback to stderr if file cannot be opened
                    eprintln!(
                        "Failed to open log file '{}': {} — falling back to stderr",
                        path, e
                    );
                    tracing_subscriber::registry()
                        .with(env_filter)
                        .with(fmt::layer().with_target(true).with_ansi(true))
                        .init();
                    return;
                }
            };
            static LOG_GUARD: OnceLock<tracing_appender::non_blocking::WorkerGuard> =
                OnceLock::new();
            let (non_blocking, guard) = tracing_appender::non_blocking(file);
            let _ = LOG_GUARD.set(guard);

            // Primary layer to file
            let file_layer = fmt::layer()
                .with_writer(non_blocking)
                .with_ansi(false)
                .with_target(true);

            // Secondary layer to stderr for warnings and above
            let stderr_layer = fmt::layer()
                .with_writer(io::stderr)
                .with_ansi(true)
                .with_target(true)
                .with_filter(LevelFilter::WARN);

            tracing_subscriber::registry()
                .with(env_filter)
                .with(file_layer)
                .with(stderr_layer)
                .init();
        }
    }
}

fn main() -> Result<()> {
    init_logging();

    log::info!("=== Oxidized Text Editor Starting ===");
    log::debug!(
        "Build type: {}",
        if cfg!(debug_assertions) {
            "DEBUG"
        } else {
            "RELEASE"
        }
    );
    log::info!("Version: {}", env!("CARGO_PKG_VERSION"));
    log::debug!(
        "Working directory: {:?}",
        std::env::current_dir().unwrap_or_default()
    );

    // Create editor instance
    let mut editor = Editor::new()?;

    // Check for command line arguments (file to open)
    let args: Vec<String> = env::args().collect();
    if args.len() > 1 {
        let file_path = PathBuf::from(&args[1]);
        if let Err(e) = editor.create_buffer(Some(file_path)) {
            eprintln!("Error opening file {}: {}", args[1], e);
        }
    }

    // Create event-driven editor and run it
    let mut event_driven_editor = EventDrivenEditor::new(editor);
    if let Err(e) = event_driven_editor.run() {
        eprintln!("Editor error: {}", e);
        std::process::exit(1);
    }

    Ok(())
}
