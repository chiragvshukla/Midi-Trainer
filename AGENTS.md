# AGENTS.md

Instructions for coding agents working in this repository. Read this before changing anything.

## What this project is

MidiTrainer is a cross-platform desktop app that plays MIDI files and teaches piano by listening to the player. It is a from-scratch reimplementation of the _concept_ of PianoBooster (https://github.com/pianobooster/PianoBooster), with a modern UI. Two note displays: a falling-note keyboard view and a sheet-music view.

Licensing is strict: **clean-room**. PianoBooster is GPLv3; this project is MIT. Never copy, paste, translate, or port code from PianoBooster or any other GPL source. Reimplement behavior from observation and documentation only. If any external code is ever used, its license must be MIT-compatible and stated in the commit.

## Plan tracking

The plan file at `C:\Users\Chirag\.opencode\plan\plan.md` is the live status of the work: read it before starting to see what is done and what still needs completing.

- Update it as part of finishing any work: mark completed items done and record new features or bugs addressed, so it always matches reality.
- If a change is not reflected in the plan (a new feature, a fixed bug, a scope shift), add or amend the relevant entry in the same change.

## What exists today

Step 2 (scaffold) is done and the app runs (`npm run tauri dev`). Present today: the root meta files (`README.md`, `LICENSE`, `package.json`, `eslint.config.js`, `index.html`), `src/App.tsx` with `src/lib/` (logger, IPC bindings) and `src/state/appStore.ts`, and `src-tauri/src/commands/` (`app_info.rs`, `log_message.rs`).

The next section is the full target tree: entries not listed above are still _planned_. Do not assume anything else exists — check the tree before referencing paths, and never overwrite the files listed above.

## Planned structure

Standard Tauri 2 layout. Rust lives in `src-tauri/`, the UI in `src/`.

```
MidiTrainer/
├── AGENTS.md               # source of truth for agents (exists)
├── README.md               # user-facing docs, build/run instructions
├── LICENSE                 # MIT
├── package.json            # frontend tooling (Vite, React, TS)
├── src/                    # TypeScript + React UI
│   ├── components/         #   chrome: toolbar, panels, dialogs, settings
│   ├── piano-roll/         #   falling-note display + on-screen keyboard
│   ├── score/              #   sheet-music display (VexFlow), cursor, highlights
│   ├── state/              #   Zustand stores (transport, settings, score state)
│   └── lib/                #   Tauri IPC bindings, shared types, logger
├── src-tauri/              # Rust backend
│   ├── src/
│   │   ├── commands/       #   one Tauri command per file
│   │   ├── engine/         #   MIDI parsing, timeline/scheduler, quantizer, scoring
│   │   ├── audio/          #   SoundFont synth, event queue, mixer
│   │   └── midi/           #   MIDI input (midir), PC-keyboard fallback
│   └── Cargo.toml
├── test_data/              # small MIDI fixtures used by tests
└── .github/workflows/      # CI: fmt, clippy, tests, build all three platforms
```

## Languages and stack

| Layer    | Language   | Key pieces                                                                         |
| -------- | ---------- | ---------------------------------------------------------------------------------- |
| Backend  | Rust       | Tauri 2 shell, MIDI parse/scheduling, scoring, audio synthesis                     |
| Frontend | TypeScript | React, Vite, Zustand, Canvas (piano roll), VexFlow (notation)                      |
| MIDI I/O | Rust       | midir for hardware MIDI input                                                      |
| Audio    | Rust       | SoundFont (`.sf2`) synthesizer; bundled baseline, optional full bank, or user file |
| Tests    | Rust + TS  | Rust unit tests for engine/scoring; Vitest for UI logic                            |

- Indentation: 2 spaces (TS), rustfmt defaults (Rust).
- New dependencies require a one-line justification in the PR/commit message. Prefer the standard library, Tauri's own APIs, or what is already in the tree. No dependency for something a few lines of code handle.
- Both note displays are thin renderers over **one** shared timeline/score-event stream produced by the Rust engine. There is exactly one source of truth for playback position, note status, and scoring — never duplicate scoring logic in a view.

## Feature plan

All features that need to be built, in order. Do not build later-phase features before earlier ones without being asked.

### Phase 1 — MVP (falling-note view)

- Load `.mid` and `.kar` files (karaoke MIDI with lyrics), parse tracks, select tracks.
- Falling-note display: notes scroll toward an on-screen keyboard, keys light up when played, right/wrong coloring.
- MIDI input from a hardware piano keyboard; PC-keyboard fallback (`x` = middle C).
- Follow-you mode: the accompaniment follows the player's tempo; if the player stops, the music stops. (Core behavior, not a setting bolted on later.)
- Listen mode: playback with no MIDI input required.
- Live tempo scaling (percent) and transpose ±12 semitones, changeable during playback.
- Playing-accuracy measurement with timing markers (ahead of / behind the beat).
- Distinct sounds for correct vs. wrong notes played.
- SoundFont synth: a small bundled baseline (microGM, WTFPL / permissive) plays out of the box; a larger FluidR3 bank is an optional download, and the user may choose their own `.sf2`.
- Settings persistence: recent files, MIDI device choice, volume, tempo, transpose.

### Phase 2 — sheet-music view and feature parity

- Engraving pipeline in the engine: rhythmic quantization and voice splitting (MIDI events → notation model). This is the hardest part of the project — keep v1 scoped: common meters (4/4, 3/4, 6/8), quarters through sixteenths, chords, up to two voices per staff.
- Sheet-music display: grand staff (treble + bass) rendered with VexFlow from the notation model.
- Playback cursor sweeping the score plus per-note highlighting with the same right/wrong feedback as the piano roll.
- Live toggle between falling-note and sheet-music views at any time during playback, with zoom/scroll remembered per view.
- One-hand practice: the player takes one hand, the app plays the other in time with them.
- Play-along on any track: the player can take over any instrument part in the file.
- Per-track and current-part volume mixing.
- Karaoke lyrics display (from `.kar` lyric events).
- Skill levels (e.g. listen / follow-you / strict timing).

### Phase 3 — polish

- Bundled course content (original material, or third-party CC-BY music with visible attribution).
- Session history and practice statistics.
- Theming / dark mode.
- Translations (i18n).
- Installers and auto-update via Tauri bundler.

Explicitly out of scope: MusicXML import (users export MIDI from their notation app), optical recognition of PDFs/images, and any networked/multiplayer features.

## Rules

These are hard requirements, not style preferences.

**Output and logging**

- Never `println!`, `print!`, `eprintln!`, `eprint!` in Rust outside of tests — use the `log` crate. Never `console.log`/`console.error` in TypeScript outside of tests — use the frontend logger in `src/lib/`.
- No debug output left in committed code.

**Secrets and safety**

- Never read, write, modify, or print anything from `secrets/`, `.env*`, signing keys, tokens, or credential stores. If a task seems to need them, stop and ask.
- No secrets, tokens, or personal data in code, logs, error messages, or test fixtures.
- No network access except explicitly user-initiated actions (e.g. downloading a soundfont or checking for updates). Never a silent background call. Never telemetry.

**Code**

- No code copied from PianoBooster or other GPL sources (see clean-room rule above).
- Error handling at trust boundaries is never skipped: MIDI file parsing, MusicXML-free input, and MIDI device input are all untrusted. Malformed MIDI must fail with a clear error, not crash the app.
- Real-time paths: the audio callback and MIDI input threads must not block, do file I/O, take contended locks, or allocate without bound. Queue events across threads; never do work inline.
- No stubs, placeholders, TODO-only modules, or "for later" abstractions in delivered work. If it is in the tree it works.
- Bug fixes go to the root cause in shared code, not to each caller.
- Keep both view implementations logic-free: parsing, scheduling, and scoring belong to the engine.

**Files and hygiene**

- Do not commit build artifacts: `target/`, `node_modules/`, `dist/`, installers, generated icons.
- Do not delete or rewrite user data, local settings, or untracked files without being asked.
- Format and lint are law: `cargo fmt`, `cargo clippy` (no warnings), Prettier and ESLint clean for TS.

**File layout (Rust and TS)**

Organize for top-to-bottom reading:

1. Imports
2. Constants
3. Types (public before private; request before response before error)
4. Public API: constructors, then public methods
5. Private helpers below the code that calls them
6. Tests last (constants, cases, helpers)

## Testing expectations

- New non-trivial engine logic (quantizer, scheduler, scoring, MIDI parsing) ships with unit tests using fixtures in `test_data/`.
- UI components need tests only where they hold logic; rendering smoke tests are not required.
- A change that breaks an existing test fixes the test in the same change, with a reason.
