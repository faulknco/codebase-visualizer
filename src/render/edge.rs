use bytemuck::{Pod, Zeroable};
use crate::scene::SceneEdge;

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable, PartialEq)]
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
