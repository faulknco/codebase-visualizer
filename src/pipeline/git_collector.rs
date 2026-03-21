use crate::scene::FileGraph;
use std::path::PathBuf;
use tokio::sync::mpsc;

pub async fn run(repo_path: PathBuf, tx: mpsc::Sender<FileGraph>) {
    log::info!("GitCollector started for {}", repo_path.display());
}
