use crate::scene::{EmbeddingMap, FileGraph};
use std::path::PathBuf;
use tokio::sync::mpsc;

pub async fn run(repo_path: PathBuf, rx: mpsc::Receiver<FileGraph>, tx: mpsc::Sender<(FileGraph, EmbeddingMap)>) {
    log::info!("Embedder started");
}
