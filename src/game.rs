use fastnoise_lite::{FastNoiseLite, NoiseType};
use glam::{IVec3, Vec3, ivec3, vec3};
use rand::{RngExt, SeedableRng, rngs::StdRng};
use winit::keyboard::KeyCode;

use crate::input::Input;

pub struct World {
	pub blocks: Vec<u32>,
	pub seed: u32,
	pub size: IVec3,

	pub camera_fov: f32,
	pub camera_position: Vec3,
	pub camera_rotation: Vec3,

	pub updated: bool,

	itneraction_cooldown: f32,
	selected_block: u32,
}

fn euler_to_vec(pitch_deg: f32, yaw_deg: f32) -> Vec3 {
	let pitch = pitch_deg.to_radians();
	let yaw = yaw_deg.to_radians();

	Vec3::new(
		yaw.sin() * pitch.cos(),
		-pitch.sin(),
		yaw.cos() * pitch.cos(),
	).normalize()
}

impl World {
	pub fn new(seed: u32) -> Self {
		const WIDTH: i32 = 256;
		const HIGHT: i32 = 256;
		const DEPTH: i32 = 256;

		Self {
			blocks: vec![],
			seed,
			size: ivec3(WIDTH, HIGHT, DEPTH),

			camera_fov: 90.0,
			camera_position: vec3((WIDTH / 2) as f32, (HIGHT / 2) as f32, (DEPTH / 2) as f32),
			camera_rotation: vec3(0.0, 0.0, 0.0),

			updated: false,

			itneraction_cooldown: 0.0,
			selected_block: 1,
		}
	}
	pub fn generate(&mut self) {
		self.blocks = vec![0u32; (self.size.x * self.size.y * self.size.z) as usize];

		// terrain noise
		let mut noise = FastNoiseLite::with_seed(self.seed as i32);
		noise.set_noise_type(Some(NoiseType::OpenSimplex2));
		// features randomization
		let mut rng = StdRng::seed_from_u64(self.seed as u64);

		for z in 0..self.size.z {
			for x in 0..self.size.x {
				let tnoise = noise.get_noise_2d(x as f32, z as f32);
				let h = (tnoise * 16.0) as i32 + 127;
				for y in 0..self.size.y {
					let block: u32 = if y==h { 2 } else if y >= h - 4 && y < h { 3 } else if y < h { 1 } else { 0 };
					if block != 0 { self.set_block(x, y, z, block) }
				}

				if rng.random_bool(0.01) {
					// generate tree
					self.fill_blocks(x, h + 1, z, x, h + 3, z, 4);
					self.fill_blocks(x-2, h+4, z-2, x+2, h+5, z+2, 5);
					self.fill_blocks(x-1, h+6, z-1, x+1, h+6, z+1, 5);
					self.fill_blocks(x-1, h+7, z, x+1, h+7, z, 5);
					self.fill_blocks(x, h+7, z-1, x, h+7, z+1, 5);
				}

				// for y in 0..self.size.y {
				// 	let fx = x as f32;
				// 	let fy = y as f32 - 128.0;
				// 	let fz = z as f32;

				// 	let n = (noise.get_noise_3d(fx * 1.1, fy * 1.1, fz * 1.1) * 32.0).min(0.0)
				// 		+ noise.get_noise_3d(fx * 1.6, fy * 1.6, fz * 1.6) * 4.0
				// 		+ noise.get_noise_3d(fx * 2.1, fy * 2.1, fz * 2.1) * 2.0;

				// 	let block: u32 = if n + fy <= 0.0 { 1 } else { 0 };
				// 	self.set_block(x, y, z, block);
				// }
			}
		}
	}

	pub fn set_block(&mut self, x: i32, y: i32, z: i32, block: u32) {
		if x >= 0 && y >= 0 && z >= 0 && x < self.size.x && y < self.size.y && z < self.size.z {
			let idx = (x + y*self.size.x + z*self.size.x*self.size.y) as usize;
			self.blocks[idx] = block;
			self.updated = true;
		}
	}
	pub fn get_block(&self, x: i32, y: i32, z: i32) -> u32 {
		if x >= 0 && y >= 0 && z >= 0 && x < self.size.x && y < self.size.y && z < self.size.z {
			let idx = (x + y*self.size.x + z*self.size.x*self.size.y) as usize;
			self.blocks[idx]
		} else {
			0
		}
	}

	pub fn fill_blocks(&mut self, x1: i32, y1: i32, z1: i32, x2: i32, y2: i32, z2: i32, block: u32) {
		for z in z1..=z2 {
			for y in y1..=y2 {
				for x in x1..=x2 {
					self.set_block(x, y, z, block);
				}
			}
		}
	}

