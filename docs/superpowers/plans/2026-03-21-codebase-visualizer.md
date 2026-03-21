# Codebase Visualizer (cviz) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a native 2.5D codebase visualizer in Rust that maps git repos as interactive spatial landscapes where file positions reflect co-change and semantic relationships.

**Architecture:** Single binary with four async pipeline stages (GitCollector → Embedder → LayoutEngine → Renderer) connected by tokio mpsc channels. Tokio runs on a background thread; winit owns the main thread. The renderer never blocks on data stages.

**Tech Stack:** Rust 1.94+, wgpu 28.0, winit 0.30, git2 0.20, tokio 1.x, pollster, env_logger

**Spec:** `docs/superpowers/specs/2026-03-21-codebase-visualizer-design.md`

---

## File Structure

```
src/
├── main.rs              — CLI arg parsing, tokio runtime setup, winit event loop launch
├── app.rs               — ApplicationHandler impl, owns renderer + channel receivers
├── pipeline/
│   ├── mod.rs           — re-exports, shared message types (FileGraph, EmbeddingMap, SceneGraph)
│   ├── git_collector.rs — git2 commit walking, co-change matrix, diff polling
│   ├── embedder.rs      — TF-IDF tokenizer, embedding computation, cache
│   └── layout.rs        — force-directed simulation, depth axis computation
├── render/
│   ├── mod.rs           — re-exports
│   ├── state.rs         — wgpu device/surface/pipeline setup, resize handling
│   ├── camera.rs        — pan, zoom, projection matrix, smooth interpolation
│   ├── node.rs          — circle instancing: vertex buffer, instance data, shader
│   └── edge.rs          — line rendering between connected nodes
├── scene.rs             — SceneGraph: node positions, colors, sizes, edges, depth values
└── ui.rs                — hover tooltip, click selection, sidebar inspector, keyboard shortcuts

tests/
├── git_collector_test.rs
├── embedder_test.rs
├── layout_test.rs
└── scene_test.rs

assets/
└── shaders/
    ├── node.wgsl        — circle + glow fragment shader
    └── edge.wgsl        — line shader with opacity
```

---

## Task 1: Project Scaffold & Window

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`
- Create: `src/app.rs`
- Create: `src/render/mod.rs`
- Create: `src/render/state.rs`

- [ ] **Step 1: Create Cargo.toml**

```toml
[package]
name = "cviz"
version = "0.1.0"
edition = "2021"
rust-version = "1.94"
authors = ["Connor Faulkner <connorjfaulkner@gmail.com>"]
description = "2.5D codebase visualizer"
license = "MIT"

[dependencies]
wgpu = "28"
winit = "0.30"
pollster = "0.4"
env_logger = "0.11"
log = "0.4"
tokio = { version = "1", features = ["rt-multi-thread", "sync", "time"] }
git2 = "0.20"
serde = { version = "1", features = ["derive"] }
bincode = "1"
bytemuck = { version = "1", features = ["derive"] }
glam = "0.29"
clap = { version = "4", features = ["derive"] }

[profile.release]
opt-level = 3
lto = true
```

- [ ] **Step 2: Create minimal main.rs with CLI arg**

```rust
// src/main.rs
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "cviz", about = "2.5D codebase visualizer")]
struct Cli {
    /// Path to git repository
    repo: PathBuf,
}

fn main() {
    env_logger::init();
    let cli = Cli::parse();

    if !cli.repo.join(".git").exists() {
        eprintln!("Error: {} is not a git repository", cli.repo.display());
        std::process::exit(1);
    }

    log::info!("Opening repo: {}", cli.repo.display());

    let event_loop = winit::event_loop::EventLoop::new().expect("Failed to create event loop");
    let mut app = app::App::new(cli.repo);
    event_loop.run_app(&mut app).expect("Event loop error");
}

mod app;
mod render;
```

- [ ] **Step 3: Create app.rs with ApplicationHandler**

```rust
// src/app.rs
use std::path::PathBuf;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowId};

use crate::render::RenderState;

pub struct App {
    repo_path: PathBuf,
    window: Option<Window>,
    render_state: Option<RenderState>,
}

