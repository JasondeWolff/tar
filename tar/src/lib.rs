use std::sync::Arc;

use crate::{
    app::{Runtime, Static},
    code_editor::{syntax::Syntax, themes::ColorTheme, CodeEditor},
    egui_util::KeyModifiers,
    time::FpsCounter,
};

pub mod app;
pub mod code_editor;
pub mod egui_util;
pub mod time;
pub mod wgpu_util;

pub struct App {
    fps_counter: FpsCounter,
    code: CodeEditor,
}

// const DEFAULT_CODE: &str = r#"@include tar/common.wgsl

// fn main(tex_coords: vec2f) -> vec4f {
//     let color = vec3f(tex_coords, 0.0);

//     return vec4f(color, 1.0);
// }
// "#;

const DEFAULT_CODE: &str = r#"@include shared/ray.wgsl

const MARCH_RES: u32 = 4;
const MARCH_RES_2: u32 = MARCH_RES * MARCH_RES;

struct Constants {
    view_to_world: mat4x4<f32>,
    clip_to_view: mat4x4<f32>,
    resolution: vec2<u32>,
    near_plane: f32,
    far_plane: f32,
    height_above_sea: f32,
    visibility: f32,
    _padding0: u32,
    _padding1: u32,
}

@group(0)
@binding(0)
var<uniform> constants: Constants;

@group(0)
@binding(1)
var linear_repeat_sampler: sampler;

@group(0)
@binding(2)
var opaque_depth_texture: texture_2d<f32>;

@group(0)
@binding(3)
var src_texture: texture_2d<f32>;

@group(0)
@binding(4)
var dst_texture: texture_storage_2d<rgba16float, write>;

const PLANET_RADIUS: f32 = 6378000.0;
const VISIBLE_ATMOSPHERE_HEIGHT: f32 = 50000.0;
const BETA_R: vec3<f32> = vec3<f32>(5.8e-6, 13.5e-6, 33.1e-6);
const BETA_M: f32 = -log(0.02);

fn rayleigh_phase_function(cos_theta: f32) -> f32 {
    return (3.0 / (16.0 * PI)) * (1.0 + cos_theta * cos_theta);
}

fn mie_phase_function(cos_theta: f32, g: f32) -> f32 {
    let g2: f32 = g * g;
    let denom: f32 = pow(1.0 + g2 - 2.0 * g * cos_theta, 1.5);
    return (1.0 - g2) / (4.0 * PI * denom);
}

fn rayleigh_density_function(height_above_sea: f32) -> f32 {
    return exp(-height_above_sea / 8000.0);
}

fn mie_density_function(height_above_sea: f32) -> f32 {
    return exp(-height_above_sea / 1200.0);
}

fn visibility_function(height_above_sea: f32) -> f32 {
    return mix(constants.visibility, 300000.0, saturate(height_above_sea / 10000.0));
}

fn max_atmosphere_distance(ray: Ray) -> f32 {
    let earth_t: f32 = Ray::intersect_sphere_enter(ray, PLANET_RADIUS);
    if (earth_t != T_MISS) {
        return earth_t;
    }

    return Ray::intersect_sphere_exit(ray, PLANET_RADIUS + VISIBLE_ATMOSPHERE_HEIGHT);
}

fn inscattering(origin: vec3<f32>, l: vec3<f32>) -> vec3<f32> {
    const MAX_STEPS: u32 = 8;

    let ray = Ray::new(origin, l);

    let max_t: f32 = max_atmosphere_distance(ray);
    let step_size: f32 = max_t / f32(MAX_STEPS);

    var optical_depth = vec3<f32>(0.0);
    var t: f32 = 0.0;

    for (var i = 0u; i < MAX_STEPS; i += 1) {
        let p: vec3<f32> = Ray::point(ray, t);
        let height_above_sea: f32 = max(length(p) - PLANET_RADIUS, 0.0);

        let rayleigh_density: f32 = rayleigh_density_function(height_above_sea);
        let mie_density: f32 = mie_density_function(height_above_sea);
        let visibility: f32 = visibility_function(height_above_sea);

        let sigma_rayleigh: vec3<f32> = rayleigh_density * BETA_R;
        let sigma_mie: f32 = mie_density * (BETA_M / (visibility + 0.0001));
        let sigma_total: vec3<f32> = sigma_rayleigh + sigma_mie;

        optical_depth += sigma_total * step_size;

        t += step_size;
    }

    return exp(-optical_depth) * 40.0;
}

