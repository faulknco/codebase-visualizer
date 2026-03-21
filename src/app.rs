// src/app.rs
use std::path::PathBuf;
use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowId};

use crate::render::RenderState;

pub struct App {
    repo_path: PathBuf,
    window: Option<Arc<Window>>,
    render_state: Option<RenderState>,
}

impl App {
    pub fn new(repo_path: PathBuf) -> Self {
        Self {
            repo_path,
            window: None,
            render_state: None,
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
            }
            WindowEvent::RedrawRequested => {
                if let Some(state) = &mut self.render_state {
                    state.render();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}