impl App {
    pub fn new(repo_path: PathBuf) -> Self {
        Self {
            repo_path,
            window: None,
            render_state: None,
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title(format!("cviz — {}", self.repo_path.display()))
            .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 800.0));
        let window = event_loop.create_window(attrs).expect("Failed to create window");
        self.render_state = Some(pollster::block_on(RenderState::new(&window)));
        self.window = Some(window);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(state) = &mut self.render_state {
                    state.resize(size);
                }
            }
            WindowEvent::RedrawRequested => {
                if let Some(state) = &mut self.render_state {
                    state.render();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}
```

- [ ] **Step 4: Create render/mod.rs**

```rust
// src/render/mod.rs
mod state;
pub use state::RenderState;
```

- [ ] **Step 5: Create render/state.rs with wgpu init**

```rust
// src/render/state.rs
use winit::dpi::PhysicalSize;
use winit::window::Window;

pub struct RenderState {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
}

impl RenderState {
    pub async fn new(window: &Window) -> Self {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });

        // SAFETY: surface lives as long as window, which lives as long as app
        let surface = unsafe {
            instance.create_surface(window).expect("Failed to create surface")
        };

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .expect("Failed to find adapter");

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("cviz device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: Default::default(),
            }, None)
            .await
            .expect("Failed to create device");

        let size = window.inner_size();
        let caps = surface.get_capabilities(&adapter);
        let format = caps.formats.iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        Self { device, queue, surface, config }
    }

    pub fn resize(&mut self, size: PhysicalSize<u32>) {
        if size.width > 0 && size.height > 0 {
            self.config.width = size.width;
            self.config.height = size.height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    pub fn render(&mut self) {
        let output = match self.surface.get_current_texture() {
            Ok(t) => t,
            Err(wgpu::SurfaceError::Lost) => {
                self.surface.configure(&self.device, &self.config);
                return;
            }
            Err(e) => {
                log::error!("Surface error: {:?}", e);
                return;
            }
        };

        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("render encoder"),
        });

        {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("clear pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.04, g: 0.04, b: 0.10, a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
    }
}
```

- [ ] **Step 6: Verify it compiles and opens a dark window**

Run: `cargo run -- ~/Projects/ising-rs`
Expected: Dark blue-black window titled "cviz — /Users/faulknco/Projects/ising-rs". No errors.

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml src/
git commit -m "feat: project scaffold — wgpu window with dark background"
```

---

## Task 2: Pipeline Message Types & Channel Wiring

**Files:**
- Create: `src/pipeline/mod.rs`
- Create: `src/scene.rs`
- Modify: `src/main.rs`
- Modify: `src/app.rs`

- [ ] **Step 1: Create scene.rs with shared types**

```rust
// src/scene.rs
use glam::Vec2;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Unique file identifier (relative path from repo root)
pub type FileId = String;

/// Output of GitCollector
#[derive(Debug, Clone)]
pub struct FileGraph {
    pub files: HashMap<FileId, FileInfo>,
    pub co_change: Vec<(FileId, FileId, f32)>, // (a, b, score 0..1)
}

/// Metadata for a single file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub path: PathBuf,
    pub lines: usize,
    pub last_modified: i64, // unix timestamp
    pub commit_count: usize,
}

/// Output of Embedder
#[derive(Debug, Clone)]
pub struct EmbeddingMap {
    pub embeddings: HashMap<FileId, Vec<f32>>,
}

/// Output of LayoutEngine — what the renderer draws
#[derive(Debug, Clone)]
pub struct SceneGraph {
    pub nodes: Vec<SceneNode>,
    pub edges: Vec<SceneEdge>,
}

#[derive(Debug, Clone)]
pub struct SceneNode {
    pub id: FileId,
    pub pos: Vec2,
    pub depth: f32,
    pub radius: f32,
    pub color: [f32; 4],
    pub glow: f32,
}

#[derive(Debug, Clone)]
pub struct SceneEdge {
    pub from: Vec2,
    pub to: Vec2,
    pub opacity: f32,
}

/// Depth axis mode
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DepthMode {
    Recency,
    Coupling,
    Importance,
}

/// Color mode
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ColorMode {
    FileType,
    Recency,
}
```

- [ ] **Step 2: Create pipeline/mod.rs**

```rust
// src/pipeline/mod.rs
pub mod git_collector;
pub mod embedder;
pub mod layout;
```

- [ ] **Step 3: Create stub pipeline modules**

Create `src/pipeline/git_collector.rs`:
```rust
// src/pipeline/git_collector.rs
use crate::scene::FileGraph;
use std::path::PathBuf;
use tokio::sync::mpsc;

pub async fn run(repo_path: PathBuf, tx: mpsc::Sender<FileGraph>) {
    log::info!("GitCollector started for {}", repo_path.display());
    // TODO: implement in Task 3
}
```

Create `src/pipeline/embedder.rs`:
```rust
// src/pipeline/embedder.rs
use crate::scene::{EmbeddingMap, FileGraph};
use std::path::PathBuf;
use tokio::sync::mpsc;

pub async fn run(repo_path: PathBuf, rx: mpsc::Receiver<FileGraph>, tx: mpsc::Sender<(FileGraph, EmbeddingMap)>) {
    log::info!("Embedder started");
    // TODO: implement in Task 4
}
```

Create `src/pipeline/layout.rs`:
```rust
// src/pipeline/layout.rs
use crate::scene::{ColorMode, DepthMode, EmbeddingMap, FileGraph, SceneGraph};
use tokio::sync::mpsc;
use std::sync::Arc;
use tokio::sync::RwLock;

pub async fn run(
    rx: mpsc::Receiver<(FileGraph, EmbeddingMap)>,
    scene_tx: mpsc::Sender<SceneGraph>,
    depth_mode: Arc<RwLock<DepthMode>>,
    color_mode: Arc<RwLock<ColorMode>>,
) {
    log::info!("LayoutEngine started");
    // TODO: implement in Task 5
}
```

- [ ] **Step 4: Wire channels in main.rs**

Update `src/main.rs`:
```rust
use clap::Parser;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

mod app;
mod pipeline;
mod render;
mod scene;

use scene::DepthMode;

#[derive(Parser)]
#[command(name = "cviz", about = "2.5D codebase visualizer")]
struct Cli {
    /// Path to git repository
    repo: PathBuf,
}

fn main() {
    env_logger::init();
    let cli = Cli::parse();

    if !cli.repo.join(".git").exists() {
        eprintln!("Error: {} is not a git repository", cli.repo.display());
        std::process::exit(1);
    }

    // Pipeline channels: GitCollector → Embedder → LayoutEngine → Renderer
    // Embedder forwards (FileGraph, EmbeddingMap) tuples so layout gets both signals.
    let (git_tx, git_rx) = mpsc::channel::<scene::FileGraph>(4);
    let (embed_tx, embed_rx) = mpsc::channel::<(scene::FileGraph, scene::EmbeddingMap)>(4);
    let (scene_tx, scene_rx) = mpsc::channel::<scene::SceneGraph>(4);
    let depth_mode = Arc::new(RwLock::new(DepthMode::Recency));
    let color_mode = Arc::new(RwLock::new(scene::ColorMode::FileType));

    let repo_path = cli.repo.clone();
    let embed_repo_path = cli.repo.clone();
    let dm = depth_mode.clone();
    let cm = color_mode.clone();

    // Spawn tokio runtime on background thread
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
        rt.block_on(async move {
            tokio::spawn(pipeline::git_collector::run(repo_path, git_tx));
            tokio::spawn(pipeline::embedder::run(embed_repo_path, git_rx, embed_tx));
            tokio::spawn(pipeline::layout::run(embed_rx, scene_tx, dm, cm));
            // Keep runtime alive
            tokio::signal::ctrl_c().await.ok();
        });
    });

    let event_loop = winit::event_loop::EventLoop::new().expect("Failed to create event loop");
    let mut app = app::App::new(cli.repo, scene_rx, depth_mode, color_mode);
    event_loop.run_app(&mut app).expect("Event loop error");
}
```

- [ ] **Step 5: Update app.rs to accept scene channel**

Update `App::new` signature to accept `mpsc::Receiver<SceneGraph>` and `Arc<RwLock<DepthMode>>`. Store them. In `about_to_wait`, call `scene_rx.try_recv()` to check for new scene data. Store the latest `SceneGraph` for rendering.

- [ ] **Step 6: Verify it compiles**

Run: `cargo build`
Expected: Compiles with warnings about unused variables (stubs not yet implemented).

- [ ] **Step 7: Commit**

```bash
git add src/
git commit -m "feat: pipeline message types and channel wiring"
```

---

## Task 3: Git Collector

**Files:**
- Modify: `src/pipeline/git_collector.rs`
- Create: `tests/git_collector_test.rs`

- [ ] **Step 1: Write test for co-change extraction**

```rust
// tests/git_collector_test.rs
use std::process::Command;
use tempfile::TempDir;

fn create_test_repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    let p = dir.path();

    let run = |args: &[&str]| {
        Command::new("git")
            .args(args)
            .current_dir(p)
            .output()
            .expect("git command failed");
    };

    run(&["init"]);
    run(&["-c", "user.name=Test", "-c", "user.email=test@test.com", "commit", "--allow-empty", "-m", "init"]);

    // Commit 1: a.rs + b.rs together
    std::fs::write(p.join("a.rs"), "fn a() {}").unwrap();
    std::fs::write(p.join("b.rs"), "fn b() {}").unwrap();
    run(&["add", "."]);
    run(&["-c", "user.name=Test", "-c", "user.email=test@test.com", "commit", "-m", "add a and b"]);

    // Commit 2: a.rs + c.rs together
    std::fs::write(p.join("a.rs"), "fn a() { updated }").unwrap();
    std::fs::write(p.join("c.rs"), "fn c() {}").unwrap();
    run(&["add", "."]);
    run(&["-c", "user.name=Test", "-c", "user.email=test@test.com", "commit", "-m", "update a, add c"]);

    dir
}

#[test]
fn test_co_change_matrix() {
    let dir = create_test_repo();
    let graph = cviz::pipeline::git_collector::collect_file_graph(dir.path(), 500);

    // a.rs appears in 2 commits, b.rs in 1, c.rs in 1
    assert_eq!(graph.files.len(), 3);
    assert_eq!(graph.files["a.rs"].commit_count, 2);
    assert_eq!(graph.files["b.rs"].commit_count, 1);

    // a+b co-changed in 1 commit, union is 2 → score = 0.5
    let ab_score = graph.co_change.iter()
        .find(|(a, b, _)| (a == "a.rs" && b == "b.rs") || (a == "b.rs" && b == "a.rs"))
        .map(|(_, _, s)| *s)
        .unwrap_or(0.0);
    assert!((ab_score - 0.5).abs() < 0.01, "a+b score was {}", ab_score);

    // a+c co-changed in 1 commit, union is 2 → score = 0.5
    let ac_score = graph.co_change.iter()
        .find(|(a, b, _)| (a == "a.rs" && b == "c.rs") || (a == "c.rs" && b == "a.rs"))
        .map(|(_, _, s)| *s)
        .unwrap_or(0.0);
    assert!((ac_score - 0.5).abs() < 0.01, "a+c score was {}", ac_score);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test git_collector_test`
Expected: FAIL — `collect_file_graph` doesn't exist yet.

- [ ] **Step 3: Implement git_collector.rs**

```rust
// src/pipeline/git_collector.rs
use crate::scene::{FileGraph, FileId, FileInfo};
use git2::{Repository, Sort};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;

/// Walk commit history and build co-change matrix.
/// Exposed as a sync function for testing; the async `run` wraps it.
pub fn collect_file_graph(repo_path: &Path, max_commits: usize) -> FileGraph {
    let repo = Repository::open(repo_path).expect("Failed to open repo");

    let mut revwalk = repo.revwalk().expect("Failed to create revwalk");
    revwalk.push_head().expect("Failed to push HEAD");
    revwalk.set_sorting(Sort::TIME).expect("Failed to set sorting");

    // Per-commit file sets
    let mut commit_files: Vec<HashSet<FileId>> = Vec::new();
    // Per-file metadata
    let mut file_commits: HashMap<FileId, Vec<i64>> = HashMap::new();
    let mut file_lines: HashMap<FileId, usize> = HashMap::new();

    for (i, oid) in revwalk.enumerate() {
        if i >= max_commits {
            break;
        }
        let oid = match oid {
            Ok(o) => o,
            Err(_) => continue,
        };
        let commit = match repo.find_commit(oid) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let tree = match commit.tree() {
            Ok(t) => t,
            Err(_) => continue,
        };

        let parent_tree = commit.parent(0).ok().and_then(|p| p.tree().ok());

        let diff = repo
            .diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None)
            .ok();

        let mut files_in_commit = HashSet::new();
        if let Some(diff) = diff {
            diff.foreach(
                &mut |delta, _| {
                    if let Some(path) = delta.new_file().path() {
                        let id = path.to_string_lossy().to_string();
                        files_in_commit.insert(id);
                    }
                    true
                },
                None,
                None,
                None,
            )
            .ok();
        }

        let timestamp = commit.time().seconds();
        for f in &files_in_commit {
            file_commits.entry(f.clone()).or_default().push(timestamp);
        }
        commit_files.push(files_in_commit);
    }

    // Count lines for files that still exist
    for file_id in file_commits.keys() {
        let full_path = repo_path.join(file_id);
        if full_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&full_path) {
                file_lines.insert(file_id.clone(), content.lines().count());
            }
        }
    }

    // Build FileInfo map
    let mut files: HashMap<FileId, FileInfo> = HashMap::new();
    for (id, timestamps) in &file_commits {
        let full_path = repo_path.join(id);
        if !full_path.exists() {
            continue; // skip deleted files
        }
        files.insert(
            id.clone(),
            FileInfo {
                path: PathBuf::from(id),
                lines: *file_lines.get(id).unwrap_or(&0),
                last_modified: *timestamps.iter().max().unwrap_or(&0),
                commit_count: timestamps.len(),
            },
        );
    }

    // Build co-change matrix
    let file_ids: Vec<FileId> = files.keys().cloned().collect();
    let mut co_change = Vec::new();
    for i in 0..file_ids.len() {
        for j in (i + 1)..file_ids.len() {
            let a = &file_ids[i];
            let b = &file_ids[j];
            let together = commit_files
                .iter()
                .filter(|c| c.contains(a) && c.contains(b))
                .count();
            if together == 0 {
                continue;
            }
            let either = commit_files
                .iter()
                .filter(|c| c.contains(a) || c.contains(b))
                .count();
            let score = together as f32 / either as f32;
            co_change.push((a.clone(), b.clone(), score));
        }
    }

    FileGraph { files, co_change }
}

pub async fn run(repo_path: PathBuf, tx: mpsc::Sender<FileGraph>) {
    log::info!("GitCollector started for {}", repo_path.display());

    // Initial full scan
    let graph = collect_file_graph(&repo_path, 500);
    log::info!(
        "GitCollector: {} files, {} co-change pairs",
        graph.files.len(),
        graph.co_change.len()
    );
    if tx.send(graph).await.is_err() {
        return;
    }

    // Poll for changes every 2 seconds
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        let graph = collect_file_graph(&repo_path, 500);
        if tx.send(graph).await.is_err() {
            break;
        }
    }
}
```

- [ ] **Step 4: Make the function public from lib**

Create `src/lib.rs`:
```rust
pub mod pipeline;
pub mod scene;
```

- [ ] **Step 5: Add tempfile dev-dependency to Cargo.toml**

Add to Cargo.toml:
```toml
[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 6: Run test**

Run: `cargo test --test git_collector_test`
Expected: PASS

- [ ] **Step 7: Verify on real repo**

Run: `cargo run -- ~/Projects/ising-rs`
Expected: Log output showing file count and co-change pairs. Window still opens with dark background.

- [ ] **Step 8: Commit**

```bash
git add src/ tests/ Cargo.toml
git commit -m "feat: git collector — co-change matrix from commit history"
```

---

## Task 4: TF-IDF Embedder

**Files:**
- Modify: `src/pipeline/embedder.rs`
- Create: `tests/embedder_test.rs`

- [ ] **Step 1: Write test for TF-IDF embeddings**

```rust
// tests/embedder_test.rs

#[test]
fn test_tfidf_similar_files_closer() {
    use std::collections::HashMap;

    let files: HashMap<String, String> = HashMap::from([
        ("a.rs".into(), "fn main() { let x = compute(); println!(x); }".into()),
        ("b.rs".into(), "fn helper() { let y = compute(); return y; }".into()),
        ("readme.md".into(), "# Project\nThis is a readme with no code".into()),
    ]);

    let embeddings = cviz::pipeline::embedder::compute_tfidf(&files);

    assert_eq!(embeddings.len(), 3);

    // a.rs and b.rs should be more similar than a.rs and readme.md
    let sim_ab = cosine_sim(&embeddings["a.rs"], &embeddings["b.rs"]);
    let sim_ac = cosine_sim(&embeddings["a.rs"], &embeddings["readme.md"]);
    assert!(sim_ab > sim_ac, "a-b sim ({}) should exceed a-readme sim ({})", sim_ab, sim_ac);
}

fn cosine_sim(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let mag_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let mag_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if mag_a == 0.0 || mag_b == 0.0 { 0.0 } else { dot / (mag_a * mag_b) }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test embedder_test`
Expected: FAIL — `compute_tfidf` doesn't exist.

- [ ] **Step 3: Implement embedder.rs**

```rust
// src/pipeline/embedder.rs
use crate::scene::{EmbeddingMap, FileGraph, FileId};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use tokio::sync::mpsc;

/// Tokenize a string: split on whitespace, punctuation, camelCase, snake_case.
fn tokenize(s: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    for word in s.split(|c: char| c.is_whitespace() || "{}()[];,:.\"'`#=<>+-*/&|!?@".contains(c)) {
        if word.is_empty() {
            continue;
        }
        // Split camelCase
        let mut start = 0;
        let chars: Vec<char> = word.chars().collect();
        for i in 1..chars.len() {
            if chars[i].is_uppercase() && chars[i - 1].is_lowercase() {
                let token = chars[start..i].iter().collect::<String>().to_lowercase();
                if token.len() > 1 {
                    tokens.push(token);
                }
                start = i;
            }
        }
        // Split snake_case
        let remainder: String = chars[start..].iter().collect();
        for part in remainder.split('_') {
            let t = part.to_lowercase();
            if t.len() > 1 {
                tokens.push(t);
            }
        }
    }
    tokens
}

/// Compute TF-IDF embeddings for a set of files.
/// Input: map of file_id → file_content.
/// Output: map of file_id → embedding vector.
pub fn compute_tfidf(files: &HashMap<String, String>) -> HashMap<String, Vec<f32>> {
    let n_docs = files.len() as f32;

    // Tokenize all files
    let file_tokens: HashMap<&str, Vec<String>> = files
        .iter()
        .map(|(id, content)| (id.as_str(), tokenize(content)))
        .collect();

    // Build vocabulary and document frequencies
    let mut vocab: HashSet<String> = HashSet::new();
    let mut doc_freq: HashMap<String, usize> = HashMap::new();

    for tokens in file_tokens.values() {
        let unique: HashSet<&String> = tokens.iter().collect();
        for t in unique {
            *doc_freq.entry(t.clone()).or_default() += 1;
            vocab.insert(t.clone());
        }
    }

    // Sort vocab for consistent ordering
    let mut vocab: Vec<String> = vocab.into_iter().collect();
    vocab.sort();

    let token_to_idx: HashMap<&str, usize> = vocab.iter().enumerate().map(|(i, t)| (t.as_str(), i)).collect();
    let dim = vocab.len();

    // Compute TF-IDF vectors
    let mut embeddings = HashMap::new();
    for (id, tokens) in &file_tokens {
        let mut tf: HashMap<&str, f32> = HashMap::new();
        for t in tokens {
            *tf.entry(t.as_str()).or_default() += 1.0;
        }
        let max_tf = tf.values().cloned().fold(0.0f32, f32::max).max(1.0);

        let mut vec = vec![0.0f32; dim];
        for (token, count) in &tf {
            if let Some(&idx) = token_to_idx.get(token) {
                let tf_norm = count / max_tf;
                let idf = (n_docs / *doc_freq.get(*token).unwrap_or(&1) as f32).ln() + 1.0;
                vec[idx] = tf_norm * idf;
            }
        }

        // L2 normalize
        let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for v in &mut vec {
                *v /= norm;
            }
        }

        embeddings.insert(id.to_string(), vec);
    }

    embeddings
}

pub async fn run(
    repo_path: PathBuf,
    mut graph_rx: mpsc::Receiver<FileGraph>,
    tx: mpsc::Sender<(FileGraph, EmbeddingMap)>,
) {
    log::info!("Embedder started");

    while let Some(graph) = graph_rx.recv().await {
        // Read file contents for embedding — resolve relative paths against repo root
        let mut file_contents: HashMap<String, String> = HashMap::new();
        for (id, info) in &graph.files {
            let full_path = repo_path.join(&info.path);
            if let Ok(content) = std::fs::read_to_string(&full_path) {
                file_contents.insert(id.clone(), content);
            }
        }

        let raw_embeddings = compute_tfidf(&file_contents);
        let embed_map = EmbeddingMap {
            embeddings: raw_embeddings,
        };

        log::info!("Embedder: computed {} embeddings", embed_map.embeddings.len());

        // Forward both graph and embeddings to layout
        if tx.send((graph, embed_map)).await.is_err() {
            break;
        }
    }
}
```

- [ ] **Step 4: Update lib.rs exports**

Ensure `src/lib.rs` exports the pipeline module so tests can access `compute_tfidf`.

- [ ] **Step 5: Run test**

Run: `cargo test --test embedder_test`
Expected: PASS — Rust source files cluster closer than Rust vs markdown.

- [ ] **Step 6: Commit**

```bash
git add src/ tests/
git commit -m "feat: TF-IDF embedder with tokenizer"
```

---

## Task 5: Force-Directed Layout Engine

**Files:**
- Modify: `src/pipeline/layout.rs`
- Create: `tests/layout_test.rs`

- [ ] **Step 1: Write test for layout producing separated clusters**

```rust
// tests/layout_test.rs
use std::collections::HashMap;
use cviz::scene::*;
use cviz::pipeline::layout::compute_layout;

#[test]
fn test_connected_files_closer_than_unconnected() {
    let mut files = HashMap::new();
    files.insert("a.rs".into(), FileInfo { path: "a.rs".into(), lines: 100, last_modified: 1000, commit_count: 10 });
    files.insert("b.rs".into(), FileInfo { path: "b.rs".into(), lines: 50, last_modified: 900, commit_count: 5 });
    files.insert("z.rs".into(), FileInfo { path: "z.rs".into(), lines: 30, last_modified: 100, commit_count: 1 });

    let graph = FileGraph {
        files,
        co_change: vec![("a.rs".into(), "b.rs".into(), 0.8)],
        // z.rs has no co-change with anyone
    };

    let embeddings = HashMap::from([
        ("a.rs".into(), vec![1.0, 0.0]),
        ("b.rs".into(), vec![0.9, 0.1]),
        ("z.rs".into(), vec![0.0, 1.0]),
    ]);
    let embed_map = EmbeddingMap { embeddings };

    let scene = compute_layout(&graph, &embed_map, DepthMode::Recency, ColorMode::FileType, 200);

    // Find nodes
    let a = scene.nodes.iter().find(|n| n.id == "a.rs").unwrap();
    let b = scene.nodes.iter().find(|n| n.id == "b.rs").unwrap();
    let z = scene.nodes.iter().find(|n| n.id == "z.rs").unwrap();

    let dist_ab = a.pos.distance(b.pos);
    let dist_az = a.pos.distance(z.pos);

    assert!(dist_ab < dist_az, "a-b ({:.2}) should be closer than a-z ({:.2})", dist_ab, dist_az);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test layout_test`
Expected: FAIL — `compute_layout` doesn't exist.

- [ ] **Step 3: Implement layout.rs**

```rust
// src/pipeline/layout.rs
use crate::scene::*;
use glam::Vec2;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

/// Compute cosine distance between two embedding vectors.
fn embedding_distance(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 1.0;
    }
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let mag_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let mag_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if mag_a == 0.0 || mag_b == 0.0 {
        1.0
    } else {
        1.0 - (dot / (mag_a * mag_b))
    }
}

/// Color for a file extension (FileType mode).
fn color_for_extension(path: &str) -> [f32; 4] {
    if path.ends_with(".rs") { [0.97, 0.44, 0.44, 1.0] }       // red
    else if path.ends_with(".toml") { [0.98, 0.75, 0.14, 1.0] } // amber
    else if path.ends_with(".md") { [0.20, 0.83, 0.60, 1.0] }   // green
    else if path.contains("test") { [0.51, 0.55, 0.97, 1.0] }   // indigo
    else { [0.58, 0.64, 0.72, 1.0] }                             // slate
}

/// Color by change recency: cold blue (stale) → green → hot red (recent).
fn color_for_recency(normalized_recency: f32) -> [f32; 4] {
    let t = normalized_recency.clamp(0.0, 1.0);
    if t < 0.5 {
        // Blue → Green
        let s = t * 2.0;
        [0.12 * (1.0 - s), 0.23 + 0.60 * s, 0.37 * (1.0 - s) + 0.40 * s, 1.0]
    } else {
        // Green → Red
        let s = (t - 0.5) * 2.0;
        [0.12 + 0.85 * s, 0.83 * (1.0 - s) + 0.27 * s, 0.40 * (1.0 - s), 1.0]
    }
}

/// Run force-directed layout and produce a SceneGraph.
/// `iterations`: number of simulation steps.
pub fn compute_layout(
    graph: &FileGraph,
    embed_map: &EmbeddingMap,
    depth_mode: DepthMode,
    color_mode: ColorMode,
    iterations: usize,
) -> SceneGraph {
    let ids: Vec<FileId> = graph.files.keys().cloned().collect();
    let n = ids.len();
    if n == 0 {
        return SceneGraph { nodes: vec![], edges: vec![] };
    }

    let id_to_idx: HashMap<&str, usize> = ids.iter().enumerate().map(|(i, id)| (id.as_str(), i)).collect();

    // Initialize positions in a circle
    let mut positions: Vec<Vec2> = (0..n)
        .map(|i| {
            let angle = (i as f32 / n as f32) * std::f32::consts::TAU;
            Vec2::new(angle.cos() * 5.0, angle.sin() * 5.0)
        })
        .collect();

    // Build co-change lookup
    let mut co_change_map: HashMap<(usize, usize), f32> = HashMap::new();
    for (a, b, score) in &graph.co_change {
        if let (Some(&ia), Some(&ib)) = (id_to_idx.get(a.as_str()), id_to_idx.get(b.as_str())) {
            let key = if ia < ib { (ia, ib) } else { (ib, ia) };
            co_change_map.insert(key, *score);
        }
    }

    // Force-directed simulation
    let repulsion = 2.0;
    let attraction = 0.5;
    let damping = 0.95;
    let mut velocities = vec![Vec2::ZERO; n];

    for _ in 0..iterations {
        let mut forces = vec![Vec2::ZERO; n];

        // Repulsion between all pairs
        for i in 0..n {
            for j in (i + 1)..n {
                let diff = positions[i] - positions[j];
                let dist = diff.length().max(0.01);
                let force = diff.normalize() * repulsion / (dist * dist);
                forces[i] += force;
                forces[j] -= force;
            }
        }

        // Attraction for co-changed pairs
        for (&(i, j), &score) in &co_change_map {
            let diff = positions[j] - positions[i];
            let dist = diff.length();
            let force = diff.normalize() * attraction * score * dist;
            forces[i] += force;
            forces[j] -= force;
        }

        // Embedding similarity attraction (weaker)
        for i in 0..n {
            for j in (i + 1)..n {
                if let (Some(ea), Some(eb)) = (
                    embed_map.embeddings.get(&ids[i]),
                    embed_map.embeddings.get(&ids[j]),
                ) {
                    let sim = 1.0 - embedding_distance(ea, eb);
                    if sim > 0.3 {
                        let diff = positions[j] - positions[i];
                        let dist = diff.length();
                        let force = diff.normalize() * 0.1 * sim * dist;
                        forces[i] += force;
                        forces[j] -= force;
                    }
                }
            }
        }

        // Apply forces
        for i in 0..n {
            velocities[i] = (velocities[i] + forces[i] * 0.01) * damping;
            positions[i] += velocities[i];
        }
    }

    // Compute depth
    let max_timestamp = graph.files.values().map(|f| f.last_modified).max().unwrap_or(1);
    let min_timestamp = graph.files.values().map(|f| f.last_modified).min().unwrap_or(0);
    let time_range = (max_timestamp - min_timestamp).max(1) as f32;

    // Coupling: count co-change edges per node
    let mut coupling: Vec<f32> = vec![0.0; n];
    for &(i, j) in co_change_map.keys() {
        coupling[i] += 1.0;
        coupling[j] += 1.0;
    }
    let max_coupling = coupling.iter().cloned().fold(0.0f32, f32::max).max(1.0);

    let nodes: Vec<SceneNode> = ids
        .iter()
        .enumerate()
        .map(|(i, id)| {
            let info = &graph.files[id];
            let depth = match depth_mode {
                DepthMode::Recency => (info.last_modified - min_timestamp) as f32 / time_range,
                DepthMode::Coupling => coupling[i] / max_coupling,
                DepthMode::Importance => {
                    // Simple PageRank-like: commit_count normalized
                    let max_cc = graph.files.values().map(|f| f.commit_count).max().unwrap_or(1) as f32;
                    info.commit_count as f32 / max_cc
                }
            };
            let radius = (info.lines as f32).ln().max(0.5) * 0.15;
            let glow = (info.commit_count as f32).ln() / 5.0;
            let recency = (info.last_modified - min_timestamp) as f32 / time_range;
            let color = match color_mode {
                ColorMode::FileType => color_for_extension(id),
                ColorMode::Recency => color_for_recency(recency),
            };
            SceneNode {
                id: id.clone(),
                pos: positions[i],
                depth,
                radius,
                color,
                glow: glow.clamp(0.0, 1.0),
            }
        })
        .collect();

    let edges: Vec<SceneEdge> = graph
        .co_change
        .iter()
        .filter_map(|(a, b, score)| {
            let ia = id_to_idx.get(a.as_str())?;
            let ib = id_to_idx.get(b.as_str())?;
            Some(SceneEdge {
                from: positions[*ia],
                to: positions[*ib],
                opacity: *score,
            })
        })
        .collect();

    SceneGraph { nodes, edges }
}

pub async fn run(
    mut rx: mpsc::Receiver<(FileGraph, EmbeddingMap)>,
    scene_tx: mpsc::Sender<SceneGraph>,
    depth_mode: Arc<RwLock<DepthMode>>,
    color_mode: Arc<RwLock<ColorMode>>,
) {
    log::info!("LayoutEngine started");

    while let Some((graph, embed_map)) = rx.recv().await {
        let dm = *depth_mode.read().await;
        let cm = *color_mode.read().await;
        let scene = compute_layout(&graph, &embed_map, dm, cm, 200);
        log::info!("Layout: {} nodes, {} edges", scene.nodes.len(), scene.edges.len());
        if scene_tx.send(scene).await.is_err() {
            break;
        }
    }
}
```

- [ ] **Step 4: Update main.rs channel types**

Update main.rs to use the new channel signature: embedder sends `(FileGraph, EmbeddingMap)` to layout. Remove the placeholder channel. The layout `run` function now takes a single `mpsc::Receiver<(FileGraph, EmbeddingMap)>`.

- [ ] **Step 5: Run test**

Run: `cargo test --test layout_test`
Expected: PASS — connected files closer than unconnected.

- [ ] **Step 6: Commit**

```bash
git add src/ tests/
git commit -m "feat: force-directed layout engine with depth modes"
```

---

## Task 6: Node Rendering with wgpu Shaders

**Files:**
- Create: `assets/shaders/node.wgsl`
- Create: `src/render/camera.rs`
- Create: `src/render/node.rs`
- Modify: `src/render/state.rs`
- Modify: `src/render/mod.rs`

- [ ] **Step 1: Create node shader**

```wgsl
// assets/shaders/node.wgsl

struct Camera {
    view_proj: mat4x4<f32>,
};
@group(0) @binding(0) var<uniform> camera: Camera;

struct VertexInput {
    @location(0) position: vec2<f32>,  // quad vertex
};

struct InstanceInput {
    @location(1) center: vec2<f32>,
    @location(2) radius: f32,
    @location(3) depth: f32,
    @location(4) color: vec4<f32>,
    @location(5) glow: f32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) glow: f32,
};

