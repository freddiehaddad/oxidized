# Oxidized Docs

A compact entry point to user and developer documentation.

## User guides

- Getting started: see the project [README](../README.md)
- Configuration overview: [CONFIGURATION.md](./CONFIGURATION.md)
- Keymaps: [KEYMAPS.md](./KEYMAPS.md)
- Themes and colors: [THEMES.md](./THEMES.md)
- Command-line completion: [COMPLETION.md](./COMPLETION.md)
- Development & debugging: [DEVELOPMENT.md](./DEVELOPMENT.md)
- Troubleshooting: [DEVELOPMENT.md](./DEVELOPMENT.md#troubleshooting)
- Feature status: [FEATURE_STATUS.md](./FEATURE_STATUS.md)
- Roadmap: [ROADMAP.md](./ROADMAP.md)
- Dependencies: [DEPENDENCIES.md](./DEPENDENCIES.md)
- Contributing: [CONTRIBUTING_USER.md](./CONTRIBUTING_USER.md)

## Architecture

- Full guide: [ARCHITECTURE.md](./ARCHITECTURE.md)
- At a glance: [ARCHITECTURE_QUICKSTART.md](./ARCHITECTURE_QUICKSTART.md)
- Contributing (developer focus): [CONTRIBUTING_ARCH.md](./CONTRIBUTING_ARCH.md)

Note: Architecture diagrams are written in Mermaid. Ensure your viewer
supports Mermaid rendering (GitHub, VS Code, or your site) or enable a
preview extension.

## Source of truth for defaults

The canonical default configuration files live at the repository root:

- `editor.toml` — editor settings and behavior
- `keymaps.toml` — default key bindings
- `themes.toml` — default theme colors

These files are loaded by the editor and are also used in tests (e.g., a
drift guard validates `keymaps.toml`). For examples and customization tips,
see the guides above.
