// src/render/state.rs
use std::sync::Arc;
use wgpu::util::DeviceExt;
use winit::dpi::PhysicalSize;
use winit::window::Window;

use super::camera::Camera;
use super::edge::{EdgeVertex, edges_to_vertices};
use super::hull::{HullVertex, build_hull_geometry};
use super::label::{LabelVertex, build_label_quads, create_font_texture};
use super::node::{NodeInstance, NodeVertex, QUAD_VERTICES};
use crate::scene::SceneGraph;

pub struct RenderState {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    node_pipeline: wgpu::RenderPipeline,
    edge_pipeline: wgpu::RenderPipeline,
    hull_pipeline: wgpu::RenderPipeline,
    camera_bind_group: wgpu::BindGroup,
    camera_buffer: wgpu::Buffer,
    vertex_buffer: wgpu::Buffer,
    instance_buffer: Option<wgpu::Buffer>,
    instance_count: u32,
    edge_vertex_buffer: Option<wgpu::Buffer>,
    edge_vertex_count: u32,
    hull_vertex_buffer: Option<wgpu::Buffer>,
    hull_vertex_count: u32,
    label_pipeline: wgpu::RenderPipeline,
    label_vertex_buffer: Option<wgpu::Buffer>,
    label_vertex_count: u32,
    font_bind_group: wgpu::BindGroup,
    camera_bind_group_layout: wgpu::BindGroupLayout,
    current_instances: Vec<NodeInstance>,
    target_instances: Vec<NodeInstance>,
    current_edge_verts: Vec<EdgeVertex>,
    target_edge_verts: Vec<EdgeVertex>,
    pub show_edges: bool,
    pub current_zoom: f32,
}

