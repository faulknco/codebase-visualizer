use bytemuck::{Pod, Zeroable};

use crate::scene::SceneNode;

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct NodeVertex {
    pub position: [f32; 2],
}

impl NodeVertex {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x2,
            }],
        }
    }
}

pub const QUAD_VERTICES: &[NodeVertex] = &[
    NodeVertex {
        position: [-1.0, -1.0],
    },
    NodeVertex {
        position: [1.0, -1.0],
    },
    NodeVertex {
        position: [1.0, 1.0],
    },
    NodeVertex {
        position: [-1.0, -1.0],
    },
    NodeVertex {
        position: [1.0, 1.0],
    },
    NodeVertex {
        position: [-1.0, 1.0],
    },
];

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct NodeInstance {
    pub center: [f32; 2],
    pub radius: f32,
    pub depth: f32,
    pub color: [f32; 4],
    pub glow: f32,
    pub activity: f32,
    pub _padding: [f32; 2],
}

impl NodeInstance {
    pub fn from_scene_node(node: &SceneNode) -> Self {
        Self {
            center: node.pos.into(),
            radius: node.radius,
            depth: node.depth,
            color: node.color,
            glow: node.glow,
            activity: 0.0,
            _padding: [0.0; 2],
        }
    }

    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: 8,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32,
                },
                wgpu::VertexAttribute {
                    offset: 12,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32,
                },
                wgpu::VertexAttribute {
                    offset: 16,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: 32,
                    shader_location: 5,
                    format: wgpu::VertexFormat::Float32,
                },
                wgpu::VertexAttribute {
                    offset: 36,
                    shader_location: 6,
                    format: wgpu::VertexFormat::Float32,
                },
            ],
        }
    }
}