@vertex
fn vs_main(vert: VertexInput, inst: InstanceInput) -> VertexOutput {
    var out: VertexOutput;
    let world_pos = vec4<f32>(
        inst.center + vert.position * inst.radius,
        inst.depth * 0.5,
        1.0
    );
    out.clip_position = camera.view_proj * world_pos;
    out.uv = vert.position;
    out.color = inst.color;
    out.glow = inst.glow;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let dist = length(in.uv);
    if dist > 1.0 {
        discard;
    }

    // Soft circle with radial gradient
    let core = smoothstep(0.8, 0.0, dist);
    let edge = smoothstep(1.0, 0.7, dist);

    // Base color with lighting
    let lit = in.color.rgb * (0.4 + 0.6 * core);

    // Glow halo
    let glow_strength = in.glow * smoothstep(1.0, 0.3, dist) * 0.5;
    let glow_color = in.color.rgb * glow_strength;

    let final_color = lit + glow_color;
    let alpha = edge;

    return vec4<f32>(final_color, alpha);
}
```

- [ ] **Step 2: Create camera.rs**

```rust
// src/render/camera.rs
use glam::{Mat4, Vec2, Vec3};

pub struct Camera {
    pub center: Vec2,
    pub zoom: f32,
    pub aspect: f32,
}

