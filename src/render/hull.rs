// src/render/hull.rs
use bytemuck::{Pod, Zeroable};
use glam::Vec2;
use std::collections::HashMap;
use wgpu::VertexBufferLayout;

use crate::scene::SceneNode;

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct HullVertex {
    pub position: [f32; 2],
    pub color: [f32; 4],
}

impl HullVertex {
    pub fn desc() -> VertexBufferLayout<'static> {
        use std::mem;
        VertexBufferLayout {
            array_stride: mem::size_of::<HullVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

fn group_by_directory(nodes: &[SceneNode]) -> HashMap<String, Vec<Vec2>> {
    let mut map: HashMap<String, Vec<Vec2>> = HashMap::new();
    for node in nodes {
        if node.directory.is_empty() {
            continue;
        }
        map.entry(node.directory.clone())
            .or_default()
            .push(node.pos);
    }
    map
}

/// Graham scan convex hull. Returns hull vertices in CCW order.
/// Input may be modified (sorted). Returns empty if < 3 points.
fn convex_hull(points: &mut Vec<Vec2>) -> Vec<Vec2> {
    let n = points.len();
    if n < 3 {
        return vec![];
    }

    // Find pivot: lowest y, then leftmost x
    let pivot_idx = points
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            a.y.partial_cmp(&b.y)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal))
        })
        .map(|(i, _)| i)
        .unwrap_or(0);

    points.swap(0, pivot_idx);
    let pivot = points[0];

    // Sort remaining by polar angle relative to pivot
    let rest = &mut points[1..];
    rest.sort_by(|a, b| {
        let da = *a - pivot;
        let db = *b - pivot;
        let cross = da.x * db.y - da.y * db.x;
        if cross.abs() < 1e-9 {
            // Same angle: closer first
            da.length_squared()
                .partial_cmp(&db.length_squared())
                .unwrap_or(std::cmp::Ordering::Equal)
        } else if cross > 0.0 {
            std::cmp::Ordering::Less // a before b (CCW)
        } else {
            std::cmp::Ordering::Greater
        }
    });

    // Graham scan
    let mut stack: Vec<Vec2> = Vec::with_capacity(n);
    for &p in points.iter() {
        while stack.len() >= 2 {
            let a = stack[stack.len() - 2];
            let b = stack[stack.len() - 1];
            let cross = (b - a).x * (p - a).y - (b - a).y * (p - a).x;
            if cross <= 0.0 {
                stack.pop();
            } else {
                break;
            }
        }
        stack.push(p);
    }

    if stack.len() < 3 {
        return vec![];
    }

    stack
}

/// Hash a directory name to a hue in [0, 1).
fn dir_to_hue(dir: &str) -> f32 {
    let hash: u32 = dir
        .bytes()
        .fold(2166136261u32, |acc, b| acc.wrapping_mul(16777619).wrapping_add(b as u32));
    (hash % 360) as f32 / 360.0
}

/// HSV (s=0.4, v=0.6) -> RGB, with given alpha.
fn hsv_to_rgba(hue: f32, alpha: f32) -> [f32; 4] {
    let s = 0.4f32;
    let v = 0.6f32;
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
    [r + m, g + m, b + m, alpha]
}

pub fn build_hull_geometry(nodes: &[SceneNode]) -> Vec<HullVertex> {
    let groups = group_by_directory(nodes);
    let mut vertices = Vec::new();

    for (dir, mut pts) in groups {
        if pts.len() < 3 {
            continue;
        }

        let hull = convex_hull(&mut pts);
        if hull.len() < 3 {
            continue;
        }

        // Expand hull outward from centroid by 1.5 units
        let centroid = hull.iter().fold(Vec2::ZERO, |acc, &p| acc + p) / hull.len() as f32;
        let expanded: Vec<Vec2> = hull
            .iter()
            .map(|&p| {
                let dir_vec = p - centroid;
                let len = dir_vec.length();
                if len < 1e-6 {
                    p
                } else {
                    p + dir_vec.normalize() * 1.5
                }
            })
            .collect();

        let hue = dir_to_hue(&dir);
        let color = hsv_to_rgba(hue, 0.07);

        // Fan-triangulate from centroid
        let centroid_vertex = HullVertex {
            position: [centroid.x, centroid.y],
            color,
        };

        let m = expanded.len();
        for i in 0..m {
            let a = expanded[i];
            let b = expanded[(i + 1) % m];
            vertices.push(centroid_vertex);
            vertices.push(HullVertex { position: [a.x, a.y], color });
            vertices.push(HullVertex { position: [b.x, b.y], color });
        }
    }

    vertices
}