fn march(ray: Ray, max_t: array<f32, MARCH_RES_2>, out_scattering: ptr<function, array<vec3<f32>, MARCH_RES_2>>, out_transmission: ptr<function, array<vec3<f32>, MARCH_RES_2>>) {
    const MAX_STEPS: u32 = 16;

    var furthest_max_t: f32 = max_t[0];
    var closest_max_t: f32 = max_t[0];
    var subray_termination_counter: u32 = 0;
    var subray_terminated: array<bool, MARCH_RES_2>;
    for (var i = 0u; i < MARCH_RES_2; i += 1) {
        subray_terminated[i] = false;
        furthest_max_t = max(furthest_max_t, max_t[i]);
        closest_max_t = min(closest_max_t, max_t[i]);
    }

    let step_size: f32 = furthest_max_t / f32(MAX_STEPS);

    let l: vec3<f32> = -normalize(vec3<f32>(0.1, -1.0, 0.2));
    let cos_theta: f32 = dot(ray.direction, l);
    let rayleigh_phase: f32 = rayleigh_phase_function(cos_theta);
    let mie_phase: f32 = mie_phase_function(cos_theta, 0.79);

    var scattering = vec3<f32>(0.0);
    var transmission = vec3<f32>(1.0);

    var t: f32 = 0.0;

    for (var i = 0u; i < MAX_STEPS; i += 1) {
        t += step_size;

        //if (t >= closest_max_t) {
            for (var k = 0u; k < MARCH_RES_2; k += 1) {
                if (t >= max_t[k] && !subray_terminated[k]) {
                    (*out_scattering)[k] = scattering;
                    (*out_transmission)[k] = transmission;

                    subray_terminated[k] = true;
                    subray_termination_counter += 1;
                    if (subray_termination_counter == MARCH_RES_2) {
                        return;
                    }
                }
            }
        //}

        let p: vec3<f32> = Ray::point(ray, t);
        let height_above_sea: f32 = max(length(p) - PLANET_RADIUS, 0.0);

        let rayleigh_density: f32 = rayleigh_density_function(height_above_sea);
        let mie_density: f32 = mie_density_function(height_above_sea);
        let visibility: f32 = visibility_function(height_above_sea);

        let sigma_rayleigh: vec3<f32> = rayleigh_density * BETA_R;
        let sigma_mie: f32 = mie_density * (BETA_M / (visibility + 0.0001));
        let sigma_total: vec3<f32> = sigma_rayleigh + sigma_mie;

        let phase: vec3<f32> = sigma_rayleigh * rayleigh_phase + sigma_mie * mie_phase;

        let inscattering = inscattering(p, l);
        let step_scatter: vec3<f32> = phase * step_size;

        scattering += transmission * step_scatter * inscattering;
        transmission *= exp(-sigma_total * step_size);

        if (length(transmission) <= 0.01) {
            transmission = vec3<f32>(0.0);
            break;
        }
    }

    for (var i = 0u; i < MARCH_RES_2; i += 1) {
        if (!subray_terminated[i]) {
            (*out_scattering)[i] = scattering;
            (*out_transmission)[i] = transmission;
        }
    }
}

@compute
@workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let id: vec2<u32> = global_id.xy;
    if (any(id * MARCH_RES >= constants.resolution)) { return; }

    let pixel_id: vec2<u32> = id * MARCH_RES;
    let uv: vec2<f32> = (vec2<f32>(pixel_id) + 0.5) / vec2<f32>(constants.resolution);
    var ray = Ray::view_ray(uv, constants.view_to_world, constants.clip_to_view);
    ray.origin.y = max(constants.height_above_sea, 1.0) + PLANET_RADIUS;

    let max_t_atmosphere: f32 = max_atmosphere_distance(ray);

    var max_t: array<f32, MARCH_RES_2>;
    for (var y = 0u; y < MARCH_RES; y += 1) {
        for (var x = 0u; x < MARCH_RES; x += 1) {
            let pixel_id: vec2<u32> = id * MARCH_RES + vec2<u32>(x, y);

            let opaque_raw_depth: f32 = textureLoad(opaque_depth_texture, pixel_id, 0).r;
            var sub_max_t: f32 = max_t_atmosphere;
            if (opaque_raw_depth < 1.0) {
                let opaque_depth: f32 = linearize_depth(opaque_raw_depth, constants.near_plane, constants.far_plane);
                sub_max_t = min(sub_max_t, opaque_depth);
            }

            max_t[y * MARCH_RES + x] = sub_max_t;
        }
    }

    var scattering: array<vec3<f32>, MARCH_RES_2>;
    var transmission: array<vec3<f32>, MARCH_RES_2>;
    march(ray, max_t, &scattering, &transmission);

    for (var y = 0u; y < MARCH_RES; y += 1) {
        for (var x = 0u; x < MARCH_RES; x += 1) {
            let pixel_id: vec2<u32> = id * MARCH_RES + vec2<u32>(x, y);

            if (any(pixel_id >= constants.resolution)) {
                continue;
            }

            let i: u32 = y * MARCH_RES + x;

            let opaque: vec3<f32> = textureLoad(src_texture, pixel_id, 0).rgb;
            let result = vec4<f32>(scattering[i] + opaque * transmission[i], 1.0);
            textureStore(dst_texture, pixel_id, result);
        }
    }
}

    const MARCH_RES: u32 = 4;
const MARCH_RES_2: u32 = MARCH_RES * MARCH_RES;

struct Constants {
    view_to_world: mat4x4<f32>,
    clip_to_view: mat4x4<f32>,
    resolution: vec2<u32>,
    near_plane: f32,
    far_plane: f32,
    height_above_sea: f32,
    visibility: f32,
    _padding0: u32,
    _padding1: u32,
}

@group(0)
@binding(0)
var<uniform> constants: Constants;

@group(0)
@binding(1)
var linear_repeat_sampler: sampler;

@group(0)
@binding(2)
var opaque_depth_texture: texture_2d<f32>;

@group(0)
@binding(3)
var src_texture: texture_2d<f32>;

@group(0)
@binding(4)
var dst_texture: texture_storage_2d<rgba16float, write>;

const PLANET_RADIUS: f32 = 6378000.0;
const VISIBLE_ATMOSPHERE_HEIGHT: f32 = 50000.0;
const BETA_R: vec3<f32> = vec3<f32>(5.8e-6, 13.5e-6, 33.1e-6);
const BETA_M: f32 = -log(0.02);

fn rayleigh_phase_function(cos_theta: f32) -> f32 {
    return (3.0 / (16.0 * PI)) * (1.0 + cos_theta * cos_theta);
}

fn mie_phase_function(cos_theta: f32, g: f32) -> f32 {
    let g2: f32 = g * g;
    let denom: f32 = pow(1.0 + g2 - 2.0 * g * cos_theta, 1.5);
    return (1.0 - g2) / (4.0 * PI * denom);
}

fn rayleigh_density_function(height_above_sea: f32) -> f32 {
    return exp(-height_above_sea / 8000.0);
}

fn mie_density_function(height_above_sea: f32) -> f32 {
    return exp(-height_above_sea / 1200.0);
}

fn visibility_function(height_above_sea: f32) -> f32 {
    return mix(constants.visibility, 300000.0, saturate(height_above_sea / 10000.0));
}

