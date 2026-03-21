// src/scene.rs
use glam::Vec2;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

pub type FileId = String;

#[derive(Debug, Clone)]
pub struct FileGraph {
    pub files: HashMap<FileId, FileInfo>,
    pub co_change: Vec<(FileId, FileId, f32)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub path: PathBuf,
    pub lines: usize,
    pub last_modified: i64,
    pub commit_count: usize,
}

#[derive(Debug, Clone)]
pub struct EmbeddingMap {
    pub embeddings: HashMap<FileId, Vec<f32>>,
}

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
    pub directory: String,
}

#[derive(Debug, Clone)]
pub struct SceneEdge {
    pub from: Vec2,
    pub to: Vec2,
    pub opacity: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DepthMode {
    Recency,
    Coupling,
    Importance,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ColorMode {
    FileType,
    Recency,
}
