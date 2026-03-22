# cviz — 2.5D Codebase Visualizer

Visualize any git repository as an interactive 2.5D map with real-time agent activity tracking.
Files are positioned by co-change frequency and semantic similarity.
When AI coding agents (Claude Code, etc.) touch files, they light up cyan in real-time.

## Usage

```bash
cargo run --release -- /path/to/git/repo
```

## Controls

| Key | Action |
|-----|--------|
| Scroll | Zoom to cursor |
| Click + drag | Pan |
| Click node | Select + inspect (prints file details to terminal) |
| F | Fit all nodes in view |
| G | Toggle edge visibility |
| Q | Quit |
| + / - | Keyboard zoom |
| 1 / 2 / 3 | Depth mode: recency / coupling / importance |
| C | Cycle color mode (file type / recency) |
| Space | Reset camera |
| Escape / Right-click | Deselect |

## Agent Activity Overlay

cviz listens on a Unix domain socket for real-time file activity events. When an AI agent reads or edits a file, the corresponding node lights up cyan with a glowing ring.

### Setup with Claude Code

1. Copy the hook script:
```bash
cp hooks/cviz-hook.sh ~/.claude/hooks/
chmod +x ~/.claude/hooks/cviz-hook.sh
```

2. Add to `~/.claude/settings.json`:
```json
{
  "hooks": {
    "PostToolUse": [
      {
        "matcher": "Read|Edit|Write",
        "hooks": [
          {
            "type": "command",
            "command": "~/.claude/hooks/cviz-hook.sh"
          }
        ]
      }
    ]
  }
}
```

3. Start cviz on your repo, then use Claude Code — every file it touches lights up.

### Manual testing

```bash
# Find the socket
SOCK=$(ls /tmp/cviz-*.sock 2>/dev/null | head -1)

# Send an event
echo '{"file":"src/main.rs","action":"edit","agent":"claude","timestamp":1711000000}' | nc -U "$SOCK"
```

## How It Works

**Pipeline architecture:** Five async stages connected by channels, running on a background tokio thread while the renderer owns the main thread.

```
GitCollector → Embedder → LayoutEngine → Renderer
   (git2)      (TF-IDF)   (force-directed)  (wgpu)
                                    ↑
                           SocketListener ← Claude Code hook
```

- **GitCollector** walks commit history via `git2`, builds a co-change matrix (Jaccard similarity).
- **Embedder** tokenizes file contents (camelCase/snake_case splitting) and computes TF-IDF vectors.
- **LayoutEngine** runs Barnes-Hut O(n log n) force-directed simulation with 500 iterations.
- **Renderer** draws instanced circles with glow shaders, convex hull directory backgrounds, edge lines, and bitmap font labels.
- **SocketListener** accepts real-time activity events via Unix domain socket, lighting up active nodes cyan.

## Visual Encoding

| Property | What it shows |
|----------|--------------|
| Position (XY) | Co-change + semantic similarity |
| Size | Lines of code (log scale) |
| Color | File type or change recency (toggle with `C`) |
| Glow | Commit frequency |
| Depth (Z) | Recency, coupling, or importance (toggle with `1/2/3`) |
| Edge opacity | Co-change strength |
| Hull background | Directory grouping |
| Cyan tint + ring | Agent activity (files being read/edited) |
| Labels | Filenames (visible when zoomed in) |

## File Inspector

Click any node to see detailed information in the terminal:

```
═══ src/render/state.rs ═══
  Directory:   src/render/
  Lines:       487
  Commits:     14
  Last change: 1 day ago
  Co-changed with:
    50%  src/app.rs
    36%  src/main.rs
    31%  src/render/camera.rs
```

## Requirements

- Rust 1.94+
- A git repository to visualize
- GPU with Vulkan/Metal support (wgpu)
- `jq` (for the Claude Code hook)