impl Camera {
    pub fn new(aspect: f32) -> Self {
        Self {
            center: Vec2::ZERO,
            zoom: 1.0,
            aspect,
        }
    }

    pub fn view_proj(&self) -> Mat4 {
        let half_w = 10.0 / self.zoom;
        let half_h = half_w / self.aspect;
        let view = Mat4::look_at_rh(
            Vec3::new(self.center.x, self.center.y, 10.0),
            Vec3::new(self.center.x, self.center.y, 0.0),
            Vec3::Y,
        );
        let proj = Mat4::orthographic_rh(-half_w, half_w, -half_h, half_h, 0.1, 100.0);
        proj * view
    }

    pub fn pan(&mut self, delta: Vec2) {
        self.center -= delta / self.zoom;
    }

    pub fn zoom_by(&mut self, factor: f32) {
        self.zoom = (self.zoom * factor).clamp(0.1, 50.0);
    }

    pub fn set_aspect(&mut self, aspect: f32) {
        self.aspect = aspect;
    }
}
```

- [ ] **Step 3: Create node.rs — instance buffer management**

```rust
// src/render/node.rs
use bytemuck::{Pod, Zeroable};
use crate::scene::SceneNode;

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct NodeVertex {
    pub position: [f32; 2],
}

impl NodeVertex {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x2,
            }],
        }
    }
}