fn max_atmosphere_distance(ray: Ray) -> f32 {
    let earth_t: f32 = Ray::intersect_sphere_enter(ray, PLANET_RADIUS);
    if (earth_t != T_MISS) {
        return earth_t;
    }

    return Ray::intersect_sphere_exit(ray, PLANET_RADIUS + VISIBLE_ATMOSPHERE_HEIGHT);
}

fn inscattering(origin: vec3<f32>, l: vec3<f32>) -> vec3<f32> {
    const MAX_STEPS: u32 = 8;

    let ray = Ray::new(origin, l);

    let max_t: f32 = max_atmosphere_distance(ray);
    let step_size: f32 = max_t / f32(MAX_STEPS);

    var optical_depth = vec3<f32>(0.0);
    var t: f32 = 0.0;

    for (var i = 0u; i < MAX_STEPS; i += 1) {
        let p: vec3<f32> = Ray::point(ray, t);
        let height_above_sea: f32 = max(length(p) - PLANET_RADIUS, 0.0);

        let rayleigh_density: f32 = rayleigh_density_function(height_above_sea);
        let mie_density: f32 = mie_density_function(height_above_sea);
        let visibility: f32 = visibility_function(height_above_sea);

        let sigma_rayleigh: vec3<f32> = rayleigh_density * BETA_R;
        let sigma_mie: f32 = mie_density * (BETA_M / (visibility + 0.0001));
        let sigma_total: vec3<f32> = sigma_rayleigh + sigma_mie;

        optical_depth += sigma_total * step_size;

        t += step_size;
    }

    return exp(-optical_depth) * 40.0;
}

fn march(ray: Ray, max_t: array<f32, MARCH_RES_2>, out_scattering: ptr<function, array<vec3<f32>, MARCH_RES_2>>, out_transmission: ptr<function, array<vec3<f32>, MARCH_RES_2>>) {
    const MAX_STEPS: u32 = 16;

    var furthest_max_t: f32 = max_t[0];
    var closest_max_t: f32 = max_t[0];
    var subray_termination_counter: u32 = 0;
    var subray_terminated: array<bool, MARCH_RES_2>;
    for (var i = 0u; i < MARCH_RES_2; i += 1) {
        subray_terminated[i] = false;
        furthest_max_t = max(furthest_max_t, max_t[i]);
        closest_max_t = min(closest_max_t, max_t[i]);
    }

    let step_size: f32 = furthest_max_t / f32(MAX_STEPS);

    let l: vec3<f32> = -normalize(vec3<f32>(0.1, -1.0, 0.2));
    let cos_theta: f32 = dot(ray.direction, l);
    let rayleigh_phase: f32 = rayleigh_phase_function(cos_theta);
    let mie_phase: f32 = mie_phase_function(cos_theta, 0.79);

    var scattering = vec3<f32>(0.0);
    var transmission = vec3<f32>(1.0);

    var t: f32 = 0.0;

    for (var i = 0u; i < MAX_STEPS; i += 1) {
        t += step_size;

        //if (t >= closest_max_t) {
            for (var k = 0u; k < MARCH_RES_2; k += 1) {
                if (t >= max_t[k] && !subray_terminated[k]) {
                    (*out_scattering)[k] = scattering;
                    (*out_transmission)[k] = transmission;

                    subray_terminated[k] = true;
                    subray_termination_counter += 1;
                    if (subray_termination_counter == MARCH_RES_2) {
                        return;
                    }
                }
            }
        //}

        let p: vec3<f32> = Ray::point(ray, t);
        let height_above_sea: f32 = max(length(p) - PLANET_RADIUS, 0.0);

        let rayleigh_density: f32 = rayleigh_density_function(height_above_sea);
        let mie_density: f32 = mie_density_function(height_above_sea);
        let visibility: f32 = visibility_function(height_above_sea);

        let sigma_rayleigh: vec3<f32> = rayleigh_density * BETA_R;
        let sigma_mie: f32 = mie_density * (BETA_M / (visibility + 0.0001));
        let sigma_total: vec3<f32> = sigma_rayleigh + sigma_mie;

        let phase: vec3<f32> = sigma_rayleigh * rayleigh_phase + sigma_mie * mie_phase;

        let inscattering = inscattering(p, l);
        let step_scatter: vec3<f32> = phase * step_size;

        scattering += transmission * step_scatter * inscattering;
        transmission *= exp(-sigma_total * step_size);

        if (length(transmission) <= 0.01) {
            transmission = vec3<f32>(0.0);
            break;
        }
    }

    for (var i = 0u; i < MARCH_RES_2; i += 1) {
        if (!subray_terminated[i]) {
            (*out_scattering)[i] = scattering;
            (*out_transmission)[i] = transmission;
        }
    }
}

@compute
@workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let id: vec2<u32> = global_id.xy;
    if (any(id * MARCH_RES >= constants.resolution)) { return; }

    let pixel_id: vec2<u32> = id * MARCH_RES;
    let uv: vec2<f32> = (vec2<f32>(pixel_id) + 0.5) / vec2<f32>(constants.resolution);
    var ray = Ray::view_ray(uv, constants.view_to_world, constants.clip_to_view);
    ray.origin.y = max(constants.height_above_sea, 1.0) + PLANET_RADIUS;

    let max_t_atmosphere: f32 = max_atmosphere_distance(ray);

    var max_t: array<f32, MARCH_RES_2>;
    for (var y = 0u; y < MARCH_RES; y += 1) {
        for (var x = 0u; x < MARCH_RES; x += 1) {
            let pixel_id: vec2<u32> = id * MARCH_RES + vec2<u32>(x, y);

            let opaque_raw_depth: f32 = textureLoad(opaque_depth_texture, pixel_id, 0).r;
            var sub_max_t: f32 = max_t_atmosphere;
            if (opaque_raw_depth < 1.0) {
                let opaque_depth: f32 = linearize_depth(opaque_raw_depth, constants.near_plane, constants.far_plane);
                sub_max_t = min(sub_max_t, opaque_depth);
            }

            max_t[y * MARCH_RES + x] = sub_max_t;
        }
    }

    var scattering: array<vec3<f32>, MARCH_RES_2>;
    var transmission: array<vec3<f32>, MARCH_RES_2>;
    march(ray, max_t, &scattering, &transmission);

    for (var y = 0u; y < MARCH_RES; y += 1) {
        for (var x = 0u; x < MARCH_RES; x += 1) {
            let pixel_id: vec2<u32> = id * MARCH_RES + vec2<u32>(x, y);

            if (any(pixel_id >= constants.resolution)) {
                continue;
            }

            let i: u32 = y * MARCH_RES + x;

            let opaque: vec3<f32> = textureLoad(src_texture, pixel_id, 0).rgb;
            let result = vec4<f32>(scattering[i] + opaque * transmission[i], 1.0);
            textureStore(dst_texture, pixel_id, result);
        }
    }
}

    const MARCH_RES: u32 = 4;
