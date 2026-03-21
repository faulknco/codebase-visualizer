# Codebase Visualizer (cviz) — Design Spec

## Goal

A native Rust application that visualizes a git repository as a 2.5D interactive map, where file positions reflect structural relationships (co-change frequency + semantic similarity), depth encodes configurable metrics, and color encodes file properties. The prototype validates whether spatial representation of codebases provides genuine insight over flat file trees.

## Context

No existing agentic coding tool provides a spatial view of codebases. Editors show file trees and chat panels. The hypothesis: codebases have high-dimensional structure (coupling, semantic similarity, change patterns, activity) that's invisible in 1D lists but obvious in 2D/3D projections — the same way PCA reveals structure in high-dimensional data.

The long-term goal is a polished local application for developers using AI coding agents. The prototype scopes to a single-repo visualizer to test the core hypothesis.

## Architecture: Async Pipeline

Single Rust binary with four async stages connected by `tokio::sync::mpsc` channels:

```
GitCollector ──→ Embedder ──→ LayoutEngine ──→ Renderer
   (git2)        (TF-IDF)     (force-directed)  (wgpu + winit)
```

Each stage owns its data and communicates through typed messages. The renderer never blocks on other stages — it re-renders the last known `SceneGraph` at vsync.

### GitCollector

- Uses `git2` crate to walk commit history (last 500 commits, configurable)
- Builds co-change matrix: score = commits where both files appear / commits touching either
- Polls `git diff HEAD` every 2 seconds for recent activity
- Collects file metadata: line count, last modified, commit frequency
- Emits `FileGraph` snapshots downstream

### Embedder

- Prototype: TF-IDF over file tokens (whitespace + camelCase + snake_case splitting). No external dependencies.
- Future (v0.2): Ollama embedding endpoint (`nomic-embed-text` or similar)
- Embeddings cached to `.cviz/embeddings.bin`, recomputed only for changed files
- Emits `EmbeddingMap`

### LayoutEngine

- Merges co-change graph + embeddings into 2D positions
- Hybrid distance: 0.6 × co-change distance + 0.4 × embedding distance
- Initial placement via UMAP-style projection, refined with continuous force-directed simulation (co-change edges as springs, embedding similarity as gentle attraction)
- Depth axis computed as separate pass after XY settles, based on selected mode:
  - **Recency**: recently modified files float higher
  - **Coupling**: files with many connections rise up
  - **Importance**: PageRank-style score from dependency/co-change graph
- Emits `SceneGraph` with positions, colors, sizes, depth values

### Renderer

- wgpu + winit event loop
- Receives `SceneGraph` updates via channel, renders at vsync
- Dark background, radial gradient nodes with glow effects
- Smooth camera interpolation for all transitions

## Visual Representation

### Node Properties

| Property | Encodes |
|----------|---------|
| Node size | Lines of code (log scale) |
| Node color | Toggleable: file type / change recency |
| Glow intensity | Change frequency (more commits = brighter) |
| Z-height (depth) | Toggleable: recency / coupling / importance |
| XY position | Force-directed layout from co-change + embeddings |
| Edge opacity | Co-change strength between file pairs |

### Color Modes

**File type (default):** Distinct hues per extension — .rs (red), .toml (amber), .md (green), tests (indigo), other (slate).

**Change recency:** Continuous gradient from cold blue (stale) through green to hot red (just changed).

### Depth Axis

The "0.5" in 2.5D. Rendered as subtle elevation with perspective projection and shadow/glow to convey height. Three toggleable modes: recency, coupling strength, importance (PageRank). User switches with keyboard shortcut `1/2/3`.

## Interaction Model

### Three Zoom Levels

**Galaxy view (default):** Entire codebase as constellation. Pan/zoom with trackpad. Clusters visible as spatial groupings. Hover for filename tooltip. Depth most visible here — terrain-like.

**Neighborhood view (zoom in):** Individual file labels readable. Edges visible. Click to select a node, highlighting connected files and dimming others.

**File view (click to inspect):** Sidebar panel slides in with: path, line count, last modified, top co-changed files, recent git activity. No code preview — not an editor.

### Controls

- Scroll/pinch: zoom between galaxy ↔ neighborhood
- Click node: select/inspect
- Right-click or Escape: deselect
- `1/2/3`: switch depth mode (recency / coupling / importance)
- `c`: cycle color mode
- `Space`: reset camera to fit-all

All state changes animate smoothly — nodes interpolate to new positions, colors fade between modes.

## Data Flow

### First Launch

Full rebuild: git walk → co-change matrix → TF-IDF embeddings → force-directed layout → render. Expected: 2-5 seconds for repos under 1000 files.

### Incremental Updates

Git poll detects changes → re-embed only changed files → layout engine nudges affected nodes (not full recompute) → renderer animates transition. Target: sub-100ms for incremental updates.

## Target Input

- Single git repository, passed as CLI argument: `cviz ~/Projects/ising-rs`
- Prototype tested against user's own repos (ising-rs, physics-llm-research, portfolio) where the spatial layout can be judged against known structure
- Stress testing later on larger open source projects (ripgrep, tokio)

## Tech Stack

- **Language:** Rust (edition 2021, rustc 1.94+)
- **Rendering:** wgpu + winit
- **Git analysis:** git2
- **Async runtime:** tokio
- **Embedding:** TF-IDF (custom, no deps). Ollama integration deferred to v0.2.
- **Layout:** Custom force-directed simulation. UMAP-style init if needed.
- **Serialization:** serde + bincode for embedding cache

## Scope

### In (prototype v0.1)

- Single repo visualization via CLI arg
- Git co-change analysis (git2, last 500 commits)
- TF-IDF fallback embeddings (self-contained, no external deps)
- Force-directed 2D layout with configurable depth axis
- wgpu rendering: nodes with glow, edges, smooth camera
- Three zoom levels with interpolation
- Color mode toggle (file type / recency)
- Depth mode toggle (recency / coupling / importance)
- File inspector sidebar on click
- Keyboard shortcuts for mode switching
- Embedding cache to disk

### Deferred (v0.2+)

- Ollama embedding integration
- Agent activity overlay (event stream via `cviz track` CLI)
- Multi-repo view
- Agent activity color mode
- Full 3D camera (if 2.5D proves too flat)
- Git branch comparison view
- Search/filter by filename

### Non-Goals

- Code editing — visualizer only, never an editor
- AI chat integration — shows agent activity, doesn't run agents
- Web version — native Rust + wgpu only
