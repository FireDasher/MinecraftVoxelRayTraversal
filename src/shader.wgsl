struct Uniforms {
	camera: mat4x4f,
};

@group(0) @binding(0)
var out_image: texture_storage_2d<rgba8unorm, write>;

@group(0) @binding(1)
var<uniform> uniforms: Uniforms;

@group(0) @binding(2)
var voxel_image: texture_3d<u32>;

@group(0) @binding(3)
var textures: texture_2d<f32>;

struct HitInfo {
	block: u32,
	point: vec3f,
	normal: vec3f,
	color: vec3f,
	// side: u32,
	// uv: vec2f,
	// steps: u32,
};

const NO_HIT = HitInfo(0, vec3f(0.0), vec3f(0.0), vec3f(0.0));

fn traverse(origin: vec3f, dir: vec3f) -> HitInfo {
	let size = textureDimensions(voxel_image);

	let inv = 1.0 / dir;
	let sgn = sign(inv);

	// slab intersection
	let t1 = -origin * inv;
	let t2 = (vec3f(size) - origin) * inv;

	let tmins = min(t1, t2);
	let tmaxs = max(t1, t2);

	let tmin = max(0.0, max(tmins.x, max(tmins.y, tmins.z)));
	let tmax = min(tmaxs.x, min(tmaxs.y, tmaxs.z));

	if tmin > tmax {
		return NO_HIT;
	}

	var stepped_axis = 0u;

	if tmins.x < tmins.y {
		if tmins.x < tmins.z {
			stepped_axis = 0;
		} else {
			stepped_axis = 2;
		}
	} else {
		if tmins.y < tmins.z {
			stepped_axis = 1;
		} else {
			stepped_axis = 2;
		}
	}
	
	// traversing
	let neworigin = origin + dir * tmin;
	
	var coord = clamp(vec3i(floor(neworigin)), vec3i(0i), vec3i(size) - 1);
	var t = (vec3f(coord) + 0.5 * (1.0 + sgn) - neworigin) * inv;

	let xstep = vec3i(sgn);
	let delta = abs(inv);

	loop {
		var t_hit = 0.0;

		// Step ray
		if t.x < t.y {
			if t.x < t.z {
				t_hit = t.x;

				coord.x += xstep.x;
				t.x += delta.x;
				stepped_axis = 0;
			} else {
				t_hit = t.z;

				coord.z += xstep.z;
				t.z += delta.z;
				stepped_axis = 2;
			}
		} else {
			if t.y < t.z {
				t_hit = t.y;

				coord.y += xstep.y;
				t.y += delta.y;
				stepped_axis = 1;
			} else {
				t_hit = t.z;

				coord.z += xstep.z;
				t.z += delta.z;
				stepped_axis = 2;
			}
		}
		// Check out of bounds
		if coord.x >= i32(size.x) || coord.y >= i32(size.y) || coord.z >= i32(size.z) || coord.x < 0 || coord.y < 0 || coord.z < 0 {
			return NO_HIT;
		}
		let block = textureLoad(voxel_image, coord, 0).r;
		if block != 0 {
			// Hit voxel
			var mask = vec3f(0.0);
			mask[stepped_axis] = 1.0;

			let hit = (neworigin + dir * t_hit);
			let localhit = hit - vec3f(coord);
			let uv = select(select(select(vec2f(localhit.x, 1.0 - localhit.y), 1.0 - localhit.xy, sgn.z < 0.0), select(1.0 - localhit.xz, vec2f(localhit.x, 1.0 - localhit.z), sgn.y < 0.0), stepped_axis == 1u), select(1.0 - localhit.zy, vec2f(localhit.z, 1.0 - localhit.y), sgn.x < 0.0), stepped_axis == 0u);
			let side = select(select(select(3u, 1u, sgn.z < 0.0), select(5u, 0u, sgn.y < 0.0), stepped_axis == 1u), select(4u, 2u, sgn.x < 0.0), stepped_axis == 0u);

			let color = textureLoad(textures, vec2i(uv * 16f) + vec2i(i32(side) * 16i, i32(block - 1) * 16i), 0).rgb;

			return HitInfo(block, hit, -mask * sgn, color);
		}
	}
	return NO_HIT;
}

fn random_direction(normal: vec3f, seed: u32) -> vec3f {
	// Uniform random numbers in [0,1)
	var x = f32((seed * 1664525u + 1013904223u) & 0xFFFFFFFFu) / 4294967295.0;
	var y = f32(((seed + 1u) * 1664525u + 1013904223u) & 0xFFFFFFFFu) / 4294967295.0;

	let r = sqrt(x);
	let theta = 2.0 * 3.1415926 * y;
	let local = vec3f(r*cos(theta), r*sin(theta), sqrt(1.0 - x));

	// Build tangent/bitangent frame
	var up = select(vec3f(1,0,0), vec3f(0,0,1), abs(normal.z) < 0.999);
	let tangent = normalize(cross(up, normal));
	let bitangent = cross(normal, tangent);

	return tangent*local.x + bitangent*local.y + normal*local.z;
}

