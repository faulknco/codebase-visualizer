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
}