const MARCH_RES_2: u32 = MARCH_RES * MARCH_RES;

struct Constants {
    view_to_world: mat4x4<f32>,
    clip_to_view: mat4x4<f32>,
    resolution: vec2<u32>,
    near_plane: f32,
    far_plane: f32,
    height_above_sea: f32,
    visibility: f32,
    _padding0: u32,
    _padding1: u32,
}

@group(0)
@binding(0)
var<uniform> constants: Constants;

@group(0)
@binding(1)
var linear_repeat_sampler: sampler;

@group(0)
@binding(2)
var opaque_depth_texture: texture_2d<f32>;

@group(0)
@binding(3)
var src_texture: texture_2d<f32>;

@group(0)
@binding(4)
var dst_texture: texture_storage_2d<rgba16float, write>;

const PLANET_RADIUS: f32 = 6378000.0;
const VISIBLE_ATMOSPHERE_HEIGHT: f32 = 50000.0;
const BETA_R: vec3<f32> = vec3<f32>(5.8e-6, 13.5e-6, 33.1e-6);
const BETA_M: f32 = -log(0.02);

fn rayleigh_phase_function(cos_theta: f32) -> f32 {
    return (3.0 / (16.0 * PI)) * (1.0 + cos_theta * cos_theta);
}

fn mie_phase_function(cos_theta: f32, g: f32) -> f32 {
    let g2: f32 = g * g;
    let denom: f32 = pow(1.0 + g2 - 2.0 * g * cos_theta, 1.5);
    return (1.0 - g2) / (4.0 * PI * denom);
}

fn rayleigh_density_function(height_above_sea: f32) -> f32 {
    return exp(-height_above_sea / 8000.0);
}

fn mie_density_function(height_above_sea: f32) -> f32 {
    return exp(-height_above_sea / 1200.0);
}

fn visibility_function(height_above_sea: f32) -> f32 {
    return mix(constants.visibility, 300000.0, saturate(height_above_sea / 10000.0));
}

fn max_atmosphere_distance(ray: Ray) -> f32 {
    let earth_t: f32 = Ray::intersect_sphere_enter(ray, PLANET_RADIUS);
    if (earth_t != T_MISS) {
        return earth_t;
    }

    return Ray::intersect_sphere_exit(ray, PLANET_RADIUS + VISIBLE_ATMOSPHERE_HEIGHT);
}

fn inscattering(origin: vec3<f32>, l: vec3<f32>) -> vec3<f32> {
    const MAX_STEPS: u32 = 8;

    let ray = Ray::new(origin, l);

    let max_t: f32 = max_atmosphere_distance(ray);
    let step_size: f32 = max_t / f32(MAX_STEPS);

    var optical_depth = vec3<f32>(0.0);
    var t: f32 = 0.0;

    for (var i = 0u; i < MAX_STEPS; i += 1) {
        let p: vec3<f32> = Ray::point(ray, t);
        let height_above_sea: f32 = max(length(p) - PLANET_RADIUS, 0.0);

        let rayleigh_density: f32 = rayleigh_density_function(height_above_sea);
        let mie_density: f32 = mie_density_function(height_above_sea);
        let visibility: f32 = visibility_function(height_above_sea);

        let sigma_rayleigh: vec3<f32> = rayleigh_density * BETA_R;
        let sigma_mie: f32 = mie_density * (BETA_M / (visibility + 0.0001));
        let sigma_total: vec3<f32> = sigma_rayleigh + sigma_mie;

        optical_depth += sigma_total * step_size;

        t += step_size;
    }

    return exp(-optical_depth) * 40.0;
}

