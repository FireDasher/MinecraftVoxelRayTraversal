use std::{sync::Arc, time::Instant};

use winit::{
	application::ApplicationHandler, event::{DeviceEvent, WindowEvent}, event_loop::{ActiveEventLoop, EventLoop}, window::WindowAttributes
};

mod game;
mod state;
mod input;
use state::State;

struct App {
	state: Option<State>,
	time: Instant,
}

impl App {
	fn new() -> Self {
		Self { state: None, time: Instant::now() }
	}
}

impl ApplicationHandler for App {
	fn resumed(&mut self, event_loop: &ActiveEventLoop) {
		if self.state.is_none() {
			let window = Arc::new(event_loop.create_window(WindowAttributes::default().with_title("Minecraft-like ray traversal").with_maximized(true)).unwrap());
			let state_future = State::new(window);
			self.state = Some(pollster::block_on(state_future));
		}
	}
	fn device_event( &mut self, _event_loop: &ActiveEventLoop, _device_id: winit::event::DeviceId, event: winit::event::DeviceEvent ) {
		if let Some(state) = &mut self.state {
			match event {
				DeviceEvent::MouseMotion { delta } => state.handle_mouse(delta),
				_ => (),
			}
		}
	}
	fn window_event( &mut self, event_loop: &ActiveEventLoop, window_id: winit::window::WindowId, event: WindowEvent ) {
		if let Some(state) = &mut self.state {
			if state.window.id() == window_id {
				match event {
					WindowEvent::CloseRequested => event_loop.exit(),
					WindowEvent::KeyboardInput {event, ..} => state.handle_key(event),
					WindowEvent::MouseInput { state: estate, button, .. } => state.handle_mouse_button(estate, button),
					WindowEvent::Resized(size) => state.resize(size.width, size.height),
					WindowEvent::RedrawRequested => {
						let now = Instant::now();
						let dt = now.duration_since(self.time).as_secs_f32();
						self.time = now;
						state.update(dt);
						match state.render() {
							Ok(_) => {},
							Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
								let size = state.window.inner_size();
								state.resize(size.width, size.height);
							}
							Err(e) => {
								println!("Unable to render {}", e);
							}
						}
					},
					_ => (),
				}
			}
		}
	}
}

fn main() {
	let event_loop = EventLoop::new().unwrap();
	let mut app = App::new();
	event_loop.run_app(&mut app).unwrap();
}