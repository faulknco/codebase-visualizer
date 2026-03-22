# cviz — 2.5D Codebase Visualizer

## What This Is

A native Rust application that visualizes git repositories as interactive 2.5D spatial maps with real-time AI agent activity tracking. Built with wgpu + winit.

## Project State (30 commits)

Fully working prototype. Tested on ising-rs (352 files), physics-llm-research (30 files), portfolio (237 files).

## Architecture

Five async pipeline stages on a tokio background thread. Winit + wgpu own the main thread.

```
GitCollector → Embedder → LayoutEngine → Renderer
   (git2)      (TF-IDF)   (Barnes-Hut)    (wgpu)
                                  ↑
                         SocketListener ← Claude Code hook
```

Channels connect all stages. Renderer never blocks on pipeline.

## Code Map

| File | Responsibility |
|------|---------------|
| `src/main.rs` | CLI args, channel wiring, tokio runtime spawn |
| `src/app.rs` | Winit ApplicationHandler, event loop, activity state |
| `src/scene.rs` | Shared types: FileGraph, SceneGraph, SceneNode |
| `src/socket.rs` | Unix domain socket listener for agent events |
| `src/ui.rs` | Input handling, hit testing, keyboard shortcuts |
| `src/pipeline/git_collector.rs` | git2 commit walking, co-change matrix |
| `src/pipeline/embedder.rs` | TF-IDF tokenizer + embedding computation |
| `src/pipeline/layout.rs` | Barnes-Hut force-directed layout, depth/color |
| `src/render/state.rs` | wgpu pipelines (node, edge, hull, label), draw calls |
| `src/render/camera.rs` | Orthographic camera with smooth interpolation |
| `src/render/node.rs` | Node instance data (center, radius, color, glow, activity) |
| `src/render/edge.rs` | Edge vertex data |
| `src/render/hull.rs` | Convex hull computation for directory grouping |
| `src/render/label.rs` | Embedded bitmap font, label quad generation |
| `assets/shaders/node.wgsl` | Circle + glow + activity ring shader |
| `assets/shaders/edge.wgsl` | Line shader with opacity |
| `assets/shaders/hull.wgsl` | Flat-color hull background shader |
| `assets/shaders/label.wgsl` | Textured quad font shader |
| `hooks/cviz-hook.sh` | Claude Code PostToolUse hook |

## Key Design Decisions

- **Barnes-Hut** over brute-force repulsion: O(n log n) allows 500 iterations for proper cluster separation
- **TF-IDF** over neural embeddings: zero external dependencies for v1. Ollama integration planned.
- **Unix socket** for agent events: language-agnostic, works with any tool that can write JSON
- **Bitmap font** over font rendering libraries: zero dependencies, 760 bytes embedded
- **Convex hulls** for directory grouping: Graham scan, expanded by 1.5 units, alpha 0.04

## Environment

- Rust 1.94+, managed by rustup
- Dependencies in Cargo.toml: wgpu 28, winit 0.30, git2 0.20, tokio, glam, clap, serde, serde_json, bytemuck, bincode, pollster, env_logger
- Dev dependency: tempfile (for tests)
- Tests: `cargo test` (3 integration tests: git_collector, embedder, layout)
- Release build: `cargo build --release` (~30s on M4)

## Agent Activity Socket

- Path: `/tmp/cviz-{hash}.sock` (hash of canonicalized repo path)
- Protocol: newline-delimited JSON
- Event format: `{"file":"src/main.rs","action":"edit","agent":"claude","timestamp":1711000000}`
- Claude Code hook: `hooks/cviz-hook.sh` (reads from stdin per Claude Code hook API)
