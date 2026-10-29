# MidiTrainer

A Windows desktop app that plays MIDI files and teaches piano by listening to
you. Two note displays: a falling-note keyboard view and a sheet-music view.

Built with Tauri 2 (Rust backend, React + TypeScript frontend). MIT licensed.
See `AGENTS.md` for the project structure, feature plan, and development rules.

> **Built with AI.** This project is vibe-coded — designed and largely written by
> an AI coding agent under human direction, then reviewed and tested. The code is
> MIT; treat it accordingly.

## Prerequisites

- **Node.js** 20+ and npm
- **Rust** (stable) — https://rustup.rs
- **Microsoft C++ Build Tools** (MSVC) and the WebView2 runtime

## Development

```sh
npm install
npm run tauri dev
```

## Build

```sh
npm run tauri build
```

On Windows this produces an MSI and an NSIS installer under
`src-tauri/target/release/bundle/`. Releases are **unsigned**, so Windows may show
a SmartScreen "unknown publisher" prompt on first install.

## Quality checks

```sh
npm run lint          # ESLint
npm run format:check  # Prettier
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
```

## Credits

- App icon: [Piano icons created by Assia Benkerroum - Flaticon](https://www.flaticon.com/free-icons/piano)
- Default sound: microGM by "michael" ([musical-artifacts.com/artifacts/2529](https://musical-artifacts.com/artifacts/2529), WTFPL). The full Fluid (R3) bank by Frank Wen (MIT) is an optional download.
- See `NOTICE` for all third-party notices.
