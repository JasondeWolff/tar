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

/// A database to store and manage shader modules and pipelines. All objects are cached to avoid unecessary shader rebuilds.
pub struct PipelineDatabase {
    shader_modules: HashMap<String, Arc<wgpu::ShaderModule>>,
    render_pipelines: HashMap<String, Arc<wgpu::RenderPipeline>>,
    compute_pipelines: HashMap<String, Arc<wgpu::ComputePipeline>>,
}

impl Default for PipelineDatabase {
    fn default() -> Self {
        Self::new()
    }
}

impl PipelineDatabase {
    pub fn new() -> Self {
        Self {
            shader_modules: HashMap::new(),
            render_pipelines: HashMap::new(),
            compute_pipelines: HashMap::new(),
        }
    }

    /// Create a shader module from a WGSL source string.
    pub fn shader_from_src(&mut self, device: &wgpu::Device, src: &str) -> Arc<wgpu::ShaderModule> {
        if let Some(module) = self.shader_modules.get(src) {
            return module.clone();
        }

        let module = Arc::new(device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(src),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(src)),
        }));

        self.shader_modules.insert(src.to_owned(), module.clone());
        module
    }

    /// Create a new render pipeline, caching it in the database. This won't create a new pipeline if one with the same label already exists.
    pub fn render_pipeline<F>(
        &mut self,
        device: &wgpu::Device,
        descriptor: wgpu::RenderPipelineDescriptor,
        create_layout_fn: F,
    ) -> Arc<wgpu::RenderPipeline>
    where
        F: Fn() -> wgpu::PipelineLayout,
    {
        let entry = descriptor
            .label
            .expect("Every pipeline must contain a label!");
        if let Some(pipeline) = self.render_pipelines.get(entry) {
            return pipeline.clone();
        }

        let pipeline_layout = create_layout_fn();
        let descriptor = wgpu::RenderPipelineDescriptor {
            layout: Some(&pipeline_layout),
            ..descriptor
        };

        let pipeline = Arc::new(device.create_render_pipeline(&descriptor));

        self.render_pipelines
            .insert(entry.to_owned(), pipeline.clone());
        pipeline
    }

    /// Create a new compute pipeline, caching it in the database. This won't create a new pipeline if one with the same label already exists.
    pub fn compute_pipeline<F>(
        &mut self,
        device: &wgpu::Device,
        descriptor: wgpu::ComputePipelineDescriptor,
        create_layout_fn: F,
    ) -> Arc<wgpu::ComputePipeline>
    where
        F: Fn() -> wgpu::PipelineLayout,
    {
        let entry = descriptor
            .label
            .expect("Every pipeline must contain a label!");
        if let Some(pipeline) = self.compute_pipelines.get(entry) {
            return pipeline.clone();
        }

        let pipeline_layout = create_layout_fn();
        let descriptor = wgpu::ComputePipelineDescriptor {
            layout: Some(&pipeline_layout),
            ..descriptor
        };

        let pipeline = Arc::new(device.create_compute_pipeline(&descriptor));

        self.compute_pipelines
            .insert(entry.to_owned(), pipeline.clone());
        pipeline
    }
}

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
