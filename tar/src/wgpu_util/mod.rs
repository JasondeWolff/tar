use core::str;
use std::cell::RefCell;
use std::future::IntoFuture;
use std::{borrow::Cow, collections::HashMap, sync::Arc};

#[cfg(not(target_arch = "wasm32"))]
use futures::executor::ThreadPool;

use bytemuck::Pod;
use futures::channel::oneshot;

pub mod blit_pass;
pub mod context_wrapper;
pub mod surface_wrapper;

pub trait ComputePipelineDescriptorExtensions<'a> {
    fn partial_default(module: &'a wgpu::ShaderModule) -> Self;
}

impl<'a> ComputePipelineDescriptorExtensions<'a> for wgpu::ComputePipelineDescriptor<'a> {
    fn partial_default(module: &'a wgpu::ShaderModule) -> Self {
        wgpu::ComputePipelineDescriptor {
            label: None,
            layout: None,
            module,
            entry_point: None,
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        }
    }
}

/// Read back the contents of a staging buffer asynchronously.
async fn readback_buffer_async<T: Pod>(staging_buffer: wgpu::Buffer) -> Vec<T> {
    let buffer_slice = staging_buffer.slice(..);
    let (sender, receiver) = oneshot::channel();
    buffer_slice.map_async(wgpu::MapMode::Read, move |v| sender.send(v).unwrap());

    receiver.into_future().await.unwrap().unwrap();

    let data = buffer_slice.get_mapped_range();
    let result = bytemuck::cast_slice(&data).to_vec();
    drop(data);
    staging_buffer.unmap();
    result
}

/// Read back the contents of a staging buffer asynchronously.
pub struct BufferReadback {
    #[cfg(not(target_arch = "wasm32"))]
    thread_pool: ThreadPool,
}

impl Default for BufferReadback {
    fn default() -> Self {
        Self::new()
    }
}

impl BufferReadback {
    pub fn new() -> Self {
        Self {
            #[cfg(not(target_arch = "wasm32"))]
            thread_pool: ThreadPool::new().unwrap(),
        }
    }

    pub fn readback<T: Pod + Send>(
        &self,
        buffer: &wgpu::Buffer,
        size: u64,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> oneshot::Receiver<Vec<T>> {
        let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        encoder.copy_buffer_to_buffer(buffer, 0, &staging_buffer, 0, size);
        queue.submit(Some(encoder.finish()));

        cfg_if::cfg_if! {
            if #[cfg(target_arch = "wasm32")] {
                let (sender, receiver) = oneshot::channel::<Vec<T>>();

                wasm_bindgen_futures::spawn_local(async move {
                    let data = readback_buffer_async(
                        staging_buffer,
                    ).await;
                    let _ = sender.send(data);
                });

                receiver
            } else {
                let (sender, receiver) = oneshot::channel::<Vec<T>>();

                self.thread_pool.spawn_ok(async move {
                    let data = readback_buffer_async(
                        staging_buffer,
                    ).await;
                    let _ = sender.send(data);
                });

                if device.poll(wgpu::PollType::Wait).is_err() {
                    panic!("Failed to readback buffer");
                }

                receiver
            }
        }
    }
}

thread_local! {
    static EMPTY_TEXTURE_VIEW: RefCell<Option<wgpu::TextureView>> = const { RefCell::new(None) };
    static EMPTY_CUBE_TEXTURE_VIEW: RefCell<Option<wgpu::TextureView>> = const { RefCell::new(None) };
}