fn march(ray: Ray, max_t: array<f32, MARCH_RES_2>, out_scattering: ptr<function, array<vec3<f32>, MARCH_RES_2>>, out_transmission: ptr<function, array<vec3<f32>, MARCH_RES_2>>) {
    const MAX_STEPS: u32 = 16;

    var furthest_max_t: f32 = max_t[0];
    var closest_max_t: f32 = max_t[0];
    var subray_termination_counter: u32 = 0;
    var subray_terminated: array<bool, MARCH_RES_2>;
    for (var i = 0u; i < MARCH_RES_2; i += 1) {
        subray_terminated[i] = false;
        furthest_max_t = max(furthest_max_t, max_t[i]);
        closest_max_t = min(closest_max_t, max_t[i]);
    }

    let step_size: f32 = furthest_max_t / f32(MAX_STEPS);

    let l: vec3<f32> = -normalize(vec3<f32>(0.1, -1.0, 0.2));
    let cos_theta: f32 = dot(ray.direction, l);
    let rayleigh_phase: f32 = rayleigh_phase_function(cos_theta);
    let mie_phase: f32 = mie_phase_function(cos_theta, 0.79);

    var scattering = vec3<f32>(0.0);
    var transmission = vec3<f32>(1.0);

    var t: f32 = 0.0;

    for (var i = 0u; i < MAX_STEPS; i += 1) {
        t += step_size;

        //if (t >= closest_max_t) {
            for (var k = 0u; k < MARCH_RES_2; k += 1) {
                if (t >= max_t[k] && !subray_terminated[k]) {
                    (*out_scattering)[k] = scattering;
                    (*out_transmission)[k] = transmission;

                    subray_terminated[k] = true;
                    subray_termination_counter += 1;
                    if (subray_termination_counter == MARCH_RES_2) {
                        return;
                    }
                }
            }
        //}

        let p: vec3<f32> = Ray::point(ray, t);
        let height_above_sea: f32 = max(length(p) - PLANET_RADIUS, 0.0);

        let rayleigh_density: f32 = rayleigh_density_function(height_above_sea);
        let mie_density: f32 = mie_density_function(height_above_sea);
        let visibility: f32 = visibility_function(height_above_sea);

        let sigma_rayleigh: vec3<f32> = rayleigh_density * BETA_R;
        let sigma_mie: f32 = mie_density * (BETA_M / (visibility + 0.0001));
        let sigma_total: vec3<f32> = sigma_rayleigh + sigma_mie;

        let phase: vec3<f32> = sigma_rayleigh * rayleigh_phase + sigma_mie * mie_phase;

        let inscattering = inscattering(p, l);
        let step_scatter: vec3<f32> = phase * step_size;

        scattering += transmission * step_scatter * inscattering;
        transmission *= exp(-sigma_total * step_size);

        if (length(transmission) <= 0.01) {
            transmission = vec3<f32>(0.0);
            break;
        }
    }

    for (var i = 0u; i < MARCH_RES_2; i += 1) {
        if (!subray_terminated[i]) {
            (*out_scattering)[i] = scattering;
            (*out_transmission)[i] = transmission;
        }
    }
}

@compute
@workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let id: vec2<u32> = global_id.xy;
    if (any(id * MARCH_RES >= constants.resolution)) { return; }

    let pixel_id: vec2<u32> = id * MARCH_RES;
    let uv: vec2<f32> = (vec2<f32>(pixel_id) + 0.5) / vec2<f32>(constants.resolution);
    var ray = Ray::view_ray(uv, constants.view_to_world, constants.clip_to_view);
    ray.origin.y = max(constants.height_above_sea, 1.0) + PLANET_RADIUS;

    let max_t_atmosphere: f32 = max_atmosphere_distance(ray);

    var max_t: array<f32, MARCH_RES_2>;
    for (var y = 0u; y < MARCH_RES; y += 1) {
        for (var x = 0u; x < MARCH_RES; x += 1) {
            let pixel_id: vec2<u32> = id * MARCH_RES + vec2<u32>(x, y);

            let opaque_raw_depth: f32 = textureLoad(opaque_depth_texture, pixel_id, 0).r;
            var sub_max_t: f32 = max_t_atmosphere;
            if (opaque_raw_depth < 1.0) {
                let opaque_depth: f32 = linearize_depth(opaque_raw_depth, constants.near_plane, constants.far_plane);
                sub_max_t = min(sub_max_t, opaque_depth);
            }

            max_t[y * MARCH_RES + x] = sub_max_t;
        }
    }

    var scattering: array<vec3<f32>, MARCH_RES_2>;
    var transmission: array<vec3<f32>, MARCH_RES_2>;
    march(ray, max_t, &scattering, &transmission);

    for (var y = 0u; y < MARCH_RES; y += 1) {
        for (var x = 0u; x < MARCH_RES; x += 1) {
            let pixel_id: vec2<u32> = id * MARCH_RES + vec2<u32>(x, y);

            if (any(pixel_id >= constants.resolution)) {
                continue;
            }

            let i: u32 = y * MARCH_RES + x;

            let opaque: vec3<f32> = textureLoad(src_texture, pixel_id, 0).rgb;
            let result = vec4<f32>(scattering[i] + opaque * transmission[i], 1.0);
            textureStore(dst_texture, pixel_id, result);
        }
    }
}

    const MARCH_RES: u32 = 4;
const MARCH_RES_2: u32 = MARCH_RES * MARCH_RES;

struct Constants {
    view_to_world: mat4x4<f32>,
    clip_to_view: mat4x4<f32>,
    resolution: vec2<u32>,
    near_plane: f32,
    far_plane: f32,
    height_above_sea: f32,
    visibility: f32,
    _padding0: u32,
    _padding1: u32,
}

@group(0)
@binding(0)
var<uniform> constants: Constants;

@group(0)
@binding(1)
var linear_repeat_sampler: sampler;

@group(0)
@binding(2)
var opaque_depth_texture: texture_2d<f32>;

@group(0)
@binding(3)
var src_texture: texture_2d<f32>;

@group(0)
@binding(4)
var dst_texture: texture_storage_2d<rgba16float, write>;

const PLANET_RADIUS: f32 = 6378000.0;
const VISIBLE_ATMOSPHERE_HEIGHT: f32 = 50000.0;
const BETA_R: vec3<f32> = vec3<f32>(5.8e-6, 13.5e-6, 33.1e-6);
const BETA_M: f32 = -log(0.02);

fn rayleigh_phase_function(cos_theta: f32) -> f32 {
    return (3.0 / (16.0 * PI)) * (1.0 + cos_theta * cos_theta);
}

fn mie_phase_function(cos_theta: f32, g: f32) -> f32 {
    let g2: f32 = g * g;
    let denom: f32 = pow(1.0 + g2 - 2.0 * g * cos_theta, 1.5);
    return (1.0 - g2) / (4.0 * PI * denom);
}

fn rayleigh_density_function(height_above_sea: f32) -> f32 {
    return exp(-height_above_sea / 8000.0);
}

fn mie_density_function(height_above_sea: f32) -> f32 {
    return exp(-height_above_sea / 1200.0);
}

