# cviz — 2.5D Codebase Visualizer

Visualize any git repository as an interactive 2.5D map.
Files are positioned by co-change frequency and semantic similarity.
Depth, color, and glow encode configurable metrics.

## Usage

```bash
cargo run --release -- /path/to/git/repo
```

## Controls

| Key | Action |
|-----|--------|
| Scroll | Zoom in/out |
| Click + drag | Pan |
| Click node | Select + inspect (prints to terminal) |
| 1 / 2 / 3 | Depth mode: recency / coupling / importance |
| c | Cycle color mode (file type / recency) |
| Space | Reset camera |
| Escape / Right-click | Deselect |

## How It Works

**Pipeline architecture:** Four async stages connected by channels, running on a background tokio thread while the renderer owns the main thread.

```
GitCollector → Embedder → LayoutEngine → Renderer
   (git2)      (TF-IDF)   (force-directed)  (wgpu)
```

- **GitCollector** walks commit history via `git2`, builds a co-change matrix (Jaccard similarity), and polls for changes every 2s.
- **Embedder** tokenizes file contents (camelCase/snake_case splitting) and computes TF-IDF vectors. Files with similar code cluster together.
- **LayoutEngine** runs a force-directed simulation: co-change edges are springs, embedding similarity adds gentle attraction. Computes depth, color, and glow per node.
- **Renderer** draws instanced circles with glow shaders and line edges. Smooth interpolation on all transitions.

## Visual Encoding

| Property | What it shows |
|----------|--------------|
| Position (XY) | Co-change + semantic similarity |
| Size | Lines of code (log scale) |
| Color | File type or change recency (toggle with `c`) |
| Glow | Commit frequency |
| Depth (Z) | Recency, coupling, or importance (toggle with `1/2/3`) |
| Edge opacity | Co-change strength |

## Requirements

- Rust 1.94+
- A git repository to visualize
- GPU with Vulkan/Metal support (wgpu)
