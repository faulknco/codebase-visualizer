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
    // By file extension
    if path.ends_with(".rs") { return [0.97, 0.44, 0.44, 1.0]; }       // red — Rust
    if path.ends_with(".toml") { return [0.98, 0.75, 0.14, 1.0]; }     // amber — config
    if path.ends_with(".md") { return [0.20, 0.83, 0.60, 1.0]; }       // green — docs
    if path.ends_with(".py") { return [0.36, 0.65, 0.96, 1.0]; }       // blue — Python
    if path.ends_with(".js") || path.ends_with(".ts") || path.ends_with(".tsx") || path.ends_with(".jsx") {
        return [0.96, 0.87, 0.25, 1.0];                                 // yellow — JS/TS
    }
    if path.ends_with(".wgsl") || path.ends_with(".glsl") || path.ends_with(".hlsl") {
        return [0.85, 0.45, 0.95, 1.0];                                 // purple — shaders
    }
    if path.ends_with(".json") || path.ends_with(".yaml") || path.ends_with(".yml") || path.ends_with(".lock") {
        return [0.75, 0.65, 0.45, 1.0];                                 // tan — data/config
    }
    if path.ends_with(".sh") || path.ends_with(".bash") || path.ends_with(".zsh") {
        return [0.55, 0.85, 0.55, 1.0];                                 // light green — scripts
    }
    if path.ends_with(".css") || path.ends_with(".scss") {
        return [0.95, 0.55, 0.75, 1.0];                                 // pink — styles
    }
    if path.ends_with(".html") || path.ends_with(".astro") || path.ends_with(".svelte") {
        return [0.95, 0.60, 0.35, 1.0];                                 // orange — templates
    }
    if path.contains("test") || path.contains("spec") {
        return [0.51, 0.55, 0.97, 1.0];                                 // indigo — tests
    }

    // For unknown types, color by parent directory (hashed) so files in the same folder cluster visually
    let dir = path.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
    if dir.is_empty() {
        return [0.58, 0.64, 0.72, 1.0]; // root files — slate
    }
    // Simple hash to pick a hue
    let hash: u32 = dir.bytes().fold(0u32, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u32));
    let hue = (hash % 360) as f32 / 360.0;
    // HSV to RGB (s=0.5, v=0.8 for muted but visible)
    let s = 0.5f32;
    let v = 0.8f32;
    let h = hue * 6.0;
    let c = v * s;
    let x = c * (1.0 - ((h % 2.0) - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match h as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    [r + m, g + m, b + m, 1.0]
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

// ---------------------------------------------------------------------------
// Barnes-Hut quadtree for O(n log n) repulsion
// ---------------------------------------------------------------------------

const MAX_DEPTH: u32 = 20;

struct QuadTree {
    min: Vec2,
    max: Vec2,
    center_of_mass: Vec2,
    total_mass: f32,
    // Leaf: Some(index), Internal: None
    node_index: Option<usize>,
    // Children: [NW, NE, SW, SE]
    children: Option<Box<[QuadTree; 4]>>,
}

impl QuadTree {
    fn new(min: Vec2, max: Vec2) -> Self {
        QuadTree {
            min,
            max,
            center_of_mass: Vec2::ZERO,
            total_mass: 0.0,
            node_index: None,
            children: None,
        }
    }

    fn is_leaf(&self) -> bool {
        self.children.is_none()
    }

    fn cell_size(&self) -> f32 {
        (self.max - self.min).max_element()
    }

    fn mid(&self) -> Vec2 {
        (self.min + self.max) * 0.5
    }

    fn quadrant_bounds(&self, q: usize) -> (Vec2, Vec2) {
        let mid = self.mid();
        match q {
            0 => (Vec2::new(self.min.x, mid.y),  Vec2::new(mid.x,      self.max.y)), // NW
            1 => (mid,                             self.max),                          // NE
            2 => (self.min,                        mid),                               // SW
            _ => (Vec2::new(mid.x, self.min.y),   Vec2::new(self.max.x, mid.y)),     // SE
        }
    }

    fn quadrant_for(&self, pos: Vec2) -> usize {
        let mid = self.mid();
        match (pos.x >= mid.x, pos.y >= mid.y) {
            (false, true)  => 0, // NW
            (true,  true)  => 1, // NE
            (false, false) => 2, // SW
            (true,  false) => 3, // SE
        }
    }

    fn insert(&mut self, pos: Vec2, idx: usize, depth: u32) {
        // Update center of mass incrementally (all masses = 1.0)
        self.center_of_mass = (self.center_of_mass * self.total_mass + pos) / (self.total_mass + 1.0);
        self.total_mass += 1.0;

        if self.total_mass == 1.0 {
            // First node in this cell — store as leaf
            self.node_index = Some(idx);
            return;
        }

        // At max depth, just accumulate mass without subdividing further
        if depth >= MAX_DEPTH {
            self.node_index = None;
            return;
        }

        // Promote existing leaf into children if needed
        if self.is_leaf() {
            // Subdivide: move the existing leaf node down
            let (b0_min, b0_max) = self.quadrant_bounds(0);
            let (b1_min, b1_max) = self.quadrant_bounds(1);
            let (b2_min, b2_max) = self.quadrant_bounds(2);
            let (b3_min, b3_max) = self.quadrant_bounds(3);
            self.children = Some(Box::new([
                QuadTree::new(b0_min, b0_max),
                QuadTree::new(b1_min, b1_max),
                QuadTree::new(b2_min, b2_max),
                QuadTree::new(b3_min, b3_max),
            ]));

            if let Some(old_idx) = self.node_index.take() {
                // We need the old position — but we only store center_of_mass.
                // Before we updated it above, the old single node's position was
                // the previous center_of_mass. Recover it:
                // new_com = (old_com * (total-1) + pos) / total  =>  old_com = (new_com * total - pos) / (total-1)
                // total is already incremented, so old total = total_mass - 1 (before this call it was 1)
                // At this point total_mass = 2 (we just added the second node above).
                // old single-node position = previous center_of_mass before update
                // = (current_com * 2 - pos) / 1  = current_com*2 - pos
                let old_pos = self.center_of_mass * 2.0 - pos;
                let q = self.quadrant_for(old_pos);
                if let Some(ref mut ch) = self.children {
                    ch[q].insert(old_pos, old_idx, depth + 1);
                }
            }
        }

        // Insert new node into appropriate child
        let q = self.quadrant_for(pos);
        if let Some(ref mut ch) = self.children {
            ch[q].insert(pos, idx, depth + 1);
        }
    }
}

fn build_quadtree(positions: &[Vec2]) -> QuadTree {
    if positions.is_empty() {
        return QuadTree::new(Vec2::ZERO, Vec2::ONE);
    }

    // Compute bounding box with a small margin
    let mut min = positions[0];
    let mut max = positions[0];
    for &p in positions.iter() {
        min = min.min(p);
        max = max.max(p);
    }
    // Ensure non-degenerate bounds (all nodes at same position)
    let size = (max - min).max_element();
    if size < 1e-6 {
        let center = (min + max) * 0.5;
        min = center - Vec2::splat(1.0);
        max = center + Vec2::splat(1.0);
    } else {
        let margin = size * 0.01;
        min -= Vec2::splat(margin);
        max += Vec2::splat(margin);
    }

    // Make it square to keep quadrants balanced
    let span = (max - min).max_element();
    let center = (min + max) * 0.5;
    min = center - Vec2::splat(span * 0.5);
    max = center + Vec2::splat(span * 0.5);

    let mut tree = QuadTree::new(min, max);
    for (i, &pos) in positions.iter().enumerate() {
        tree.insert(pos, i, 0);
    }
    tree
}

/// Accumulate Barnes-Hut repulsion force on a particle at `pos`.
/// theta: opening angle criterion (0.7 is standard).
fn barnes_hut_force(tree: &QuadTree, pos: Vec2, theta: f32, repulsion: f32) -> Vec2 {
    if tree.total_mass == 0.0 {
        return Vec2::ZERO;
    }

    let diff = pos - tree.center_of_mass;
    let dist = diff.length().max(0.01); // prevent division by zero / explosion

    // Avoid self-interaction: if this is a leaf at essentially the same position
    if tree.is_leaf() && tree.total_mass <= 1.0 && dist < 0.02 {
        return Vec2::ZERO;
    }

    let cell_size = tree.cell_size();

    // Barnes-Hut criterion: treat cell as a single body if far enough away
    if tree.is_leaf() || (cell_size / dist < theta) {
        let force_mag = (repulsion * tree.total_mass / (dist * dist)).min(100.0); // cap force
        return diff / dist * force_mag; // manual normalize to avoid NaN
    }

    // Otherwise recurse into children
    match &tree.children {
        Some(ch) => ch.iter().map(|c| barnes_hut_force(c, pos, theta, repulsion)).sum(),
        None => Vec2::ZERO,
    }
}

// ---------------------------------------------------------------------------

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
            let spread = (n as f32).sqrt() * 2.0;
            Vec2::new(angle.cos() * spread, angle.sin() * spread)
        })
        .collect();

    let mut co_change_map: HashMap<(usize, usize), f32> = HashMap::new();
    for (a, b, score) in &graph.co_change {
        if let (Some(&ia), Some(&ib)) = (id_to_idx.get(a.as_str()), id_to_idx.get(b.as_str())) {
            let key = if ia < ib { (ia, ib) } else { (ib, ia) };
            co_change_map.insert(key, *score);
        }
    }

    // Scale repulsion with node count so large graphs spread out
    let repulsion = 2.0 + (n as f32).sqrt();
    let attraction = 0.15;
    let damping = 0.9;
    let mut velocities = vec![Vec2::ZERO; n];

    // Barnes-Hut opening-angle criterion
    let theta = 0.7_f32;

    for _ in 0..iterations {
        let mut forces = vec![Vec2::ZERO; n];

        // ── Barnes-Hut O(n log n) repulsion ──────────────────────────────
        if n > 1 {
            let tree = build_quadtree(&positions);
            for i in 0..n {
                forces[i] += barnes_hut_force(&tree, positions[i], theta, repulsion);
            }
        }

        for (&(i, j), &score) in &co_change_map {
            let diff = positions[j] - positions[i];
            let dist = diff.length();
            let force = diff.normalize() * attraction * score * dist;
            forces[i] += force;
            forces[j] -= force;
        }

        // Embedding similarity attraction — skip for large graphs (O(n²) is too expensive)
        if n <= 100 {
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
        }

        for i in 0..n {
            velocities[i] = (velocities[i] + forces[i] * 0.01) * damping;
            // Cap velocity to prevent explosion
            let speed = velocities[i].length();
            if speed > 5.0 {
                velocities[i] = velocities[i] / speed * 5.0;
            }
            positions[i] += velocities[i];
            // Sanitize NaN (shouldn't happen but safety net)
            if positions[i].x.is_nan() || positions[i].y.is_nan() {
                positions[i] = Vec2::ZERO;
                velocities[i] = Vec2::ZERO;
            }
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
        let radius = (info.lines as f32 + 1.0).ln().max(1.0) * 0.3;
        let glow = (info.commit_count as f32).ln() / 5.0;
        let recency = (info.last_modified - min_timestamp) as f32 / time_range;
        let color = match color_mode {
            ColorMode::FileType => color_for_extension(id),
            ColorMode::Recency => color_for_recency(recency),
        };
        let directory = if let Some(idx) = id.find('/') { id[..idx].to_string() } else { String::new() };
        SceneNode { id: id.clone(), pos: positions[i], depth, radius, color, glow: glow.clamp(0.0, 1.0), directory }
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
    eprintln!("[cviz] LayoutEngine started");

    let mut last_file_count: usize = 0;
    let mut cached_scene: Option<SceneGraph> = None;
    let mut last_dm = DepthMode::Recency;
    let mut last_cm = ColorMode::FileType;

    while let Some((graph, embed_map)) = rx.recv().await {
        let dm = *depth_mode.read().await;
        let cm = *color_mode.read().await;

        // Only recompute layout if file count changed or modes changed
        let files_changed = graph.files.len() != last_file_count;
        let mode_changed = dm != last_dm || cm != last_cm;

        if files_changed || mode_changed || cached_scene.is_none() {
            eprintln!("[cviz] Layout computing for {} files...", graph.files.len());
            let iters = if graph.files.len() > 500 { 500 } else if graph.files.len() > 200 { 500 } else { 200 };
            let scene = compute_layout(&graph, &embed_map, dm, cm, iters);
            eprintln!("[cviz] Layout: {} nodes, {} edges", scene.nodes.len(), scene.edges.len());
            last_file_count = graph.files.len();
            last_dm = dm;
            last_cm = cm;
            cached_scene = Some(scene.clone());
            if scene_tx.send(scene).await.is_err() { break; }
        }
    }
}
