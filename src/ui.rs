use glam::Vec2;
use winit::event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent};
use winit::keyboard::{Key, NamedKey};
use crate::scene::DepthMode;

pub struct InputState {
    pub mouse_pos: Vec2,
    pub dragging: bool,
    pub last_mouse_pos: Vec2,
}

impl InputState {
    pub fn new() -> Self {
        Self {
            mouse_pos: Vec2::ZERO,
            dragging: false,
            last_mouse_pos: Vec2::ZERO,
        }
    }
}

pub enum UiAction {
    Pan(Vec2),
    Zoom(f32),
    SetDepthMode(DepthMode),
    CycleColorMode,
    ResetCamera,
    Deselect,
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
                UiAction::Pan(delta)
            } else {
                UiAction::None
            }
        }
        WindowEvent::MouseInput { state: btn_state, button: MouseButton::Left, .. } => {
            state.dragging = *btn_state == ElementState::Pressed;
            UiAction::None
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
                Key::Named(NamedKey::Escape) => UiAction::Deselect,
                Key::Named(NamedKey::Space) => UiAction::ResetCamera,
                _ => UiAction::None,
            }
        }
        _ => UiAction::None,
    }
}