fn lerp_f32(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn lerp_color(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    [
        lerp_f32(a[0], b[0], t),
        lerp_f32(a[1], b[1], t),
        lerp_f32(a[2], b[2], t),
        lerp_f32(a[3], b[3], t),
    ]
}

impl RenderState {
    pub async fn new(window: Arc<Window>) -> Self {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });

        let surface = instance
            .create_surface(Arc::clone(&window))
            .expect("Failed to create surface");

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .expect("Failed to find adapter");

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("cviz device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: Default::default(),
                trace: Default::default(),
                experimental_features: Default::default(),
            })
            .await
            .expect("Failed to create device");

        let size = window.inner_size();
        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        // Camera uniform buffer (64 bytes = Mat4)
        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("camera uniform"),
            contents: bytemuck::cast_slice(&[0.0f32; 16]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("camera bind group layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("camera bind group"),
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("shared pipeline layout"),
            bind_group_layouts: &[&camera_bind_group_layout],
            immediate_size: 0,
        });

        // Node shader
        let node_shader_src = include_str!("../../assets/shaders/node.wgsl");
        let node_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("node shader"),
            source: wgpu::ShaderSource::Wgsl(node_shader_src.into()),
        });

        let node_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("node pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &node_shader,
                entry_point: Some("vs_main"),
                buffers: &[NodeVertex::desc(), NodeInstance::desc()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &node_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        // Edge shader
        let edge_shader_src = include_str!("../../assets/shaders/edge.wgsl");
        let edge_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("edge shader"),
            source: wgpu::ShaderSource::Wgsl(edge_shader_src.into()),
        });

        let edge_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("edge pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &edge_shader,
                entry_point: Some("vs_main"),
                buffers: &[EdgeVertex::desc()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &edge_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::LineList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        // Hull shader
        let hull_shader_src = include_str!("../../assets/shaders/hull.wgsl");
        let hull_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("hull shader"),
            source: wgpu::ShaderSource::Wgsl(hull_shader_src.into()),
        });

        let hull_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("hull pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &hull_shader,
                entry_point: Some("vs_main"),
                buffers: &[HullVertex::desc()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &hull_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        // --- Label pipeline (2 bind groups: camera + font texture) ---
        let (_font_texture, font_texture_view) = create_font_texture(&device, &queue);

        let font_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("font bind group layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let font_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("font sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let font_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("font bind group"),
            layout: &font_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&font_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&font_sampler),
                },
            ],
        });

        let label_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("label pipeline layout"),
                bind_group_layouts: &[&camera_bind_group_layout, &font_bind_group_layout],
                immediate_size: 0,
            });

        let label_shader_src = include_str!("../../assets/shaders/label.wgsl");
        let label_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("label shader"),
            source: wgpu::ShaderSource::Wgsl(label_shader_src.into()),
        });

        let label_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("label pipeline"),
            layout: Some(&label_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &label_shader,
                entry_point: Some("vs_main"),
                buffers: &[LabelVertex::desc()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &label_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        // Quad vertex buffer (created once)
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("quad vertex buffer"),
            contents: bytemuck::cast_slice(QUAD_VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });

        Self {
            device,
            queue,
            surface,
            config,
            node_pipeline,
            edge_pipeline,
            hull_pipeline,
            label_pipeline,
            label_vertex_buffer: None,
            label_vertex_count: 0,
            font_bind_group,
            camera_bind_group_layout,
            camera_bind_group,
            camera_buffer,
            vertex_buffer,
            instance_buffer: None,
            instance_count: 0,
            edge_vertex_buffer: None,
            edge_vertex_count: 0,
            hull_vertex_buffer: None,
            hull_vertex_count: 0,
            current_instances: Vec::new(),
            target_instances: Vec::new(),
            current_edge_verts: Vec::new(),
            target_edge_verts: Vec::new(),
            show_edges: true,
            current_zoom: 0.2,
        }
    }

    pub fn resize(&mut self, size: PhysicalSize<u32>) {
        if size.width > 0 && size.height > 0 {
            self.config.width = size.width;
            self.config.height = size.height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    pub fn update_scene(&mut self, scene: &SceneGraph) {
        // Build target instances from new scene
        let new_targets: Vec<NodeInstance> = scene
            .nodes
            .iter()
            .map(NodeInstance::from_scene_node)
            .collect();

        self.target_instances = new_targets;

        // If current_instances is empty (first scene), snap immediately
        if self.current_instances.is_empty() {
            self.current_instances = self.target_instances.clone();
            self.rebuild_instance_buffer();
        }

        // Build target edge vertices
        let new_edge_verts = edges_to_vertices(&scene.edges);
        self.target_edge_verts = new_edge_verts;

        if self.current_edge_verts.is_empty() {
            self.current_edge_verts = self.target_edge_verts.clone();
            self.rebuild_edge_buffer();
        }
    }

    pub fn update_hulls(&mut self, scene: &SceneGraph) {
        let verts = build_hull_geometry(&scene.nodes);
        self.hull_vertex_count = verts.len() as u32;
        if !verts.is_empty() {
            self.hull_vertex_buffer = Some(
                self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("hull vertex buffer"),
                    contents: bytemuck::cast_slice(&verts),
                    usage: wgpu::BufferUsages::VERTEX,
                }),
            );
        } else {
            self.hull_vertex_buffer = None;
        }
    }

    pub fn update_labels(&mut self, scene: &SceneGraph, zoom: f32) {
        let verts = build_label_quads(&scene.nodes, zoom);
        self.label_vertex_count = verts.len() as u32;
        if !verts.is_empty() {
            self.label_vertex_buffer = Some(
                self.device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("label vertex buffer"),
                        contents: bytemuck::cast_slice(&verts),
                        usage: wgpu::BufferUsages::VERTEX,
                    }),
            );
        } else {
            self.label_vertex_buffer = None;
        }
    }

    pub fn animate(&mut self, dt: f32) {
        if self.current_instances.is_empty() {
            return;
        }

        let speed = 8.0 * dt;
        let t = speed.min(1.0);
        let mut changed = false;

        // Lerp node instances toward targets
        let target_len = self.target_instances.len();
        // Resize current to match target (snap new entries)
        if self.current_instances.len() != target_len {
            self.current_instances.resize(target_len, NodeInstance {
                center: [0.0; 2],
                radius: 0.0,
                depth: 0.0,
                color: [0.0; 4],
                glow: 0.0,
                _padding: [0.0; 3],
            });
            // Snap any new entries
            for i in self.current_instances.len()..target_len {
                self.current_instances[i] = self.target_instances[i];
            }
            changed = true;
        }

        for i in 0..target_len.min(self.current_instances.len()) {
            let cur = &mut self.current_instances[i];
            let tgt = &self.target_instances[i];

            cur.center[0] = lerp_f32(cur.center[0], tgt.center[0], t);
            cur.center[1] = lerp_f32(cur.center[1], tgt.center[1], t);
            cur.radius = lerp_f32(cur.radius, tgt.radius, t);
            cur.depth = lerp_f32(cur.depth, tgt.depth, t);
            cur.color = lerp_color(cur.color, tgt.color, t);
            cur.glow = lerp_f32(cur.glow, tgt.glow, t);
            changed = true;
        }

        // Snap edges (they move with nodes anyway)
        if self.current_edge_verts != self.target_edge_verts {
            self.current_edge_verts = self.target_edge_verts.clone();
            self.rebuild_edge_buffer();
        }

        if changed {
            self.rebuild_instance_buffer();
        }
    }

    /// Apply selection highlighting: boost selected node glow, dim unrelated nodes.
    pub fn apply_selection(&mut self, selected_id: Option<&str>, node_ids: &[String]) {
        if let Some(sel_id) = selected_id {
            for (i, inst) in self.current_instances.iter_mut().enumerate() {
                if i < node_ids.len() {
                    if node_ids[i] == sel_id {
                        // Boost selected node glow
                        inst.glow = 1.0;
                    } else {
                        // Dim unrelated nodes
                        inst.color[3] = inst.color[3].min(0.3);
                    }
                }
            }
            self.rebuild_instance_buffer();
        }
    }

    pub fn apply_lod(&mut self, zoom: f32) {
        self.current_zoom = zoom;
        // Only apply LOD filtering when zoomed out significantly
        // At low zoom values, min_visible_radius would hide everything
        if zoom > 0.5 {
            let min_visible_radius = 0.15 / zoom;
            for inst in &mut self.current_instances {
                if inst.radius < min_visible_radius {
                    inst.color[3] = 0.0; // hide small nodes
                }
            }
            self.rebuild_instance_buffer();
        }
    }

    fn rebuild_instance_buffer(&mut self) {
        self.instance_count = self.current_instances.len() as u32;
        if !self.current_instances.is_empty() {
            self.instance_buffer =
                Some(
                    self.device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("node instance buffer"),
                            contents: bytemuck::cast_slice(&self.current_instances),
                            usage: wgpu::BufferUsages::VERTEX,
                        }),
                );
        } else {
            self.instance_buffer = None;
        }
    }

    fn rebuild_edge_buffer(&mut self) {
        self.edge_vertex_count = self.current_edge_verts.len() as u32;
        if !self.current_edge_verts.is_empty() {
            self.edge_vertex_buffer =
                Some(
                    self.device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("edge vertex buffer"),
                            contents: bytemuck::cast_slice(&self.current_edge_verts),
                            usage: wgpu::BufferUsages::VERTEX,
                        }),
                );
        } else {
            self.edge_vertex_buffer = None;
        }
    }

    pub fn update_camera(&mut self, camera: &Camera) {
        let matrix = camera.view_proj();
        self.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&matrix.to_cols_array()),
        );
    }

    pub fn render(&mut self) {
        let output = match self.surface.get_current_texture() {
            Ok(t) => t,
            Err(wgpu::SurfaceError::Lost) => {
                self.surface.configure(&self.device, &self.config);
                return;
            }
            Err(e) => {
                log::error!("Surface error: {:?}", e);
                return;
            }
        };

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render encoder"),
            });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.04,
                            g: 0.04,
                            b: 0.10,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });

            // Draw hull backgrounds FIRST (behind everything)
            if let Some(hull_buf) = &self.hull_vertex_buffer {
                if self.hull_vertex_count > 0 {
                    pass.set_pipeline(&self.hull_pipeline);
                    pass.set_bind_group(0, &self.camera_bind_group, &[]);
                    pass.set_vertex_buffer(0, hull_buf.slice(..));
                    pass.draw(0..self.hull_vertex_count, 0..1);
                }
            }

            // Draw edges BEFORE nodes so nodes render on top
            if self.show_edges && self.current_zoom > 0.15 {
                if let Some(edge_buf) = &self.edge_vertex_buffer {
                    if self.edge_vertex_count > 0 {
                        pass.set_pipeline(&self.edge_pipeline);
                        pass.set_bind_group(0, &self.camera_bind_group, &[]);
                        pass.set_vertex_buffer(0, edge_buf.slice(..));
                        pass.draw(0..self.edge_vertex_count, 0..1);
                    }
                }
            }

            // Draw nodes on top of edges
            if let Some(instance_buffer) = &self.instance_buffer {
                if self.instance_count > 0 {
                    pass.set_pipeline(&self.node_pipeline);
                    pass.set_bind_group(0, &self.camera_bind_group, &[]);
                    pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
                    pass.set_vertex_buffer(1, instance_buffer.slice(..));
                    pass.draw(0..6, 0..self.instance_count);
                }
            }

            // Draw labels LAST (on top of everything)
            if let Some(label_buf) = &self.label_vertex_buffer {
                if self.label_vertex_count > 0 {
                    pass.set_pipeline(&self.label_pipeline);
                    pass.set_bind_group(0, &self.camera_bind_group, &[]);
                    pass.set_bind_group(1, &self.font_bind_group, &[]);
                    pass.set_vertex_buffer(0, label_buf.slice(..));
                    pass.draw(0..self.label_vertex_count, 0..1);
                }
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
    }
}