// Unit quad vertices (will be scaled by instance radius in shader)
pub const QUAD_VERTICES: &[NodeVertex] = &[
    NodeVertex { position: [-1.0, -1.0] },
    NodeVertex { position: [1.0, -1.0] },
    NodeVertex { position: [1.0, 1.0] },
    NodeVertex { position: [-1.0, -1.0] },
    NodeVertex { position: [1.0, 1.0] },
    NodeVertex { position: [-1.0, 1.0] },
];

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct NodeInstance {
    pub center: [f32; 2],
    pub radius: f32,
    pub depth: f32,
    pub color: [f32; 4],
    pub glow: f32,
    pub _padding: [f32; 3], // align to 16 bytes
}

impl NodeInstance {
    pub fn from_scene_node(node: &SceneNode) -> Self {
        Self {
            center: node.pos.into(),
            radius: node.radius,
            depth: node.depth,
            color: node.color,
            glow: node.glow,
            _padding: [0.0; 3],
        }
    }

    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute { offset: 0, shader_location: 1, format: wgpu::VertexFormat::Float32x2 },   // center
                wgpu::VertexAttribute { offset: 8, shader_location: 2, format: wgpu::VertexFormat::Float32 },     // radius
                wgpu::VertexAttribute { offset: 12, shader_location: 3, format: wgpu::VertexFormat::Float32 },    // depth
                wgpu::VertexAttribute { offset: 16, shader_location: 4, format: wgpu::VertexFormat::Float32x4 },  // color
                wgpu::VertexAttribute { offset: 32, shader_location: 5, format: wgpu::VertexFormat::Float32 },    // glow
            ],
        }
    }
}
```

- [ ] **Step 4: Update render/state.rs to create pipeline and draw nodes**

Add to `RenderState`:
- Load `node.wgsl` shader from `assets/shaders/`
- Create render pipeline with vertex + instance buffers and camera uniform bind group
- Create camera uniform buffer
- Add `update_scene(&mut self, scene: &SceneGraph)` method that rebuilds the instance buffer
- Update `render()` to draw instanced quads

This is the largest single code block. Key points:
- Use `include_str!("../../assets/shaders/node.wgsl")` to embed shader at compile time
- Camera uniform is a single `Mat4` (64 bytes)
- Instance buffer recreated each frame the scene changes (not every frame)
- Enable alpha blending on the pipeline for glow transparency

- [ ] **Step 5: Update render/mod.rs exports**

```rust
pub mod state;
pub mod camera;
pub mod node;
pub use state::RenderState;
```

- [ ] **Step 6: Update app.rs to pass scene to renderer**

In `about_to_wait`, call `self.scene_rx.try_recv()`. If new scene arrives, call `render_state.update_scene(&scene)`. Store latest scene for hover/click later.

- [ ] **Step 7: Verify nodes render**

Run: `cargo run -- ~/Projects/ising-rs`
Expected: Dark window with colored glowing circles representing files. Circles cluster based on co-change patterns.

- [ ] **Step 8: Commit**

```bash
git add src/ assets/
git commit -m "feat: node rendering with instanced circles and glow shader"
```

---

## Task 7: Edge Rendering

**Files:**
- Create: `assets/shaders/edge.wgsl`
- Create: `src/render/edge.rs`
- Modify: `src/render/state.rs`

- [ ] **Step 1: Create edge shader**

```wgsl
// assets/shaders/edge.wgsl