fn visibility_function(height_above_sea: f32) -> f32 {
    return mix(constants.visibility, 300000.0, saturate(height_above_sea / 10000.0));
}

fn max_atmosphere_distance(ray: Ray) -> f32 {
    let earth_t: f32 = Ray::intersect_sphere_enter(ray, PLANET_RADIUS);
    if (earth_t != T_MISS) {
        return earth_t;
    }

    return Ray::intersect_sphere_exit(ray, PLANET_RADIUS + VISIBLE_ATMOSPHERE_HEIGHT);
}

fn inscattering(origin: vec3<f32>, l: vec3<f32>) -> vec3<f32> {
    const MAX_STEPS: u32 = 8;

    let ray = Ray::new(origin, l);

    let max_t: f32 = max_atmosphere_distance(ray);
    let step_size: f32 = max_t / f32(MAX_STEPS);

    var optical_depth = vec3<f32>(0.0);
    var t: f32 = 0.0;

    for (var i = 0u; i < MAX_STEPS; i += 1) {
        let p: vec3<f32> = Ray::point(ray, t);
        let height_above_sea: f32 = max(length(p) - PLANET_RADIUS, 0.0);

        let rayleigh_density: f32 = rayleigh_density_function(height_above_sea);
        let mie_density: f32 = mie_density_function(height_above_sea);
        let visibility: f32 = visibility_function(height_above_sea);

        let sigma_rayleigh: vec3<f32> = rayleigh_density * BETA_R;
        let sigma_mie: f32 = mie_density * (BETA_M / (visibility + 0.0001));
        let sigma_total: vec3<f32> = sigma_rayleigh + sigma_mie;

        optical_depth += sigma_total * step_size;

        t += step_size;
    }

    return exp(-optical_depth) * 40.0;
}

fn march(ray: Ray, max_t: array<f32, MARCH_RES_2>, out_scattering: ptr<function, array<vec3<f32>, MARCH_RES_2>>, out_transmission: ptr<function, array<vec3<f32>, MARCH_RES_2>>) {
    const MAX_STEPS: u32 = 16;

    var furthest_max_t: f32 = max_t[0];
    var closest_max_t: f32 = max_t[0];
    var subray_termination_counter: u32 = 0;
    var subray_terminated: array<bool, MARCH_RES_2>;
    for (var i = 0u; i < MARCH_RES_2; i += 1) {
        subray_terminated[i] = false;
        furthest_max_t = max(furthest_max_t, max_t[i]);
        closest_max_t = min(closest_max_t, max_t[i]);
    }

    let step_size: f32 = furthest_max_t / f32(MAX_STEPS);

    let l: vec3<f32> = -normalize(vec3<f32>(0.1, -1.0, 0.2));
    let cos_theta: f32 = dot(ray.direction, l);
    let rayleigh_phase: f32 = rayleigh_phase_function(cos_theta);
    let mie_phase: f32 = mie_phase_function(cos_theta, 0.79);

    var scattering = vec3<f32>(0.0);
    var transmission = vec3<f32>(1.0);

    var t: f32 = 0.0;

    for (var i = 0u; i < MAX_STEPS; i += 1) {
        t += step_size;

        //if (t >= closest_max_t) {
            for (var k = 0u; k < MARCH_RES_2; k += 1) {
                if (t >= max_t[k] && !subray_terminated[k]) {
                    (*out_scattering)[k] = scattering;
                    (*out_transmission)[k] = transmission;

                    subray_terminated[k] = true;
                    subray_termination_counter += 1;
                    if (subray_termination_counter == MARCH_RES_2) {
                        return;
                    }
                }
            }
        //}

        let p: vec3<f32> = Ray::point(ray, t);
        let height_above_sea: f32 = max(length(p) - PLANET_RADIUS, 0.0);

        let rayleigh_density: f32 = rayleigh_density_function(height_above_sea);
        let mie_density: f32 = mie_density_function(height_above_sea);
        let visibility: f32 = visibility_function(height_above_sea);

        let sigma_rayleigh: vec3<f32> = rayleigh_density * BETA_R;
        let sigma_mie: f32 = mie_density * (BETA_M / (visibility + 0.0001));
        let sigma_total: vec3<f32> = sigma_rayleigh + sigma_mie;

        let phase: vec3<f32> = sigma_rayleigh * rayleigh_phase + sigma_mie * mie_phase;

        let inscattering = inscattering(p, l);
        let step_scatter: vec3<f32> = phase * step_size;

        scattering += transmission * step_scatter * inscattering;
        transmission *= exp(-sigma_total * step_size);

        if (length(transmission) <= 0.01) {
            transmission = vec3<f32>(0.0);
            break;
        }
    }

    for (var i = 0u; i < MARCH_RES_2; i += 1) {
        if (!subray_terminated[i]) {
            (*out_scattering)[i] = scattering;
            (*out_transmission)[i] = transmission;
        }
    }
}