/// Create an empty texture view that can be used as a placeholder in bind groups.
pub fn empty_texture_view(device: &wgpu::Device) -> wgpu::TextureView {
    EMPTY_TEXTURE_VIEW.with(|v| {
        let mut v = v.borrow_mut();
        if v.is_none() {
            *v = Some({
                let texture = device.create_texture(&wgpu::TextureDescriptor {
                    size: wgpu::Extent3d {
                        width: 1,
                        height: 1,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING,
                    label: Some("Empty"),
                    view_formats: &[],
                });
                texture.create_view(&wgpu::TextureViewDescriptor {
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    ..Default::default()
                })
            })
        }
        v.as_ref().unwrap().clone()
    })
}

/// Create an empty cube texture view that can be used as a placeholder in bind groups.
pub fn empty_cube_texture_view(device: &wgpu::Device) -> wgpu::TextureView {
    EMPTY_CUBE_TEXTURE_VIEW.with(|v| {
        let mut v = v.borrow_mut();
        if v.is_none() {
            *v = Some({
                let texture = device.create_texture(&wgpu::TextureDescriptor {
                    size: wgpu::Extent3d {
                        width: 1,
                        height: 1,
                        depth_or_array_layers: 6,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING,
                    label: Some("Empty"),
                    view_formats: &[],
                });
                texture.create_view(&wgpu::TextureViewDescriptor {
                    dimension: Some(wgpu::TextureViewDimension::Cube),
                    ..Default::default()
                })
            })
        }
        v.as_ref().unwrap().clone()
    })
}

thread_local! {
    static EMPTY_BIND_GROUP_LAYOUT: RefCell<Option<wgpu::BindGroupLayout>> = const { RefCell::new(None) };
}

/// Create an empty bind group layout that can be used as a placeholder in bind groups.
pub fn empty_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    EMPTY_BIND_GROUP_LAYOUT.with(|v| {
        let mut v = v.borrow_mut();
        if v.is_none() {
            *v = Some({
                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: None,
                    entries: &[],
                })
            })
        }
        v.as_ref().unwrap().clone()
    })
}

thread_local! {
    static EMPTY_BIND_GROUP: RefCell<Option<wgpu::BindGroup>> = const { RefCell::new(None) };
}

/// Create an empty bind group that can be used as a placeholder in pipelines.
pub fn empty_bind_group(device: &wgpu::Device) -> wgpu::BindGroup {
    EMPTY_BIND_GROUP.with(|v| {
        let mut v = v.borrow_mut();
        if v.is_none() {
            *v = Some({
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: None,
                    layout: &empty_bind_group_layout(device),
                    entries: &[],
                })
            })
        }
        v.as_ref().unwrap().clone()
    })
}

#[repr(C)]
#[derive(
    Default,
    Copy,
    Clone,
    Debug,
    Hash,
    Eq,
    PartialEq,
    serde::Serialize,
    serde::Deserialize,
    strum::EnumIter,
    strum::Display,
)]
pub enum BasicColorTextureFormat {
    // Normal 8 bit formats
    /// Red channel only. 8 bit integer per channel. [0, 255] converted to/from float [0, 1] in shader.
    R8Unorm,
    /// Red channel only. 8 bit integer per channel. [&minus;127, 127] converted to/from float [&minus;1, 1] in shader.
    R8Snorm,
    /// Red channel only. 8 bit integer per channel. Unsigned in shader.
    R8Uint,
    /// Red channel only. 8 bit integer per channel. Signed in shader.
    R8Sint,

    // Normal 16 bit formats
    /// Red channel only. 16 bit integer per channel. Unsigned in shader.
    R16Uint,
    /// Red channel only. 16 bit integer per channel. Signed in shader.
    R16Sint,
    /// Red channel only. 16 bit float per channel. Float in shader.
    R16Float,
    /// Red and green channels. 8 bit integer per channel. [0, 255] converted to/from float [0, 1] in shader.
    Rg8Unorm,
    /// Red and green channels. 8 bit integer per channel. [&minus;127, 127] converted to/from float [&minus;1, 1] in shader.
    Rg8Snorm,
    /// Red and green channels. 8 bit integer per channel. Unsigned in shader.
    Rg8Uint,
    /// Red and green channels. 8 bit integer per channel. Signed in shader.
    Rg8Sint,

