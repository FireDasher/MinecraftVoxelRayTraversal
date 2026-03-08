use glam::Mat4;
use image::GenericImageView;
use wgpu::util::DeviceExt;
use winit::event::{ElementState, KeyEvent, MouseButton};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorGrabMode, Window};
use std::sync::Arc;
use std::borrow::Cow;

use crate::game::World;
use crate::input::Input;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
	camera: [[f32; 4]; 4],
}

pub struct State {
	// Rendering
	pub window:         Arc<Window>,
	surface:            wgpu::Surface<'static>,
	device:             wgpu::Device,
	queue:              wgpu::Queue,
	config:             wgpu::SurfaceConfiguration,

	compute_pipeline:   wgpu::ComputePipeline,

	compute_texture:    wgpu::Texture,
	compute_bgl:        wgpu::BindGroupLayout,
	compute_bind_group: wgpu::BindGroup,

	uniform_buffer:     wgpu::Buffer,
	voxel_texture:      wgpu::Texture,

	block_textures_texture: wgpu::Texture,

	// Game
	pub world:          World,

	// Input
	input:              Input,
	cursor_grabbed:     bool,
}

impl State {
	pub async fn new(window: Arc<Window>) -> Self {
		let size = window.inner_size();

		// Device stuff
		let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
			backends: wgpu::Backends::PRIMARY,
			..Default::default()
		});

		let surface = instance.create_surface(window.clone()).unwrap();

		let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions {
			power_preference: wgpu::PowerPreference::default(),
			compatible_surface: Some(&surface),
			force_fallback_adapter: false,
		}).await.unwrap();

		let (device, queue) = adapter.request_device(&wgpu::DeviceDescriptor {
			label: None,
			required_features: wgpu::Features::empty(),
			required_limits: wgpu::Limits::defaults(),
			memory_hints: Default::default(),
			trace: wgpu::Trace::Off,
		}).await.unwrap();

		// Surface
		let surface_caps = surface.get_capabilities(&adapter);
		let surface_format = surface_caps.formats.iter()
			.find(|f| f.is_srgb())
			.copied()
			.unwrap_or(surface_caps.formats[0]);

		let config = wgpu::SurfaceConfiguration {
			usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_DST,
			format: surface_format,
			width: size.width,
			height: size.height,
			present_mode: wgpu::PresentMode::Fifo,
			alpha_mode: wgpu::CompositeAlphaMode::Opaque,
			view_formats: vec![],
			desired_maximum_frame_latency: 2,
		};

		// Render Texture
		let compute_texture = device.create_texture(&wgpu::TextureDescriptor {
			label: Some("compute_texture"),
			size: wgpu::Extent3d {
				width: size.width,
				height: size.height,
				depth_or_array_layers: 1,
			},
			mip_level_count: 1,
			sample_count: 1,
			dimension: wgpu::TextureDimension::D2,
			format: wgpu::TextureFormat::Rgba8Unorm,
			usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::COPY_SRC,
			view_formats: &[],
		});
		
		// Unfirms
		let uniforms = Uniforms {
			camera: Mat4::IDENTITY.to_cols_array_2d(),
		};
		
		let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some("compute_uniform_buffer"),
			contents: bytemuck::cast_slice(&[uniforms]),
			usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
		});
		
		// Voxel texture
		let mut world = World::new(67);
		world.generate();
		
		let voxel_texture = device.create_texture(&wgpu::TextureDescriptor {
			label: Some("voxel_textyre"),
			size: wgpu::Extent3d{ width: world.size.x as u32, height: world.size.y as u32, depth_or_array_layers: world.size.z as u32 },
			mip_level_count: 1,
			sample_count: 1,
			dimension: wgpu::TextureDimension::D3,
			format: wgpu::TextureFormat::R32Uint,
			usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
			view_formats: &[],
		});
		
		// Block textures
		let img = image::open("assets/textures.png").expect("Textures failed to load!!!");
		let img_rgb = img.to_rgba8();
		let img_dimensions = img.dimensions();
		
		let img_size = wgpu::Extent3d {
			width: img_dimensions.0,
			height: img_dimensions.1,
			depth_or_array_layers: 1,
		};
		
		let block_textures_texture = device.create_texture(&wgpu::TextureDescriptor {
			label: Some("block_textures_texture"),
			size: img_size,
			mip_level_count: 1,
			sample_count: 1,
			dimension: wgpu::TextureDimension::D2,
			format: wgpu::TextureFormat::Rgba8Unorm,
			usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
			view_formats: &[],
		});
		
		queue.write_texture(wgpu::TexelCopyTextureInfo{
				texture: &block_textures_texture,
				mip_level: 0,
				origin: wgpu::Origin3d::ZERO,
				aspect: wgpu::TextureAspect::All,
			},
			&img_rgb,
			wgpu::TexelCopyBufferLayout{
				offset: 0,
				bytes_per_row: Some(4 * img_dimensions.0),
				rows_per_image: None,
			},
			img_size
		);
	
	// Shader
	let shader_source = include_str!("shader.wgsl");
	let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
		label: Some("compute_shader"),
		source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(shader_source)),
	});
	
	let compute_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
		label: Some("compute_bgl"),
		entries: &[
			wgpu::BindGroupLayoutEntry {
				binding: 0,
				visibility: wgpu::ShaderStages::COMPUTE,
				ty: wgpu::BindingType::StorageTexture {
					access: wgpu::StorageTextureAccess::WriteOnly,
					format: wgpu::TextureFormat::Rgba8Unorm,
					view_dimension: wgpu::TextureViewDimension::D2,
				},
				count: None,
			},
			wgpu::BindGroupLayoutEntry {
				binding: 1,
				visibility: wgpu::ShaderStages::COMPUTE,
				ty: wgpu::BindingType::Buffer {
					ty: wgpu::BufferBindingType::Uniform,
					has_dynamic_offset: false,
					min_binding_size: None,
				},
				count: None,
			},
			wgpu::BindGroupLayoutEntry {
				binding: 2,
				visibility: wgpu::ShaderStages::COMPUTE,
				ty: wgpu::BindingType::Texture {
					sample_type: wgpu::TextureSampleType::Uint,
					view_dimension: wgpu::TextureViewDimension::D3,
					multisampled: false,
				},
				count: None,
			},
			wgpu::BindGroupLayoutEntry {
				binding: 3,
				visibility: wgpu::ShaderStages::COMPUTE,
				ty: wgpu::BindingType::Texture {
					sample_type: wgpu::TextureSampleType::Float { filterable: false },
					view_dimension: wgpu::TextureViewDimension::D2,
					multisampled: false
				},
				count: None,
			}
			],
		});
		
		let compute_view = compute_texture.create_view(&wgpu::TextureViewDescriptor::default());
		let voxel_view = voxel_texture.create_view(&wgpu::TextureViewDescriptor::default());
		let block_textures_texture_view = block_textures_texture.create_view(&wgpu::TextureViewDescriptor::default());

		let compute_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
			label: Some("compute_bind_group"),
			layout: &compute_bgl,
			entries: &[
				wgpu::BindGroupEntry {
					binding: 0,
					resource: wgpu::BindingResource::TextureView(&compute_view),
				},
				wgpu::BindGroupEntry {
					binding: 1,
					resource: uniform_buffer.as_entire_binding(),
				},
				wgpu::BindGroupEntry {
					binding: 2,
					resource: wgpu::BindingResource::TextureView(&voxel_view),
				},
				wgpu::BindGroupEntry {
					binding: 3,
					resource: wgpu::BindingResource::TextureView(&block_textures_texture_view),
				}
			],
		});

		// Pipeline
		let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
			label: Some("compute_pipeline_layout"),
			bind_group_layouts: &[&compute_bgl],
			push_constant_ranges: &[],
		});

		let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
			label: Some("compute_pipeline"),
			layout: Some(&pipeline_layout),
			module: &shader_module,
			entry_point: Some("main"),
			cache: None,
			compilation_options: Default::default(),
		});

		surface.configure(&device, &config);

		// Creation and return
		Self {
			window,
			surface,
			device,
			queue,
			config,

			compute_pipeline,

			compute_texture,
			compute_bgl,
			compute_bind_group,

			uniform_buffer,
			voxel_texture,

			block_textures_texture,

			world,

			input: Input::new(),
			cursor_grabbed: false,
		}
	}

	fn update_world(&self) {
		self.queue.write_texture(
			wgpu::TexelCopyTextureInfo {
				texture: &self.voxel_texture,
				mip_level: 0,
				origin: wgpu::Origin3d::ZERO,
				aspect: wgpu::TextureAspect::All,
			},
			bytemuck::cast_slice(&self.world.blocks),
			wgpu::TexelCopyBufferLayout {
				offset: 0,
				bytes_per_row: Some(self.world.size.x as u32 * 4),
				rows_per_image: Some(self.world.size.y as u32),
			},
			wgpu::Extent3d{ width: self.world.size.x as u32, height: self.world.size.y as u32, depth_or_array_layers: self.world.size.z as u32 }
		);
	}

	pub fn resize(&mut self, width: u32, height: u32) {
		if width > 0 && height > 0 {
			self.config.width = width;
			self.config.height = height;
			self.surface.configure(&self.device, &self.config);

			self.compute_texture = self.device.create_texture(&wgpu::TextureDescriptor {
				label: Some("compute_texture"),
				size: wgpu::Extent3d {
					width: width,
					height: height,
					depth_or_array_layers: 1,
				},
				mip_level_count: 1,
				sample_count: 1,
				dimension: wgpu::TextureDimension::D2,
				format: wgpu::TextureFormat::Rgba8Unorm,
				usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::COPY_SRC,
				view_formats: &[],
			});

			let compute_view = self.compute_texture.create_view(&wgpu::TextureViewDescriptor::default());
			let voxel_view = self.voxel_texture.create_view(&wgpu::TextureViewDescriptor::default());
			let block_textures_texture_view = self.block_textures_texture.create_view(&wgpu::TextureViewDescriptor::default());

			self.compute_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
				label: Some("compute_bind_group"),
				layout: &self.compute_bgl,
				entries: &[
					wgpu::BindGroupEntry {
						binding: 0,
						resource: wgpu::BindingResource::TextureView(&compute_view),
					},
					wgpu::BindGroupEntry {
						binding: 1,
						resource: self.uniform_buffer.as_entire_binding(),
					},
					wgpu::BindGroupEntry {
						binding: 2,
						resource: wgpu::BindingResource::TextureView(&voxel_view),
					},
					wgpu::BindGroupEntry {
						binding: 3,
						resource: wgpu::BindingResource::TextureView(&block_textures_texture_view),
					}
				],
			});
		}
	}

	fn get_camrea_matrix(&self) -> Mat4 {
		let aspect = self.config.width as f32 / self.config.height as f32;
		let tan_fov = (self.world.camera_fov.to_radians() * 0.5).tan();

		// Center pixel offset
		let center_pixel = Mat4::from_cols_array(&[
			1.0, 0.0, 0.0, 0.0,
			0.0, 1.0, 0.0, 0.0,
			0.5, 0.5, 1.0, 0.0,
			0.0, 0.0, 0.0, 1.0,
		]);

		// Pixel → UV transform
		let pixel_to_uv = Mat4::from_cols_array(&[
			2.0 / self.config.width as f32, 0.0, 0.0, 0.0,
			0.0, -2.0 / self.config.height as f32, 0.0, 0.0,
			-1.0, 1.0, 1.0, 0.0,
			0.0, 0.0, 0.0, 1.0,
		]);

		// UV → view space scaling
		let uv_to_view = Mat4::from_cols_array(&[
			tan_fov * aspect.max(1.0), 0.0, 0.0, 0.0,
			0.0, tan_fov / aspect.min(1.0), 0.0, 0.0,
			0.0, 0.0, 1.0, 0.0,
			0.0, 0.0, 0.0, 1.0,
		]);

		// Build rotation (from Euler angles)
		let rotation = Mat4::from_euler(
			glam::EulerRot::YXZ,
			self.world.camera_rotation.y.to_radians(),
			self.world.camera_rotation.x.to_radians(),
			self.world.camera_rotation.z.to_radians(),
		);

		// Translation matrix
		let translation = Mat4::from_translation(self.world.camera_position);

		translation * rotation * uv_to_view * pixel_to_uv * center_pixel
	}

	pub fn update(&mut self, dt: f32) {
		self.world.update(dt, &self.input);
		self.input.mouse_delta = (0.0, 0.0);
		if self.world.updated {
			self.update_world();
			self.world.updated = false;
		}
	}

	pub fn handle_mouse(&mut self, delta: (f64, f64)) {
		if self.cursor_grabbed {
			self.input.mouse_delta = delta;
		} else {
			self.input.mouse_delta = (0.0, 0.0);
		}
	}

	pub fn handle_key(&mut self, event: KeyEvent) {
		if let PhysicalKey::Code(code) = event.physical_key {
			if !event.repeat {
				if event.state == ElementState::Pressed {
					self.input.pressed_keys.insert(code);
					if code == KeyCode::Escape {
						self.cursor_grabbed = false;
						self.window.set_cursor_grab(CursorGrabMode::None).unwrap();
						self.window.set_cursor_visible(true);
					} else {
						self.cursor_grabbed = true;
						self.window.set_cursor_grab(CursorGrabMode::Locked).unwrap();
						self.window.set_cursor_visible(false);
					}
				} else {
					self.input.pressed_keys.remove(&code);
				}
			}
		}
	}

	pub fn handle_mouse_button(&mut self, state: ElementState, button: MouseButton) {
		match button {
			MouseButton::Left => self.input.lmb = state.is_pressed(),
			MouseButton::Right => self.input.rmb = state.is_pressed(),
			_ => ()
		}
	}

	pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
		let output = match self.surface.get_current_texture() {
			Ok(t) => t,
			Err(e) => return Err(e),
		};

		let cameramatrix = self.get_camrea_matrix();
		let uniforms = Uniforms {
			camera: cameramatrix.to_cols_array_2d(),
		};
		self.queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));

		let mut encoder = self.device.create_command_encoder(&wgpu::wgt::CommandEncoderDescriptor { label: Some("compute_encoder") });

		{
			let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("compute_pass"), timestamp_writes: None });
			cpass.set_pipeline(&self.compute_pipeline);
			cpass.set_bind_group(0, &self.compute_bind_group, &[]);
			cpass.dispatch_workgroups(
				(self.config.width + 15) / 16,
				(self.config.height + 15) / 16,
				1
			);
		}

		encoder.copy_texture_to_texture(
			wgpu::TexelCopyTextureInfo {
				texture: &self.compute_texture,
				mip_level: 0,
				origin: wgpu::Origin3d::ZERO,
				aspect: wgpu::TextureAspect::All,
			},
			wgpu::TexelCopyTextureInfo {
				texture: &output.texture,
				mip_level: 0,
				origin: wgpu::Origin3d::ZERO,
				aspect: wgpu::TextureAspect::All,
			},
			wgpu::Extent3d {
				width: self.config.width,
				height: self.config.height,
				depth_or_array_layers: 1,
			},
		);

		self.queue.submit(Some(encoder.finish()));
		output.present();

		self.window.request_redraw();

		Ok(())
	}
}