@compute
@workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let id: vec2<u32> = global_id.xy;
    if (any(id * MARCH_RES >= constants.resolution)) { return; }

    let pixel_id: vec2<u32> = id * MARCH_RES;
    let uv: vec2<f32> = (vec2<f32>(pixel_id) + 0.5) / vec2<f32>(constants.resolution);
    var ray = Ray::view_ray(uv, constants.view_to_world, constants.clip_to_view);
    ray.origin.y = max(constants.height_above_sea, 1.0) + PLANET_RADIUS;

    let max_t_atmosphere: f32 = max_atmosphere_distance(ray);

    var max_t: array<f32, MARCH_RES_2>;
    for (var y = 0u; y < MARCH_RES; y += 1) {
        for (var x = 0u; x < MARCH_RES; x += 1) {
            let pixel_id: vec2<u32> = id * MARCH_RES + vec2<u32>(x, y);

            let opaque_raw_depth: f32 = textureLoad(opaque_depth_texture, pixel_id, 0).r;
            var sub_max_t: f32 = max_t_atmosphere;
            if (opaque_raw_depth < 1.0) {
                let opaque_depth: f32 = linearize_depth(opaque_raw_depth, constants.near_plane, constants.far_plane);
                sub_max_t = min(sub_max_t, opaque_depth);
            }

            max_t[y * MARCH_RES + x] = sub_max_t;
        }
    }

    var scattering: array<vec3<f32>, MARCH_RES_2>;
    var transmission: array<vec3<f32>, MARCH_RES_2>;
    march(ray, max_t, &scattering, &transmission);

    for (var y = 0u; y < MARCH_RES; y += 1) {
        for (var x = 0u; x < MARCH_RES; x += 1) {
            let pixel_id: vec2<u32> = id * MARCH_RES + vec2<u32>(x, y);

            if (any(pixel_id >= constants.resolution)) {
                continue;
            }

            let i: u32 = y * MARCH_RES + x;

            let opaque: vec3<f32> = textureLoad(src_texture, pixel_id, 0).rgb;
            let result = vec4<f32>(scattering[i] + opaque * transmission[i], 1.0);
            textureStore(dst_texture, pixel_id, result);
        }
    }
}

    const MARCH_RES: u32 = 4;
const MARCH_RES_2: u32 = MARCH_RES * MARCH_RES;

struct Constants {
    view_to_world: mat4x4<f32>,
    clip_to_view: mat4x4<f32>,
    resolution: vec2<u32>,
    near_plane: f32,
    far_plane: f32,
    height_above_sea: f32,
    visibility: f32,
    _padding0: u32,
    _padding1: u32,
}

@group(0)
@binding(0)
var<uniform> constants: Constants;

@group(0)
@binding(1)
var linear_repeat_sampler: sampler;

@group(0)
@binding(2)
var opaque_depth_texture: texture_2d<f32>;

@group(0)
@binding(3)
var src_texture: texture_2d<f32>;

@group(0)
@binding(4)
var dst_texture: texture_storage_2d<rgba16float, write>;

const PLANET_RADIUS: f32 = 6378000.0;
const VISIBLE_ATMOSPHERE_HEIGHT: f32 = 50000.0;
const BETA_R: vec3<f32> = vec3<f32>(5.8e-6, 13.5e-6, 33.1e-6);
const BETA_M: f32 = -log(0.02);

fn rayleigh_phase_function(cos_theta: f32) -> f32 {
    return (3.0 / (16.0 * PI)) * (1.0 + cos_theta * cos_theta);
}

fn mie_phase_function(cos_theta: f32, g: f32) -> f32 {
    let g2: f32 = g * g;
    let denom: f32 = pow(1.0 + g2 - 2.0 * g * cos_theta, 1.5);
    return (1.0 - g2) / (4.0 * PI * denom);
}

fn rayleigh_density_function(height_above_sea: f32) -> f32 {
    return exp(-height_above_sea / 8000.0);
}

fn mie_density_function(height_above_sea: f32) -> f32 {
    return exp(-height_above_sea / 1200.0);
}

fn visibility_function(height_above_sea: f32) -> f32 {
    return mix(constants.visibility, 300000.0, saturate(height_above_sea / 10000.0));
}

fn max_atmosphere_distance(ray: Ray) -> f32 {
    let earth_t: f32 = Ray::intersect_sphere_enter(ray, PLANET_RADIUS);
    if (earth_t != T_MISS) {
        return earth_t;
    }

    return Ray::intersect_sphere_exit(ray, PLANET_RADIUS + VISIBLE_ATMOSPHERE_HEIGHT);
}

fn inscattering(origin: vec3<f32>, l: vec3<f32>) -> vec3<f32> {
    const MAX_STEPS: u32 = 8;

    let ray = Ray::new(origin, l);

    let max_t: f32 = max_atmosphere_distance(ray);
    let step_size: f32 = max_t / f32(MAX_STEPS);

    var optical_depth = vec3<f32>(0.0);
    var t: f32 = 0.0;

    for (var i = 0u; i < MAX_STEPS; i += 1) {
        let p: vec3<f32> = Ray::point(ray, t);
        let height_above_sea: f32 = max(length(p) - PLANET_RADIUS, 0.0);

        let rayleigh_density: f32 = rayleigh_density_function(height_above_sea);
        let mie_density: f32 = mie_density_function(height_above_sea);
        let visibility: f32 = visibility_function(height_above_sea);

        let sigma_rayleigh: vec3<f32> = rayleigh_density * BETA_R;
        let sigma_mie: f32 = mie_density * (BETA_M / (visibility + 0.0001));
        let sigma_total: vec3<f32> = sigma_rayleigh + sigma_mie;

        optical_depth += sigma_total * step_size;

        t += step_size;
    }

    return exp(-optical_depth) * 40.0;
}

