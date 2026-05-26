# Kish · Turkish Draughts Studio

A native Rust desktop interface for testing your `kish`-based Turkish Draughts engine.

## Included features

- Human vs engine: choose White or Black.
- Two-player local mode.
- Engine vs engine watching mode.
- Flip board.
- New game and undo turn.
- Click-to-move board with legal source/destination highlights.
- Handles ambiguous multi-capture destinations through a route chooser.
- Background engine search: the UI remains responsive while thinking.
- Live analysis: depth, nodes, qnodes, NPS, TT hits, cutoffs, time and principal variation.
- Evaluation bar from White's perspective.
- Uses `kish::Game` for the real match history/draw detection and `kish::Board` for fast search.

## Install in your existing project

Your current folder is:

```bat
D:\Yazan Design\2026\kish_engine
```

1. Back up your current engine file:

```bat
cd /d "D:\Yazan Design\2026\kish_engine"
copy src\main.rs src\main_engine_backup.rs
```

2. Copy the three Rust files from this package into your project's `src` folder:

```text
src\main.rs
src\app.rs
src\engine.rs
```

3. Replace your `Cargo.toml` with the provided one, or add this dependency to your current `[dependencies]` section:

```toml
eframe = { version = "0.34.2", default-features = false, features = ["default_fonts", "glow"] }
```

4. Build and run in release mode:

```bat
cargo run --release
```

The first eframe build downloads and compiles the UI dependencies, so it will take longer than the small console engine build. Later builds are much faster.

## Engine settings

- **Move time**: thinking time for the engine when it must play.
- **Maximum depth**: hard safety cap for iterative deepening.
- **Live analysis**: when it is your turn, the engine analyzes without moving.
- **Analysis time**: time used for live analysis.

Suggested initial settings:

```text
Move time: 3 seconds
Maximum depth: 14
Live analysis: enabled
Analysis time: 2 seconds
```

## Important meaning of evaluation

The evaluation bar is the engine's calculated estimate from the current search, displayed from White's perspective:

- Positive value: White is better.
- Negative value: Black is better.
- `+1.00` is approximately one ordinary piece of evaluation in the current scoring model.

It is not a proven tablebase result unless a winning terminal line is reached.