struct Camera {
    view_proj: mat4x4<f32>,
};
@group(0) @binding(0) var<uniform> camera: Camera;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) opacity: f32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) opacity: f32,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = camera.view_proj * vec4<f32>(in.position, 0.0, 1.0);
    out.opacity = in.opacity;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(0.4, 0.45, 0.55, in.opacity * 0.4);
}
```

- [ ] **Step 2: Create edge.rs vertex type**

```rust
// src/render/edge.rs
use bytemuck::{Pod, Zeroable};
use crate::scene::SceneEdge;

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct EdgeVertex {
    pub position: [f32; 2],
    pub opacity: f32,
}

impl EdgeVertex {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute { offset: 0, shader_location: 0, format: wgpu::VertexFormat::Float32x2 },
                wgpu::VertexAttribute { offset: 8, shader_location: 1, format: wgpu::VertexFormat::Float32 },
            ],
        }
    }
}

pub fn edges_to_vertices(edges: &[SceneEdge]) -> Vec<EdgeVertex> {
    let mut verts = Vec::with_capacity(edges.len() * 2);
    for e in edges {
        verts.push(EdgeVertex { position: e.from.into(), opacity: e.opacity });
        verts.push(EdgeVertex { position: e.to.into(), opacity: e.opacity });
    }
    verts
}
```

- [ ] **Step 3: Add edge pipeline to state.rs**

Create a second render pipeline for edges using `PrimitiveTopology::LineList`. Draw edges before nodes so nodes render on top. Share the camera bind group between both pipelines.

- [ ] **Step 4: Verify edges render**

Run: `cargo run -- ~/Projects/ising-rs`
Expected: Faint lines connecting related files, with circles drawn on top.

- [ ] **Step 5: Commit**

```bash
git add src/ assets/
git commit -m "feat: edge rendering with opacity-based co-change strength"
```

---

## Task 8: Camera Controls (Pan, Zoom, Keyboard)

**Files:**
- Modify: `src/app.rs`
- Create: `src/ui.rs`

- [ ] **Step 1: Create ui.rs with input state**

```rust
// src/ui.rs
use glam::Vec2;
use winit::event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent};
use winit::keyboard::{Key, NamedKey};
use crate::scene::{ColorMode, DepthMode};