    // Normal 32 bit formats
    /// Red channel only. 32 bit integer per channel. Unsigned in shader.
    R32Uint,
    /// Red channel only. 32 bit integer per channel. Signed in shader.
    R32Sint,
    /// Red channel only. 32 bit float per channel. Float in shader.
    R32Float,
    /// Red and green channels. 16 bit integer per channel. Unsigned in shader.
    Rg16Uint,
    /// Red and green channels. 16 bit integer per channel. Signed in shader.
    Rg16Sint,
    /// Red and green channels. 16 bit float per channel. Float in shader.
    Rg16Float,
    /// Red, green, blue, and alpha channels. 8 bit integer per channel. [0, 255] converted to/from float [0, 1] in shader.
    Rgba8Unorm,
    /// Red, green, blue, and alpha channels. 8 bit integer per channel. Srgb-color [0, 255] converted to/from linear-color float [0, 1] in shader.
    Rgba8UnormSrgb,
    /// Red, green, blue, and alpha channels. 8 bit integer per channel. [&minus;127, 127] converted to/from float [&minus;1, 1] in shader.
    Rgba8Snorm,
    /// Red, green, blue, and alpha channels. 8 bit integer per channel. Unsigned in shader.
    Rgba8Uint,
    /// Red, green, blue, and alpha channels. 8 bit integer per channel. Signed in shader.
    Rgba8Sint,
    /// Blue, green, red, and alpha channels. 8 bit integer per channel. [0, 255] converted to/from float [0, 1] in shader.
    Bgra8Unorm,
    /// Blue, green, red, and alpha channels. 8 bit integer per channel. Srgb-color [0, 255] converted to/from linear-color float [0, 1] in shader.
    Bgra8UnormSrgb,

    // Packed 32 bit formats
    /// Packed unsigned float with 9 bits mantisa for each RGB component, then a common 5 bits exponent
    Rgb9e5Ufloat,
    /// Red, green, blue, and alpha channels. 10 bit integer for RGB channels, 2 bit integer for alpha channel. Unsigned in shader.
    Rgb10a2Uint,
    /// Red, green, blue, and alpha channels. 10 bit integer for RGB channels, 2 bit integer for alpha channel. [0, 1023] ([0, 3] for alpha) converted to/from float [0, 1] in shader.
    Rgb10a2Unorm,
    /// Red, green, and blue channels. 11 bit float with no sign bit for RG channels. 10 bit float with no sign bit for blue channel. Float in shader.
    Rg11b10Ufloat,

    // Normal 64 bit formats
    /// Red and green channels. 32 bit integer per channel. Unsigned in shader.
    Rg32Uint,
    /// Red and green channels. 32 bit integer per channel. Signed in shader.
    Rg32Sint,
    /// Red and green channels. 32 bit float per channel. Float in shader.
    Rg32Float,
    /// Red, green, blue, and alpha channels. 16 bit integer per channel. Unsigned in shader.
    Rgba16Uint,
    /// Red, green, blue, and alpha channels. 16 bit integer per channel. Signed in shader.
    Rgba16Sint,
    /// Red, green, blue, and alpha channels. 16 bit float per channel. Float in shader.
    #[default]
    Rgba16Float,

    // Normal 128 bit formats
    /// Red, green, blue, and alpha channels. 32 bit integer per channel. Unsigned in shader.
    Rgba32Uint,
    /// Red, green, blue, and alpha channels. 32 bit integer per channel. Signed in shader.
    Rgba32Sint,
    /// Red, green, blue, and alpha channels. 32 bit float per channel. Float in shader.
    Rgba32Float,
}

