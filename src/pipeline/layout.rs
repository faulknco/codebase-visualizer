use crate::scene::*;
use glam::Vec2;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

fn embedding_distance(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() { return 1.0; }
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let mag_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let mag_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if mag_a == 0.0 || mag_b == 0.0 { 1.0 } else { 1.0 - (dot / (mag_a * mag_b)) }
}

fn color_for_extension(path: &str) -> [f32; 4] {
    if path.ends_with(".rs") { [0.97, 0.44, 0.44, 1.0] }
    else if path.ends_with(".toml") { [0.98, 0.75, 0.14, 1.0] }
    else if path.ends_with(".md") { [0.20, 0.83, 0.60, 1.0] }
    else if path.contains("test") { [0.51, 0.55, 0.97, 1.0] }
    else { [0.58, 0.64, 0.72, 1.0] }
}

fn color_for_recency(normalized_recency: f32) -> [f32; 4] {
    let t = normalized_recency.clamp(0.0, 1.0);
    if t < 0.5 {
        let s = t * 2.0;
        [0.12 * (1.0 - s), 0.23 + 0.60 * s, 0.37 * (1.0 - s) + 0.40 * s, 1.0]
    } else {
        let s = (t - 0.5) * 2.0;
        [0.12 + 0.85 * s, 0.83 * (1.0 - s) + 0.27 * s, 0.40 * (1.0 - s), 1.0]
    }
}

pub fn compute_layout(
    graph: &FileGraph,
    embed_map: &EmbeddingMap,
    depth_mode: DepthMode,
    color_mode: ColorMode,
    iterations: usize,
) -> SceneGraph {
    let ids: Vec<FileId> = graph.files.keys().cloned().collect();
    let n = ids.len();
    if n == 0 { return SceneGraph { nodes: vec![], edges: vec![] }; }

    let id_to_idx: HashMap<&str, usize> = ids.iter().enumerate().map(|(i, id)| (id.as_str(), i)).collect();

    let mut positions: Vec<Vec2> = (0..n)
        .map(|i| {
            let angle = (i as f32 / n as f32) * std::f32::consts::TAU;
            Vec2::new(angle.cos() * 5.0, angle.sin() * 5.0)
        })
        .collect();

    let mut co_change_map: HashMap<(usize, usize), f32> = HashMap::new();
    for (a, b, score) in &graph.co_change {
        if let (Some(&ia), Some(&ib)) = (id_to_idx.get(a.as_str()), id_to_idx.get(b.as_str())) {
            let key = if ia < ib { (ia, ib) } else { (ib, ia) };
            co_change_map.insert(key, *score);
        }
    }

    let repulsion = 2.0;
    let attraction = 0.5;
    let damping = 0.95;
    let mut velocities = vec![Vec2::ZERO; n];

    for _ in 0..iterations {
        let mut forces = vec![Vec2::ZERO; n];

        for i in 0..n {
            for j in (i + 1)..n {
                let diff = positions[i] - positions[j];
                let dist = diff.length().max(0.01);
                let force = diff.normalize() * repulsion / (dist * dist);
                forces[i] += force;
                forces[j] -= force;
            }
        }

        for (&(i, j), &score) in &co_change_map {
            let diff = positions[j] - positions[i];
            let dist = diff.length();
            let force = diff.normalize() * attraction * score * dist;
            forces[i] += force;
            forces[j] -= force;
        }

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

        for i in 0..n {
            velocities[i] = (velocities[i] + forces[i] * 0.01) * damping;
            positions[i] += velocities[i];
        }
    }

    let max_timestamp = graph.files.values().map(|f| f.last_modified).max().unwrap_or(1);
    let min_timestamp = graph.files.values().map(|f| f.last_modified).min().unwrap_or(0);
    let time_range = (max_timestamp - min_timestamp).max(1) as f32;

    let mut coupling: Vec<f32> = vec![0.0; n];
    for &(i, j) in co_change_map.keys() {
        coupling[i] += 1.0;
        coupling[j] += 1.0;
    }
    let max_coupling = coupling.iter().cloned().fold(0.0f32, f32::max).max(1.0);

    let nodes: Vec<SceneNode> = ids.iter().enumerate().map(|(i, id)| {
        let info = &graph.files[id];
        let depth = match depth_mode {
            DepthMode::Recency => (info.last_modified - min_timestamp) as f32 / time_range,
            DepthMode::Coupling => coupling[i] / max_coupling,
            DepthMode::Importance => {
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
        SceneNode { id: id.clone(), pos: positions[i], depth, radius, color, glow: glow.clamp(0.0, 1.0) }
    }).collect();

    let edges: Vec<SceneEdge> = graph.co_change.iter().filter_map(|(a, b, score)| {
        let ia = id_to_idx.get(a.as_str())?;
        let ib = id_to_idx.get(b.as_str())?;
        Some(SceneEdge { from: positions[*ia], to: positions[*ib], opacity: *score })
    }).collect();

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
        if scene_tx.send(scene).await.is_err() { break; }
    }
}
