// src/app.rs
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{mpsc, RwLock};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowId};
use glam::Vec2;

use crate::render::camera::Camera;
use crate::render::RenderState;
use crate::scene::{ColorMode, DepthMode, SceneGraph};
use crate::ui::{self, InputState, UiAction};

pub struct App {
    repo_path: PathBuf,
    window: Option<Arc<Window>>,
    render_state: Option<RenderState>,
    scene_rx: mpsc::Receiver<SceneGraph>,
    depth_mode: Arc<RwLock<DepthMode>>,
    color_mode: Arc<RwLock<ColorMode>>,
    latest_scene: Option<SceneGraph>,
    camera: Option<Camera>,
    input_state: InputState,
    last_frame: Instant,
    selected_node: Option<String>,
    hovered_node: Option<String>,
}

impl App {
    pub fn new(
        repo_path: PathBuf,
        scene_rx: mpsc::Receiver<SceneGraph>,
        depth_mode: Arc<RwLock<DepthMode>>,
        color_mode: Arc<RwLock<ColorMode>>,
    ) -> Self {
        Self {
            repo_path,
            window: None,
            render_state: None,
            scene_rx,
            depth_mode,
            color_mode,
            latest_scene: None,
            camera: None,
            input_state: InputState::new(),
            last_frame: Instant::now(),
            selected_node: None,
            hovered_node: None,
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title(format!("cviz — {}", self.repo_path.display()))
            .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 800.0));
        let window = Arc::new(event_loop.create_window(attrs).expect("Failed to create window"));

        let size = window.inner_size();
        let aspect = size.width as f32 / size.height.max(1) as f32;
        self.camera = Some(Camera::new(aspect));

        self.render_state = Some(pollster::block_on(RenderState::new(Arc::clone(&window))));
        self.window = Some(window);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let action = ui::handle_event(&event, &mut self.input_state);
        match action {
            UiAction::Pan(delta) => {
                if let Some(camera) = &mut self.camera {
                    camera.pan(delta);
                }
            }
            UiAction::Zoom(factor) => {
                if let Some(camera) = &mut self.camera {
                    camera.zoom_by(factor);
                }
            }
            UiAction::SetDepthMode(mode) => {
                *self.depth_mode.blocking_write() = mode;
            }
            UiAction::CycleColorMode => {
                let current = *self.color_mode.blocking_read();
                let next = match current {
                    ColorMode::FileType => ColorMode::Recency,
                    ColorMode::Recency => ColorMode::FileType,
                };
                *self.color_mode.blocking_write() = next;
            }
            UiAction::ResetCamera => {
                if let Some(camera) = &mut self.camera {
                    camera.reset();
                }
            }
            UiAction::Click(pos) => {
                // Hit test on click
                if let (Some(scene), Some(camera), Some(window)) =
                    (&self.latest_scene, &self.camera, &self.window)
                {
                    let size = window.inner_size();
                    let window_size = Vec2::new(size.width as f32, size.height as f32);
                    if let Some(node) = ui::hit_test(pos, scene, camera, window_size) {
                        self.selected_node = Some(node.id.clone());
                        // Print inspector info to stdout
                        println!();
                        println!("\u{2550}\u{2550}\u{2550} {} \u{2550}\u{2550}\u{2550}", node.id);
                        println!("  Position: ({:.2}, {:.2})", node.pos.x, node.pos.y);
                        println!("  Radius: {:.3}", node.radius);
                        println!("  Depth: {:.3}", node.depth);
                        println!("  Glow: {:.3}", node.glow);
                    } else {
                        self.selected_node = None;
                    }
                }
            }
            UiAction::Deselect => {
                self.selected_node = None;
            }
            UiAction::None => {}
        }

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(state) = &mut self.render_state {
                    state.resize(size);
                }
                if let Some(camera) = &mut self.camera {
                    if size.width > 0 && size.height > 0 {
                        camera.set_aspect(size.width as f32 / size.height as f32);
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                if let (Some(state), Some(camera)) = (&mut self.render_state, &self.camera) {
                    state.update_camera(camera);
                    state.render();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        // Compute delta time
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f32();
        self.last_frame = now;

        // Drain the latest scene update without blocking
        let mut scene_changed = false;
        while let Ok(scene) = self.scene_rx.try_recv() {
            self.latest_scene = Some(scene);
            scene_changed = true;
        }

        if scene_changed {
            if let (Some(state), Some(scene)) = (&mut self.render_state, &self.latest_scene) {
                state.update_scene(scene);
            }
        }

        // Animate camera interpolation
        if let Some(camera) = &mut self.camera {
            camera.update(dt);
        }

        // Animate node/edge interpolation
        if let Some(state) = &mut self.render_state {
            state.animate(dt);
        }

        // Apply selection highlighting
        if let (Some(state), Some(scene)) = (&mut self.render_state, &self.latest_scene) {
            let node_ids: Vec<String> = scene.nodes.iter().map(|n| n.id.clone()).collect();
            state.apply_selection(self.selected_node.as_deref(), &node_ids);
        }

        // Update camera uniform
        if let (Some(state), Some(camera)) = (&mut self.render_state, &self.camera) {
            state.update_camera(camera);
        }

        // Hover: hit test at current mouse position, update window title
        if let (Some(scene), Some(camera), Some(window)) =
            (&self.latest_scene, &self.camera, &self.window)
        {
            let size = window.inner_size();
            let window_size = Vec2::new(size.width as f32, size.height as f32);
            let hit = ui::hit_test(self.input_state.mouse_pos, scene, camera, window_size);
            let new_hover = hit.map(|n| n.id.clone());
            if new_hover != self.hovered_node {
                self.hovered_node = new_hover;
                let title = if let Some(ref name) = self.hovered_node {
                    format!("cviz — {}", name)
                } else {
                    format!("cviz — {}", self.repo_path.display())
                };
                window.set_title(&title);
            }
        }

        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}