impl From<BasicColorTextureFormat> for wgpu::TextureFormat {
    fn from(format: BasicColorTextureFormat) -> Self {
        match format {
            BasicColorTextureFormat::R8Unorm => wgpu::TextureFormat::R8Unorm,
            BasicColorTextureFormat::R8Snorm => wgpu::TextureFormat::R8Snorm,
            BasicColorTextureFormat::R8Uint => wgpu::TextureFormat::R8Uint,
            BasicColorTextureFormat::R8Sint => wgpu::TextureFormat::R8Sint,
            BasicColorTextureFormat::R16Uint => wgpu::TextureFormat::R16Uint,
            BasicColorTextureFormat::R16Sint => wgpu::TextureFormat::R16Sint,
            BasicColorTextureFormat::R16Float => wgpu::TextureFormat::R16Float,
            BasicColorTextureFormat::Rg8Unorm => wgpu::TextureFormat::Rg8Unorm,
            BasicColorTextureFormat::Rg8Snorm => wgpu::TextureFormat::Rg8Snorm,
            BasicColorTextureFormat::Rg8Uint => wgpu::TextureFormat::Rg8Uint,
            BasicColorTextureFormat::Rg8Sint => wgpu::TextureFormat::Rg8Sint,
            BasicColorTextureFormat::R32Uint => wgpu::TextureFormat::R32Uint,
            BasicColorTextureFormat::R32Sint => wgpu::TextureFormat::R32Sint,
            BasicColorTextureFormat::R32Float => wgpu::TextureFormat::R32Float,
            BasicColorTextureFormat::Rg16Uint => wgpu::TextureFormat::Rg16Uint,
            BasicColorTextureFormat::Rg16Sint => wgpu::TextureFormat::Rg16Sint,
            BasicColorTextureFormat::Rg16Float => wgpu::TextureFormat::Rg16Float,
            BasicColorTextureFormat::Rgba8Unorm => wgpu::TextureFormat::Rgba8Unorm,
            BasicColorTextureFormat::Rgba8UnormSrgb => wgpu::TextureFormat::Rgba8UnormSrgb,
            BasicColorTextureFormat::Rgba8Snorm => wgpu::TextureFormat::Rgba8Snorm,
            BasicColorTextureFormat::Rgba8Uint => wgpu::TextureFormat::Rgba8Uint,
            BasicColorTextureFormat::Rgba8Sint => wgpu::TextureFormat::Rgba8Sint,
            BasicColorTextureFormat::Bgra8Unorm => wgpu::TextureFormat::Bgra8Unorm,
            BasicColorTextureFormat::Bgra8UnormSrgb => wgpu::TextureFormat::Bgra8UnormSrgb,
            BasicColorTextureFormat::Rgb9e5Ufloat => wgpu::TextureFormat::Rgb9e5Ufloat,
            BasicColorTextureFormat::Rgb10a2Uint => wgpu::TextureFormat::Rgb10a2Uint,
            BasicColorTextureFormat::Rgb10a2Unorm => wgpu::TextureFormat::Rgb10a2Unorm,
            BasicColorTextureFormat::Rg11b10Ufloat => wgpu::TextureFormat::Rg11b10Ufloat,
            BasicColorTextureFormat::Rg32Uint => wgpu::TextureFormat::Rg32Uint,
            BasicColorTextureFormat::Rg32Sint => wgpu::TextureFormat::Rg32Sint,
            BasicColorTextureFormat::Rg32Float => wgpu::TextureFormat::Rg32Float,
            BasicColorTextureFormat::Rgba16Uint => wgpu::TextureFormat::Rgba16Uint,
            BasicColorTextureFormat::Rgba16Sint => wgpu::TextureFormat::Rgba16Sint,
            BasicColorTextureFormat::Rgba16Float => wgpu::TextureFormat::Rgba16Float,
            BasicColorTextureFormat::Rgba32Uint => wgpu::TextureFormat::Rgba32Uint,
            BasicColorTextureFormat::Rgba32Sint => wgpu::TextureFormat::Rgba32Sint,
            BasicColorTextureFormat::Rgba32Float => wgpu::TextureFormat::Rgba32Float,
        }
    }
}