fn march(ray: Ray, max_t: array<f32, MARCH_RES_2>, out_scattering: ptr<function, array<vec3<f32>, MARCH_RES_2>>, out_transmission: ptr<function, array<vec3<f32>, MARCH_RES_2>>) {
    const MAX_STEPS: u32 = 16;

    var furthest_max_t: f32 = max_t[0];
    var closest_max_t: f32 = max_t[0];
    var subray_termination_counter: u32 = 0;
    var subray_terminated: array<bool, MARCH_RES_2>;
    for (var i = 0u; i < MARCH_RES_2; i += 1) {
        subray_terminated[i] = false;
        furthest_max_t = max(furthest_max_t, max_t[i]);
        closest_max_t = min(closest_max_t, max_t[i]);
    }

    let step_size: f32 = furthest_max_t / f32(MAX_STEPS);

    let l: vec3<f32> = -normalize(vec3<f32>(0.1, -1.0, 0.2));
    let cos_theta: f32 = dot(ray.direction, l);
    let rayleigh_phase: f32 = rayleigh_phase_function(cos_theta);
    let mie_phase: f32 = mie_phase_function(cos_theta, 0.79);

    var scattering = vec3<f32>(0.0);
    var transmission = vec3<f32>(1.0);

    var t: f32 = 0.0;

    for (var i = 0u; i < MAX_STEPS; i += 1) {
        t += step_size;

        //if (t >= closest_max_t) {
            for (var k = 0u; k < MARCH_RES_2; k += 1) {
                if (t >= max_t[k] && !subray_terminated[k]) {
                    (*out_scattering)[k] = scattering;
                    (*out_transmission)[k] = transmission;

                    subray_terminated[k] = true;
                    subray_termination_counter += 1;
                    if (subray_termination_counter == MARCH_RES_2) {
                        return;
                    }
                }
            }
        //}

        let p: vec3<f32> = Ray::point(ray, t);
        let height_above_sea: f32 = max(length(p) - PLANET_RADIUS, 0.0);

        let rayleigh_density: f32 = rayleigh_density_function(height_above_sea);
        let mie_density: f32 = mie_density_function(height_above_sea);
        let visibility: f32 = visibility_function(height_above_sea);

        let sigma_rayleigh: vec3<f32> = rayleigh_density * BETA_R;
        let sigma_mie: f32 = mie_density * (BETA_M / (visibility + 0.0001));
        let sigma_total: vec3<f32> = sigma_rayleigh + sigma_mie;

        let phase: vec3<f32> = sigma_rayleigh * rayleigh_phase + sigma_mie * mie_phase;

        let inscattering = inscattering(p, l);
        let step_scatter: vec3<f32> = phase * step_size;

        scattering += transmission * step_scatter * inscattering;
        transmission *= exp(-sigma_total * step_size);

        if (length(transmission) <= 0.01) {
            transmission = vec3<f32>(0.0);
            break;
        }
    }

    for (var i = 0u; i < MARCH_RES_2; i += 1) {
        if (!subray_terminated[i]) {
            (*out_scattering)[i] = scattering;
            (*out_transmission)[i] = transmission;
        }
    }
}

@compute
@workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let id: vec2<u32> = global_id.xy;
    if (any(id * MARCH_RES >= constants.resolution)) { return; }

    let pixel_id: vec2<u32> = id * MARCH_RES;
    let uv: vec2<f32> = (vec2<f32>(pixel_id) + 0.5) / vec2<f32>(constants.resolution);
    var ray = Ray::view_ray(uv, constants.view_to_world, constants.clip_to_view);
    ray.origin.y = max(constants.height_above_sea, 1.0) + PLANET_RADIUS;

    let max_t_atmosphere: f32 = max_atmosphere_distance(ray);

    var max_t: array<f32, MARCH_RES_2>;
    for (var y = 0u; y < MARCH_RES; y += 1) {
        for (var x = 0u; x < MARCH_RES; x += 1) {
            let pixel_id: vec2<u32> = id * MARCH_RES + vec2<u32>(x, y);

            let opaque_raw_depth: f32 = textureLoad(opaque_depth_texture, pixel_id, 0).r;
            var sub_max_t: f32 = max_t_atmosphere;
            if (opaque_raw_depth < 1.0) {
                let opaque_depth: f32 = linearize_depth(opaque_raw_depth, constants.near_plane, constants.far_plane);
                sub_max_t = min(sub_max_t, opaque_depth);
            }

            max_t[y * MARCH_RES + x] = sub_max_t;
        }
    }

    var scattering: array<vec3<f32>, MARCH_RES_2>;
    var transmission: array<vec3<f32>, MARCH_RES_2>;
    march(ray, max_t, &scattering, &transmission);

    for (var y = 0u; y < MARCH_RES; y += 1) {
        for (var x = 0u; x < MARCH_RES; x += 1) {
            let pixel_id: vec2<u32> = id * MARCH_RES + vec2<u32>(x, y);

            if (any(pixel_id >= constants.resolution)) {
                continue;
            }

            let i: u32 = y * MARCH_RES + x;

            let opaque: vec3<f32> = textureLoad(src_texture, pixel_id, 0).rgb;
            let result = vec4<f32>(scattering[i] + opaque * transmission[i], 1.0);
            textureStore(dst_texture, pixel_id, result);
        }
    }
}
"#;

impl App {
    fn new() -> Self {
        Self {
            fps_counter: FpsCounter::new(),
            code: CodeEditor::new(DEFAULT_CODE, ColorTheme::GITHUB_DARK, Syntax::wgsl()),
        }
    }
}

pub struct RenderPipeline {}

impl app::RenderPipeline<App> for RenderPipeline {
    fn required_limits() -> wgpu::Limits {
        wgpu::Limits {
            max_texture_dimension_2d: 1024 * 8,
            ..wgpu::Limits::downlevel_defaults()
        }
    }

    fn init(
        _config: wgpu::SurfaceConfiguration,
        _adapter: &wgpu::Adapter,
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
        _window: Arc<winit::window::Window>,
    ) -> Self {
        Self {}
    }

    fn resize(
        &mut self,
        _config: wgpu::SurfaceConfiguration,
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
    ) {
    }

    fn render(
        &mut self,
        _target_view: &wgpu::TextureView,
        _target_format: wgpu::TextureFormat,
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
        egui_ctx: &mut egui::Context,
        key_modifiers: &KeyModifiers,
        app: &mut App,
    ) {
        if app.fps_counter.update() {
            log::info!(
                "FPS {} (ms {:.2})",
                app.fps_counter.fps(),
                app.fps_counter.ms()
            );
        }

        egui::CentralPanel::default().show(egui_ctx, |ui| {
            app.code.ui(ui, key_modifiers);
        });
    }
}

pub fn internal_main(#[cfg(target_os = "android")] android_app: android_activity::AndroidApp) {
    Static::init();

    let app = App::new();

    Runtime::new(app).run::<RenderPipeline>(
        #[cfg(target_os = "android")]
        android_app,
    );
}
