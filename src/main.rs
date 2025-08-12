use anyhow::Result;
use oxidized::{Editor, EventDrivenEditor};
use std::env;
use std::path::PathBuf;
use std::sync::OnceLock;

fn init_logging() {
    use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

    // Determine default level: debug in debug builds, info otherwise
    let default_level = if cfg!(debug_assertions) {
        "debug"
    } else {
        "info"
    };
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_level));

    // Always log to oxidized.log (simple, tail-friendly)
    let path = "oxidized.log";
    let file = match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        Ok(f) => f,
        Err(e) => {
            // If file cannot be opened, fall back to stderr so we still get logs
            eprintln!(
                "Failed to open log file '{}': {} — logging to stderr",
                path, e
            );
            tracing_subscriber::registry()
                .with(env_filter)
                .with(fmt::layer().with_target(true).with_ansi(true))
                .init();
            return;
        }
    };
    static LOG_GUARD: OnceLock<tracing_appender::non_blocking::WorkerGuard> = OnceLock::new();
    let (non_blocking, guard) = tracing_appender::non_blocking(file);
    let _ = LOG_GUARD.set(guard);

    let file_layer = fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_target(true);

    tracing_subscriber::registry()
        .with(env_filter)
        .with(file_layer)
        .init();
}

fn main() -> Result<()> {
    init_logging();

    tracing::info!("=== Oxidized Text Editor Starting ===");
    tracing::debug!(
        "Build type: {}",
        if cfg!(debug_assertions) {
            "DEBUG"
        } else {
            "RELEASE"
        }
    );
    tracing::info!("Version: {}", env!("CARGO_PKG_VERSION"));
    tracing::debug!(
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