pub struct InputState {
    pub mouse_pos: Vec2,
    pub dragging: bool,
    pub last_mouse_pos: Vec2,
}

impl InputState {
    pub fn new() -> Self {
        Self {
            mouse_pos: Vec2::ZERO,
            dragging: false,
            last_mouse_pos: Vec2::ZERO,
        }
    }
}

pub enum UiAction {
    Pan(Vec2),
    Zoom(f32),
    SetDepthMode(DepthMode),
    CycleColorMode,
    ResetCamera,
    Deselect,
    None,
}

pub fn handle_event(event: &WindowEvent, state: &mut InputState) -> UiAction {
    match event {
        WindowEvent::MouseWheel { delta, .. } => {
            let scroll = match delta {
                MouseScrollDelta::LineDelta(_, y) => *y,
                MouseScrollDelta::PixelDelta(p) => p.y as f32 / 100.0,
            };
            UiAction::Zoom(1.0 + scroll * 0.1)
        }
        WindowEvent::CursorMoved { position, .. } => {
            let pos = Vec2::new(position.x as f32, position.y as f32);
            state.last_mouse_pos = state.mouse_pos;
            state.mouse_pos = pos;
            if state.dragging {
                let delta = pos - state.last_mouse_pos;
                UiAction::Pan(delta)
            } else {
                UiAction::None
            }
        }
        WindowEvent::MouseInput { state: btn_state, button: MouseButton::Left, .. } => {
            state.dragging = *btn_state == ElementState::Pressed;
            UiAction::None
        }
        WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Right, .. } => {
            UiAction::Deselect
        }
        WindowEvent::KeyboardInput { event: KeyEvent { logical_key, state: ElementState::Pressed, .. }, .. } => {
            match logical_key {
                Key::Character(c) if c.as_str() == "1" => UiAction::SetDepthMode(DepthMode::Recency),
                Key::Character(c) if c.as_str() == "2" => UiAction::SetDepthMode(DepthMode::Coupling),
                Key::Character(c) if c.as_str() == "3" => UiAction::SetDepthMode(DepthMode::Importance),
                Key::Character(c) if c.as_str() == "c" => UiAction::CycleColorMode,
                Key::Named(NamedKey::Escape) => UiAction::Deselect,
                Key::Named(NamedKey::Space) => UiAction::ResetCamera,
                _ => UiAction::None,
            }
        }
        _ => UiAction::None,
    }
}
```

- [ ] **Step 2: Wire ui.rs into app.rs**

In `window_event`, call `ui::handle_event` and apply the resulting `UiAction` to the camera and depth/color mode state. When depth mode changes, write the new value to the shared `Arc<RwLock<DepthMode>>` so the layout engine picks it up.

- [ ] **Step 3: Verify controls work**

Run: `cargo run -- ~/Projects/ising-rs`
Expected:
- Scroll to zoom in/out
- Click-drag to pan
- Press 1/2/3 to switch depth mode (triggers re-layout)
- Press Space to reset camera

- [ ] **Step 4: Commit**

```bash
git add src/
git commit -m "feat: camera controls — pan, zoom, keyboard shortcuts"
```

---

## Task 9: Hover, Selection, File Inspector & Smooth Animation

**Files:**
- Modify: `src/render/state.rs`
- Modify: `src/render/camera.rs`
- Modify: `src/app.rs`
- Modify: `src/ui.rs`

- [ ] **Step 1: Implement hit testing in ui.rs**

Add `hit_test(mouse_pos: Vec2, scene: &SceneGraph, camera: &Camera, window_size: Vec2) -> Option<&SceneNode>` that converts screen coordinates to world space and finds the nearest node within its radius.

- [ ] **Step 2: Add hover tooltip via window title**

For the prototype, show hovered file info in the window title bar (zero-dependency):

```rust
// In app.rs, when hover changes:
if let Some(node) = hovered {
    window.set_title(&format!("cviz — {} ({} lines, {} commits)",
        node.id, /* lines */, /* commits */));
} else {
    window.set_title(&format!("cviz — {}", self.repo_path.display()));
}
```

- [ ] **Step 3: Add selection with highlight/dim**

When a node is hovered, increase its glow in the instance buffer. When selected (clicked), highlight all connected nodes and dim others by reducing their alpha. Handle `UiAction::Deselect` (Escape / right-click) to clear selection.

- [ ] **Step 4: Add file inspector sidebar**

When a node is selected (clicked), print file details to stdout as a simple inspector panel. This avoids GUI text rendering complexity in the prototype while still surfacing the spec-required information:

```rust
// In app.rs, when a node is clicked:
println!("\n═══ {} ═══", node.id);
println!("  Lines: {}", info.lines);
println!("  Last modified: {}", format_timestamp(info.last_modified));
println!("  Commits: {}", info.commit_count);
println!("  Top co-changed files:");
for (other, score) in top_cochanged {
    println!("    {:.0}% — {}", score * 100.0, other);
}
```

The full sidebar GUI (rendered in-window) is deferred to v0.2 when text rendering is added.

- [ ] **Step 5: Add smooth camera interpolation**

Update `camera.rs` to support animated transitions:

```rust
// In camera.rs, add:
pub struct Camera {
    pub center: Vec2,
    pub zoom: f32,
    pub aspect: f32,
    // Animation targets
    target_center: Vec2,
    target_zoom: f32,
}

