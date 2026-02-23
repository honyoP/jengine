use std::collections::{HashSet, HashMap};
use std::hash::Hash;
pub use winit::keyboard::KeyCode;
pub use winit::event::MouseButton;

/// Represents a raw input source that can be bound to an action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InputSource {
    Key(KeyCode),
    Mouse(MouseButton),
}

/// Raw hardware state for a single frame.
#[derive(Debug, Default)]
pub struct InputState {
    pub keys_held: HashSet<KeyCode>,
    pub keys_pressed: HashSet<KeyCode>,
    pub keys_released: HashSet<KeyCode>,
    
    pub mouse_pos: [f32; 2],
    pub mouse_wheel: f32,
    pub mouse_held: HashSet<MouseButton>,
    pub mouse_pressed: HashSet<MouseButton>,
    pub mouse_released: HashSet<MouseButton>,
    
    pub chars_typed: Vec<char>,
    /// Set to true if a UI element has consumed keyboard input this frame.
    pub key_consumed: bool,
    /// Set to true if a UI element has consumed mouse input this frame.
    pub mouse_consumed: bool,
}

impl InputState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear_frame_state(&mut self) {
        self.keys_pressed.clear();
        self.keys_released.clear();
        self.mouse_pressed.clear();
        self.mouse_released.clear();
        self.chars_typed.clear();
        self.mouse_wheel = 0.0;
        self.key_consumed = false;
        self.mouse_consumed = false;
    }

    pub fn is_key_held(&self, key: KeyCode) -> bool { self.keys_held.contains(&key) }
    pub fn is_key_pressed(&self, key: KeyCode) -> bool { self.keys_pressed.contains(&key) }
    pub fn is_key_released(&self, key: KeyCode) -> bool { self.keys_released.contains(&key) }

    pub fn is_mouse_held(&self, button: MouseButton) -> bool { self.mouse_held.contains(&button) }
    pub fn is_mouse_pressed(&self, button: MouseButton) -> bool { self.mouse_pressed.contains(&button) }
    pub fn is_mouse_released(&self, button: MouseButton) -> bool { self.mouse_released.contains(&button) }

    /// Returns true if the mouse is currently within the given pixel bounds.
    pub fn is_mouse_over(&self, x: f32, y: f32, w: f32, h: f32) -> bool {
        let [mx, my] = self.mouse_pos;
        mx >= x && mx < x + w && my >= y && my < y + h
    }

    /// Returns true if the mouse was clicked (pressed) within the given bounds this frame.
    pub fn was_clicked(&self, x: f32, y: f32, w: f32, h: f32) -> bool {
        self.is_mouse_pressed(MouseButton::Left) && self.is_mouse_over(x, y, w, h)
    }
}

/// Maps logical actions (defined by the game) to one or more physical inputs.
#[derive(Debug, Clone)]
pub struct ActionMap<A: Hash + Eq + Copy> {
    bindings: HashMap<A, Vec<InputSource>>,
}

impl<A: Hash + Eq + Copy> ActionMap<A> {
    pub fn new() -> Self {
        Self { bindings: HashMap::new() }
    }

    pub fn bind(&mut self, action: A, source: InputSource) {
        self.bindings.entry(action).or_insert_with(Vec::new).push(source);
    }

    /// Returns true if the action was triggered this frame (pressed).
    pub fn is_pressed(&self, action: A, input: &InputState) -> bool {
        self.bindings.get(&action).map_or(false, |sources| {
            sources.iter().any(|s| match s {
                InputSource::Key(k) => !input.key_consumed && input.is_key_pressed(*k),
                InputSource::Mouse(b) => !input.mouse_consumed && input.is_mouse_pressed(*b),
            })
        })
    }

    /// Returns true if the action is currently being held.
    ///
    /// `key_consumed` does NOT suppress held queries — a focused text field blocks
    /// new presses but should not stop ongoing held movement or camera keys.
    pub fn is_held(&self, action: A, input: &InputState) -> bool {
        self.bindings.get(&action).map_or(false, |sources| {
            sources.iter().any(|s| match s {
                InputSource::Key(k) => input.is_key_held(*k),
                InputSource::Mouse(b) => input.is_mouse_held(*b),
            })
        })
    }

    /// Returns true if any bound source was released this frame.
    ///
    /// Like `is_held`, this does NOT check `key_consumed` — release events
    /// should always be observable regardless of UI focus state.
    pub fn is_released(&self, action: A, input: &InputState) -> bool {
        self.bindings.get(&action).map_or(false, |sources| {
            sources.iter().any(|s| match s {
                InputSource::Key(k) => input.is_key_released(*k),
                InputSource::Mouse(b) => input.is_mouse_released(*b),
            })
        })
    }
}

impl<A: Hash + Eq + Copy> Default for ActionMap<A> {
    fn default() -> Self { Self::new() }
}
