//! Estado de input unificado: ratón, teclado, táctil, mando (básico).

use std::collections::HashSet;

#[derive(Debug, Default, Clone)]
pub struct InputState {
    pub mouse_x: f32,
    pub mouse_y: f32,
    pub mouse_down: bool,
    pub mouse_pressed: bool,
    pub mouse_released: bool,
    pub wheel: f32,
    pub keys_down: HashSet<Key>,
    pub keys_pressed: HashSet<Key>,
    /// Texto introducido este frame (para campos de texto).
    pub text_input: String,
    /// Última posición táctil (si aplica).
    pub touch: Option<(f32, f32)>,
    pub screen: (f32, f32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    W,
    A,
    S,
    D,
    Space,
    Shift,
    Ctrl,
    E,
    Escape,
    Tab,
    F,
    J,
    C,
    R,
    T,
    F3,
    F5,
    Digit1,
    Digit2,
    Digit3,
    Digit4,
    Digit5,
    Digit6,
    Digit7,
    Digit8,
    Digit9,
    Up,
    Down,
    Left,
    Right,
    Enter,
    Backspace,
}

impl InputState {
    pub fn begin_frame(&mut self) {
        self.mouse_pressed = false;
        self.mouse_released = false;
        self.wheel = 0.0;
        self.keys_pressed.clear();
        self.text_input.clear();
    }
    pub fn key(&self, k: Key) -> bool {
        self.keys_down.contains(&k)
    }
    pub fn key_pressed(&self, k: Key) -> bool {
        self.keys_pressed.contains(&k)
    }
    pub fn wants_text(&self) -> bool {
        !self.text_input.is_empty()
    }
}