impl Camera {
    /// Call every frame to interpolate toward targets.
    pub fn update(&mut self, dt: f32) {
        let lerp_speed = 8.0 * dt; // smooth ~8fps equivalent
        self.center = self.center.lerp(self.target_center, lerp_speed.min(1.0));
        self.zoom = self.zoom + (self.target_zoom - self.zoom) * lerp_speed.min(1.0);
    }

    pub fn pan(&mut self, delta: Vec2) {
        self.target_center -= delta / self.zoom;
    }

    pub fn zoom_by(&mut self, factor: f32) {
        self.target_zoom = (self.target_zoom * factor).clamp(0.1, 50.0);
    }

    pub fn reset(&mut self) {
        self.target_center = Vec2::ZERO;
        self.target_zoom = 1.0;
    }
}
```

Call `camera.update(dt)` in `about_to_wait` before rendering. Compute `dt` from `std::time::Instant` between frames.

- [ ] **Step 6: Add smooth node position interpolation**

In `RenderState`, when a new `SceneGraph` arrives, don't replace positions instantly. Instead, store both current and target positions, and lerp each frame:

```rust
// In render/state.rs:
pub fn update_scene(&mut self, new_scene: &SceneGraph) {
    // Store target positions from new scene
    self.target_instances = new_scene.nodes.iter().map(NodeInstance::from_scene_node).collect();
    // If first scene, snap immediately
    if self.current_instances.is_empty() {
        self.current_instances = self.target_instances.clone();
    }
}

pub fn animate(&mut self, dt: f32) {
    let speed = 5.0 * dt;
    for (current, target) in self.current_instances.iter_mut().zip(&self.target_instances) {
        current.center[0] += (target.center[0] - current.center[0]) * speed.min(1.0);
        current.center[1] += (target.center[1] - current.center[1]) * speed.min(1.0);
        // Lerp color channels for smooth mode transitions
        for i in 0..4 {
            current.color[i] += (target.color[i] - current.color[i]) * speed.min(1.0);
        }
    }
    // Rebuild instance buffer from current_instances
}
```

- [ ] **Step 7: Verify hover, selection, inspector, and animation work**

Run: `cargo run -- ~/Projects/ising-rs`
Expected:
- Hovering shows filename in title bar
- Clicking prints file details to terminal and highlights connected nodes
- Escape / right-click deselects
- Switching depth/color mode animates nodes smoothly to new positions/colors
- Camera pan/zoom interpolates smoothly

- [ ] **Step 8: Commit**

```bash
git add src/
git commit -m "feat: hover, selection, file inspector, smooth animation"
```

---

## Task 10: Integration Test on Real Repos

**Files:**
- No new files — manual verification

- [ ] **Step 1: Test on ising-rs**

Run: `cargo run --release -- ~/Projects/ising-rs`
Expected: ~20-30 file nodes. Bin files should cluster separately from lib files. Test files should group together. Verify depth modes work.

- [ ] **Step 2: Test on physics-llm-research**

Run: `cargo run --release -- ~/Projects/physics-llm-research`
Expected: ~15 Python scripts + results. Experiment scripts should cluster. The CLAUDE.md and README should be separate from code.

- [ ] **Step 3: Test on portfolio**

Run: `cargo run --release -- ~/Projects/portfolio`
Expected: Larger repo. Astro components should cluster. Content files separate from components.

- [ ] **Step 4: Performance check**

For the largest repo, check that:
- Initial load completes in < 5 seconds
- Render stays at 60fps (no jank during pan/zoom)
- Incremental updates don't cause visible stutter

- [ ] **Step 5: Fix any issues found**

Address bugs or layout quality issues discovered during testing.

- [ ] **Step 6: Commit any fixes**

```bash
git add -A
git commit -m "fix: integration testing fixes"
```

---

## Task 11: README & Final Polish

**Files:**
- Create: `README.md`

- [ ] **Step 1: Write README**

```markdown
# cviz — 2.5D Codebase Visualizer

Visualize any git repository as an interactive 2.5D map.
Files are positioned by co-change frequency and semantic similarity.
Depth, color, and glow encode configurable metrics.

## Usage

\`\`\`bash
cargo run --release -- /path/to/git/repo
\`\`\`

## Controls

| Key | Action |
|-----|--------|
| Scroll | Zoom in/out |
| Click + drag | Pan |
| 1 / 2 / 3 | Depth mode: recency / coupling / importance |
| c | Cycle color mode |
| Space | Reset camera |

## Requirements

- Rust 1.94+
- A git repository to visualize
```

- [ ] **Step 2: Commit**

```bash
git add README.md
git commit -m "docs: add README with usage and controls"
```

- [ ] **Step 3: Create GitHub repo and push**

```bash
gh repo create codebase-visualizer --public --source=. --push
```

---