	fn traverse(&self, origin: Vec3, dir: Vec3) -> Option<(IVec3, IVec3)> {
		let inv = 1.0 / dir;
		let sgn = inv.signum();

		// slab intersection
		let t1 = -origin * inv;
		let t2 = (self.size.as_vec3() - origin) * inv;

		let tmins = t1.min(t2);
		let tmaxs = t1.max(t2);

		let tmin = tmins.x.max(tmins.y.max(tmins.z)).max(0.0);
		let tmax = tmaxs.x.min(tmaxs.y).min(tmaxs.z);

		if tmin > tmax {
			return None;
		}
		let mut stepped_axis;

		// traversing
		let neworigin = origin + dir * tmin;
		let mut coord = neworigin.floor().as_ivec3().clamp(ivec3(0, 0, 0), self.size);
		let mut t = (coord.as_vec3() + 0.5 * (1.0 + sgn) - neworigin) * inv;

		let xstep = sgn.as_ivec3();
		let delta = inv * sgn;

		loop {
			if t.x < t.y {
				if t.x < t.z {
					coord.x += xstep.x;
					t.x += delta.x;
					stepped_axis = 0;
				} else {
					coord.z += xstep.z;
					t.z += delta.z;
					stepped_axis = 2;
				}
			} else {
				if t.y < t.z {
					coord.y += xstep.y;
					t.y += delta.y;
					stepped_axis = 1;
				} else {
					coord.z += xstep.z;
					t.z += delta.z;
					stepped_axis = 2;
				}
			}
			if coord.x >= self.size.x as i32 || coord.y >= self.size.y as i32 || coord.z >= self.size.z as i32 || coord.x < 0 || coord.y < 0 || coord.z < 0 {
				return None;
			}
			if self.get_block(coord.x, coord.y, coord.z) != 0 {
				let normal = match stepped_axis {
					0 => if sgn.x > 0.0 { ivec3(-1, 0, 0) } else { ivec3(1, 0, 0) },
					1 => if sgn.y > 0.0 { ivec3(0, -1, 0) } else { ivec3(0, 1, 0) },
					_ => if sgn.z > 0.0 { ivec3(0, 0, -1) } else { ivec3(0, 0, 1) },
				};
				return Some((coord, normal));
			}
		}
	}

	pub fn update(&mut self, dt: f32, input: &Input) {
		if input.pressed(&KeyCode::Digit1) { self.selected_block = 1 }
		if input.pressed(&KeyCode::Digit2) { self.selected_block = 2 }
		if input.pressed(&KeyCode::Digit3) { self.selected_block = 3 }
		if input.pressed(&KeyCode::Digit4) { self.selected_block = 4 }
		if input.pressed(&KeyCode::Digit5) { self.selected_block = 5 }
		if input.pressed(&KeyCode::Digit6) { self.selected_block = 6 }
		if input.pressed(&KeyCode::Digit7) { self.selected_block = 7 }
		if input.pressed(&KeyCode::Digit8) { self.selected_block = 8 }
		if input.pressed(&KeyCode::Digit9) { self.selected_block = 9 }

		const MOVE_SPEED: f32 = 16.0;
		let mouvez = vec3(self.camera_rotation.y.to_radians().sin(), 0.0, self.camera_rotation.y.to_radians().cos());
		if input.pressed(&KeyCode::KeyW) {
			self.camera_position += mouvez * MOVE_SPEED * dt;
		}
		if input.pressed(&KeyCode::KeyS) {
			self.camera_position += mouvez * -MOVE_SPEED * dt;
		}
		if input.pressed(&KeyCode::KeyD) {
			self.camera_position += vec3(0.0, 1.0, 0.0).cross(mouvez) * MOVE_SPEED * dt;
		}
		if input.pressed(&KeyCode::KeyA) {
			self.camera_position += vec3(0.0, 1.0, 0.0).cross(mouvez) * -MOVE_SPEED * dt;
		}
		if input.pressed(&KeyCode::Space) {
			self.camera_position.y += MOVE_SPEED * dt;
		}
		if input.pressed(&KeyCode::ShiftLeft) {
			self.camera_position.y += -MOVE_SPEED * dt;
		}
		if input.pressed(&KeyCode::KeyE) {
			let hit = self.traverse(self.camera_position, euler_to_vec(self.camera_rotation.x, self.camera_rotation.y));
			if let Some(hit) = hit {
				self.camera_position = (hit.0 + hit.1).as_vec3() + vec3(0.5, 0.5, 0.5);
			} 
		}
		
		const PLACE_DELAY: f32 = 0.25;

		self.itneraction_cooldown -= dt;
		if input.rmb && self.itneraction_cooldown < 0.0 {
			let hit = self.traverse(self.camera_position, euler_to_vec(self.camera_rotation.x, self.camera_rotation.y));
			if let Some(hit) = hit {
				let pos = hit.0 + hit.1;
				self.set_block(pos.x, pos.y, pos.z, self.selected_block);
			}
			self.itneraction_cooldown = PLACE_DELAY;
		}
		if input.lmb && self.itneraction_cooldown < 0.0 {
			let hit = self.traverse(self.camera_position, euler_to_vec(self.camera_rotation.x, self.camera_rotation.y));
			if let Some(hit) = hit {
				self.set_block(hit.0.x, hit.0.y, hit.0.z, 0);
			}
			self.itneraction_cooldown = PLACE_DELAY;
		}
		if !input.lmb && !input.rmb {
			self.itneraction_cooldown = 0.0;
		}
		
		self.camera_rotation.y += (input.mouse_delta.0 as f32) * 0.1;
		self.camera_rotation.x += (input.mouse_delta.1 as f32) * 0.1;

		self.camera_rotation.x = self.camera_rotation.x.clamp(-90.0, 90.0);
	}
}