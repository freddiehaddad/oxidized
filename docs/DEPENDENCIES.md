# Dependencies

## Core

- crossterm — terminal manipulation and events (cross‑platform)
- toml — configuration parsing/serialization
- serde — serialization framework
- anyhow — ergonomic error handling
- regex — regular expressions for search

## Advanced features

- tree-sitter — AST parsing for syntax highlighting
- tree-sitter-rust — Rust grammar for Tree-sitter
- notify — filesystem monitoring for hot reload
- tokio — async runtime for background tasks
- unicode-width / unicode-segmentation — Unicode handling

## Development

- log + tracing-subscriber/appender — logging (file by default in TTY, stderr
  fallback)
- criterion — benchmarking framework
