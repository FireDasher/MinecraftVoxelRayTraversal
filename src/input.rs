use std::collections::HashSet;

use winit::keyboard::KeyCode;

pub struct Input {
	pub pressed_keys: HashSet<KeyCode>,
	pub mouse_delta: (f64, f64),
	pub lmb: bool,
	pub rmb: bool,
}

impl Input {
	pub fn new() -> Self {
		Self {
			pressed_keys: HashSet::new(),
			mouse_delta: (0.0, 0.0),
			lmb: false, rmb: false,
		}
	}
	
	pub fn pressed(&self, key: &KeyCode) -> bool {
		self.pressed_keys.contains(key)
	}
}