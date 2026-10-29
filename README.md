# MidiTrainer

A cross-platform desktop app that plays MIDI files and teaches piano by listening
to you. Two note displays: a falling-note keyboard view and a sheet-music view.

Built with Tauri 2 (Rust backend, React + TypeScript frontend). MIT licensed.
See `AGENTS.md` for the project structure, feature plan, and development rules.

## Prerequisites

- **Node.js** 20+ and npm
- **Rust** (stable) — https://rustup.rs
- **Windows**: Microsoft C++ Build Tools (MSVC) and the WebView2 runtime
- **macOS**: Xcode command line tools
- **Linux**: WebKitGTK and ALSA development packages (see
  https://tauri.app/start/prerequisites/)

## Development

```sh
npm install
npm run tauri dev
```

## Build

```sh
npm run tauri build
```

## Quality checks

```sh
npm run lint          # ESLint
npm run format:check  # Prettier
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
```
