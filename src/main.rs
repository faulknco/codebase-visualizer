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

    let (git_tx, git_rx) = mpsc::channel::<scene::FileGraph>(4);
    let (embed_tx, embed_rx) = mpsc::channel::<(scene::FileGraph, scene::EmbeddingMap)>(4);
    let (scene_tx, scene_rx) = mpsc::channel::<scene::SceneGraph>(4);
    let depth_mode = Arc::new(RwLock::new(DepthMode::Recency));
    let color_mode = Arc::new(RwLock::new(scene::ColorMode::FileType));

    let repo_path = cli.repo.clone();
    let embed_repo_path = cli.repo.clone();
    let dm = depth_mode.clone();
    let cm = color_mode.clone();

    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
        rt.block_on(async move {
            tokio::spawn(pipeline::git_collector::run(repo_path, git_tx));
            tokio::spawn(pipeline::embedder::run(embed_repo_path, git_rx, embed_tx));
            tokio::spawn(pipeline::layout::run(embed_rx, scene_tx, dm, cm));
            std::future::pending::<()>().await;
        });
    });

    let event_loop = winit::event_loop::EventLoop::new().expect("Failed to create event loop");
    let mut app = app::App::new(cli.repo, scene_rx, depth_mode, color_mode);
    event_loop.run_app(&mut app).expect("Event loop error");
}