fn rand(state: ptr<function, u32>) -> f32 {
	(*state) = (*state) * 747796405 + 2891336453;
	let result = (((*state) >> (((*state) >> 28) + 4)) ^ (*state)) * 277803737;
	return f32((result >> 22) ^ result) / 4294967295.0;
}
fn randNormal(state: ptr<function, u32>) -> f32 {
	let theta = 6.28318530718 * rand(state);
	let rho = sqrt(-2 * log(rand(state)));
	return rho * cos(theta);
}
fn randDir(state: ptr<function, u32>) -> vec3f {
    // r^3 ~ U(0, 1)
    let r = pow(rand(state), 0.33333f);
    let cosTheta = 1.0 - 2.0 * rand(state);
    let sinTheta = sqrt(1.0 - cosTheta * cosTheta);
    let phi = 6.28318530718 * rand(state);

    let x = r * sinTheta * cos(phi);
    let y = r * sinTheta * sin(phi);
    let z = cosTheta;

    return vec3(x, y, z);
}
// fn randDir(state: ptr<function, u32>) -> vec3f {
// 	return normalize(vec3f(randNormal(state), randNormal(state), randNormal(state)));
// }

const LIGHT_DIRECTION = vec3f(0.36, 0.80, -0.48);

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) id: vec3u) {
	let dims = textureDimensions(out_image);
	if (id.x >= dims.x || id.y >= dims.y) {
		return;
	}

	let basis = mat3x3f(
		uniforms.camera[0].xyz,
		uniforms.camera[1].xyz,
		uniforms.camera[2].xyz
	);

	let initial_origin = uniforms.camera[3].xyz;
	let initial_dir = normalize(basis * vec3f(f32(id.x), f32(id.y), 1.0));

	// Simple render
	let hit = traverse(initial_origin, initial_dir);
	var color = mix(vec3f(1.0, 1.0, 1.0), vec3f(0.5, 0.7, 1.0), 0.5 + (initial_dir.y + 1.0)); // Air

	if hit.block != 0u {
		color = hit.color * (dot(hit.normal, LIGHT_DIRECTION) * 0.25 + 0.75);
		if traverse(hit.point + hit.normal * 1e-4, LIGHT_DIRECTION).block != 0u {
			color *= 0.5; //shadow
		}
	}

	color = clamp(color, vec3f(0.0), vec3f(1.0)); // clamp

	// Ray-Tracing
	// var seed = id.x + id.y*dims.x;
	// var color = vec3f(0.0);
	// for (var i = 0u; i < 4u; i += 1u) {
	// 	var current_colour = vec3f(1.0);
	// 	var current_light = vec3f(0.0);
	// 	var origin = initial_origin;
	// 	var dir = initial_dir;
	// 	for (var i = 0u; i < 4u; i += 1u) {
	// 		let hit = traverse(origin, dir);
	// 		if hit.block != 0u {
	// 			origin = hit.point + hit.normal * 1e-4;
	// 			dir = normalize(hit.normal + randDir(&seed));
	// 			// dir = reflect(dir, hit.normal);

	// 			let emmision = select(0.0, 5.0, hit.block == 6);
	// 			current_light += emmision * current_colour;
	// 			current_colour = current_colour * hit.color;
	// 		} else {
	// 			let sky_color = mix(vec3f(0.5, 0.7, 1.0), vec3f(1.0, 0.95, 0.8), pow(max(dot(dir, LIGHT_DIRECTION), 0.0), 32.0));
	// 			current_light += sky_color * current_colour;
	// 			break;
	// 		}
	// 	}
	// 	color += current_light * 0.25;
	// }
	// color = clamp((color * (2.51 * color + vec3f(0.03))) / (color * (2.43 * color + vec3f(0.59)) + vec3f(0.14)), vec3f(0.0), vec3f(1.0));
	// color = color / (color + vec3f(1.0));
	// color = (color * (2.51 * color + 0.03)) / (color * (2.43 * color + 0.59) + 0.14); // aces
	
	// Crosshair
	let centerx = dims.x / 2;
	let centery = dims.y / 2;
	if (id.x >= centerx - 2 && id.x <= centerx + 2 && id.y >= centery - 2 && id.y <= centery + 2) {
		color = 1.0 - color;
	}

	textureStore(out_image, vec2i(id.xy), vec4f(color, 1f));
}