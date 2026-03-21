// src/app.rs
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowId};

use crate::render::camera::Camera;
use crate::render::RenderState;
use crate::scene::{ColorMode, DepthMode, SceneGraph};

pub struct App {
    repo_path: PathBuf,
    window: Option<Arc<Window>>,
    render_state: Option<RenderState>,
    scene_rx: mpsc::Receiver<SceneGraph>,
    depth_mode: Arc<RwLock<DepthMode>>,
    color_mode: Arc<RwLock<ColorMode>>,
    latest_scene: Option<SceneGraph>,
    camera: Option<Camera>,
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

        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}
