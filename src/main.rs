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
