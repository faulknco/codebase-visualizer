use glam::Vec2;
use winit::event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent};
use winit::keyboard::{Key, NamedKey};
use crate::render::camera::Camera;
use crate::scene::{DepthMode, SceneGraph, SceneNode};

pub struct InputState {
    pub mouse_pos: Vec2,
    pub dragging: bool,
    pub last_mouse_pos: Vec2,
    drag_start: Vec2,
    drag_distance: f32,
}

impl InputState {
    pub fn new() -> Self {
        Self {
            mouse_pos: Vec2::ZERO,
            dragging: false,
            last_mouse_pos: Vec2::ZERO,
            drag_start: Vec2::ZERO,
            drag_distance: 0.0,
        }
    }

    pub fn was_click(&self) -> bool {
        self.drag_distance < 5.0
    }
}

pub enum UiAction {
    Pan(Vec2),
    Zoom(f32),
    SetDepthMode(DepthMode),
    CycleColorMode,
    ResetCamera,
    FitAll,
    ToggleEdges,
    Deselect,
    Click(Vec2),
    Quit,
    None,
}

pub fn handle_event(event: &WindowEvent, state: &mut InputState) -> UiAction {
    match event {
        WindowEvent::MouseWheel { delta, .. } => {
            let scroll = match delta {
                MouseScrollDelta::LineDelta(_, y) => *y,
                MouseScrollDelta::PixelDelta(p) => p.y as f32 / 100.0,
            };
            UiAction::Zoom(1.0 + scroll * 0.1)
        }
        WindowEvent::CursorMoved { position, .. } => {
            let pos = Vec2::new(position.x as f32, position.y as f32);
            state.last_mouse_pos = state.mouse_pos;
            state.mouse_pos = pos;
            if state.dragging {
                let delta = pos - state.last_mouse_pos;
                state.drag_distance += delta.length();
                UiAction::Pan(delta)
            } else {
                UiAction::None
            }
        }
        WindowEvent::MouseInput { state: btn_state, button: MouseButton::Left, .. } => {
            if *btn_state == ElementState::Pressed {
                state.dragging = true;
                state.drag_start = state.mouse_pos;
                state.drag_distance = 0.0;
                UiAction::None
            } else {
                state.dragging = false;
                if state.was_click() {
                    UiAction::Click(state.mouse_pos)
                } else {
                    UiAction::None
                }
            }
        }
        WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Right, .. } => {
            UiAction::Deselect
        }
        WindowEvent::KeyboardInput { event: KeyEvent { logical_key, state: ElementState::Pressed, .. }, .. } => {
            match logical_key {
                Key::Character(c) if c.as_str() == "1" => UiAction::SetDepthMode(DepthMode::Recency),
                Key::Character(c) if c.as_str() == "2" => UiAction::SetDepthMode(DepthMode::Coupling),
                Key::Character(c) if c.as_str() == "3" => UiAction::SetDepthMode(DepthMode::Importance),
                Key::Character(c) if c.as_str() == "c" => UiAction::CycleColorMode,
                Key::Character(c) if c.as_str() == "f" => UiAction::FitAll,
                Key::Character(c) if c.as_str() == "g" => UiAction::ToggleEdges,
                Key::Character(c) if c.as_str() == "q" => UiAction::Quit,
                Key::Character(c) if c.as_str() == "=" || c.as_str() == "+" => UiAction::Zoom(1.2),
                Key::Character(c) if c.as_str() == "-" => UiAction::Zoom(0.8),
                Key::Named(NamedKey::Escape) => UiAction::Deselect,
                Key::Named(NamedKey::Space) => UiAction::ResetCamera,
                _ => UiAction::None,
            }
        }
        _ => UiAction::None,
    }
}

pub fn hit_test<'a>(
    mouse_pos: Vec2,
    scene: &'a SceneGraph,
    camera: &Camera,
    window_size: Vec2,
) -> Option<&'a SceneNode> {
    // Convert screen coords to world coords
    let ndc_x = (mouse_pos.x / window_size.x) * 2.0 - 1.0;
    let ndc_y = -((mouse_pos.y / window_size.y) * 2.0 - 1.0); // flip Y
    let half_w = 10.0 / camera.zoom;
    let half_h = half_w / camera.aspect;
    let world_x = camera.center.x + ndc_x * half_w;
    let world_y = camera.center.y + ndc_y * half_h;
    let world_pos = Vec2::new(world_x, world_y);

    // Find nearest node within its radius
    let mut best: Option<(&SceneNode, f32)> = None;
    for node in &scene.nodes {
        let dist = world_pos.distance(node.pos);
        if dist <= node.radius * 2.0 {
            // generous hit area
            if best.is_none() || dist < best.unwrap().1 {
                best = Some((node, dist));
            }
        }
    }
    best.map(|(node, _)| node)
}
