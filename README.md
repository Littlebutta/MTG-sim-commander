# MTG Commander Simulator

A desktop Commander game for playing with friends using your own decks, with Steam lobbies and invites.

The project is split into layers so the rules can be built and tested before any UI exists:

| Crate / folder | What it is |
|---|---|
| `crates/rules-core` | The rules engine. Implements the Comprehensive Rules (Nov 6, 2026) for Commander: turn structure, priority, the stack, state-based actions, triggered abilities, combat, and the continuous-effect layer system (CR 613). No UI or networking code. |
| `crates/carddb` | Card data: reads MTGJSON `AtomicCards`, turns oracle text into card scripts, imports and validates Commander decklists. |
| `crates/client-api` | Per-player views with hidden information removed, legal-action hints, practice opponents, starter decks. |
| `crates/engine-wasm` | The engine compiled to WebAssembly for the UI. |
| `app/` | Tauri 2 + React desktop client, shipped as a portable folder that updates itself from this repo's GitHub Releases. Steam lobbies/invites are next. |

## How the engine works

The host owns a `Game`. `game.decision()` says who must act (priority, attackers, blockers, trigger order, discard…), and `game.submit(player, action)` answers it. Everything the rules do on their own (turn-based actions, state-based actions, putting triggers on the stack, resolution) happens automatically in between. The game is deterministic from its seed, so a networked game is just the host's engine plus a stream of player answers.

Characteristics are never stored mutated: every query recomputes them from printed values plus continuous effects, layer by layer, with CDAs first, dependency ordering (613.8) and timestamps (613.7). See `crates/rules-core/src/layers.rs`.

## Card support

MTGJSON gives every card's oracle text, but no tool can turn all of it into working code automatically. `carddb::oracle` handles common templated wording; anything else is marked `Unsupported` and shows up in the deck import report so it can be scripted (or played by hand) instead of silently misbehaving.

## Development

```sh
cargo test --workspace

# desktop client
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.130
cd app
npm ci
npm run wasm        # build the engine for the UI
npm run dev         # play in a browser at http://localhost:1420
npx tauri dev       # or run the desktop window
```

## Downloading the game

Grab `CommanderTable-v…-windows-portable.zip` from the [latest release](https://github.com/Littlebutta/MTG-sim-commander/releases/latest), unzip it anywhere and run `Commander Table.exe`. It checks for updates on launch.

See [docs/STATUS.md](docs/STATUS.md) for what's implemented and what's next.
