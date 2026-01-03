use core::str;
use std::cell::RefCell;
use std::future::IntoFuture;
use std::{borrow::Cow, collections::HashMap, sync::Arc};

#[cfg(not(target_arch = "wasm32"))]
use futures::executor::ThreadPool;
use winit::{
    dpi::PhysicalSize,
    event::{Event, StartCause},
    window::Window,
};

use bytemuck::Pod;
use futures::channel::oneshot;

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

#[derive(Default)]
pub struct Surface {
    surface: Option<wgpu::Surface<'static>>,
    config: Option<wgpu::SurfaceConfiguration>,
}

impl Surface {
    /// Create a new surface wrapper with no surface or configuration.
    pub fn new() -> Self {
        Self {
            surface: None,
            config: None,
        }
    }

    /// Called after the instance is created, but before we request an adapter.
    ///
    /// On wasm, we need to create the surface here, as the WebGL backend needs
    /// a surface (and hence a canvas) to be present to create the adapter.
    ///
    /// We cannot unconditionally create a surface here, as Android requires
    /// us to wait until we receive the `Resumed` event to do so.
    pub fn pre_adapter(&mut self, instance: &wgpu::Instance, window: Arc<Window>) {
        if cfg!(target_arch = "wasm32") {
            self.surface = Some(instance.create_surface(window).unwrap());
        }
    }

    /// Check if the event is the start condition for the surface.
    pub fn start_condition(e: &Event<()>) -> bool {
        match e {
            // On all other platforms, we can create the surface immediately.
            Event::NewEvents(StartCause::Init) => !cfg!(target_os = "android"),
            // On android we need to wait for a resumed event to create the surface.
            Event::Resumed => cfg!(target_os = "android"),
            _ => false,
        }
    }

    pub fn resume(&mut self, context: &Context, window: Arc<Window>, srgb: bool) {
        // Window size is only actually valid after we enter the event loop.
        let window_size = window.inner_size();
        let width = window_size.width.max(1);
        let height = window_size.height.max(1);

        // We didn't create the surface in pre_adapter, so we need to do so now.
        if !cfg!(target_arch = "wasm32") {
            self.surface = Some(context.instance.create_surface(window).unwrap());
        }

        // From here on, self.surface should be Some.

        let surface = self.surface.as_ref().unwrap();

        // Get the default configuration,
        let mut config = surface
            .get_default_config(&context.adapter, width, height)
            .expect("Surface isn't supported by the adapter.");
        if srgb {
            // Not all platforms (WebGPU) support sRGB swapchains, so we need to use view formats
            let view_format = config.format.add_srgb_suffix();
            config.view_formats.push(view_format);
        } else {
            // All platforms support non-sRGB swapchains, so we can just use the format directly.
            let format = config.format.remove_srgb_suffix();
            config.format = format;
            config.view_formats.push(format);
        };
        config.present_mode = wgpu::PresentMode::AutoNoVsync;

        surface.configure(&context.device, &config);
        self.config = Some(config);
    }

    /// Resize the surface, making sure to not resize to zero.
    pub fn resize(&mut self, context: &Context, size: PhysicalSize<u32>) {
        let config = self.config.as_mut().unwrap();
        config.width = size.width.max(1);
        config.height = size.height.max(1);
        let surface = self.surface.as_ref().unwrap();
        surface.configure(&context.device, config);
    }

    /// Acquire the next surface texture.
    pub fn acquire(&mut self, context: &Context) -> Option<wgpu::SurfaceTexture> {
        let surface = self.surface.as_ref()?;

        Some(match surface.get_current_texture() {
            Ok(frame) => frame,
            // If we timed out, just try again
            Err(wgpu::SurfaceError::Timeout) => surface
                .get_current_texture()
                .expect("Failed to acquire next surface texture!"),
            Err(
                // If the surface is outdated, or was lost, reconfigure it.
                wgpu::SurfaceError::Outdated
                | wgpu::SurfaceError::Lost
                // If OutOfMemory happens, reconfiguring may not help, but we might as well try
                | wgpu::SurfaceError::OutOfMemory | wgpu::SurfaceError::Other,
            ) => {
                surface.configure(&context.device, self.config());
                surface
                    .get_current_texture()
                    .expect("Failed to acquire next surface texture!")
            }
        })
    }

    /// On suspend on android, we drop the surface, as it's no longer valid.
    ///
    /// A suspend event is always followed by at least one resume event.
    pub fn suspend(&mut self) {
        if cfg!(target_os = "android") {
            self.surface = None;
        }
    }

    pub fn get(&self) -> Option<&wgpu::Surface<'_>> {
        self.surface.as_ref()
    }

    pub fn config(&self) -> &wgpu::SurfaceConfiguration {
        self.config.as_ref().unwrap()
    }
}

pub struct Context {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

impl Context {
    /// Initialize the WGPU context with the given parameters using a window.
    pub async fn init_with_window(
        surface: &mut Surface,
        window: Arc<Window>,
        optional_features: wgpu::Features,
        mut required_features: wgpu::Features,
        required_downlevel_capabilities: wgpu::DownlevelCapabilities,
        required_limits: wgpu::Limits,
        no_gpu_validation: bool,
    ) -> Self {
        let mut flags = wgpu::InstanceFlags::DEBUG;
        if !no_gpu_validation {
            flags |= wgpu::InstanceFlags::VALIDATION;
        }

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN | wgpu::Backends::BROWSER_WEBGPU,
            flags,
            backend_options: wgpu::BackendOptions::default(),
        });
        surface.pre_adapter(&instance, window);

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: None,
            })
            .await
            .expect("Failed to find suitable GPU adapter.");

        if adapter
            .features()
            .contains(wgpu::Features::TEXTURE_COMPRESSION_BC)
        {
            required_features |= wgpu::Features::TEXTURE_COMPRESSION_BC;
        } else {
            required_features |= wgpu::Features::TEXTURE_COMPRESSION_ETC2;
        }

        let adapter_features = adapter.features();
        assert!(
            adapter_features.contains(required_features),
            "Adapter does not support required features for this example: {:?}",
            required_features - adapter_features
        );

        let downlevel_capabilities = adapter.get_downlevel_capabilities();
        assert!(
            downlevel_capabilities.shader_model >= required_downlevel_capabilities.shader_model,
            "Adapter does not support the minimum shader model required to run this example: {:?}",
            required_downlevel_capabilities.shader_model
        );
        assert!(
            downlevel_capabilities
                .flags
                .contains(required_downlevel_capabilities.flags),
            "Adapter does not support the downlevel capabilities required to run this example: {:?}",
            required_downlevel_capabilities.flags - downlevel_capabilities.flags
        );

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: None,
                required_features: (optional_features & adapter_features) | required_features,
                required_limits,
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::Off,
            })
            .await
            .expect("Unable to find a suitable GPU adapter!");

        Self {
            instance,
            adapter,
            device,
            queue,
        }
    }
}

const BLIT_SHADER_SRC: &str = "
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var result: VertexOutput;
    let x = i32(vertex_index) / 2;
    let y = i32(vertex_index) & 1;
    let tc = vec2<f32>(
        f32(x) * 2.0,
        f32(y) * 2.0
    );
    result.position = vec4<f32>(
        tc.x * 2.0 - 1.0,
        1.0 - tc.y * 2.0,
        0.0, 1.0
    );
    result.tex_coords = tc;
    return result;
}

@group(0)
@binding(0)
var r_color: texture_2d<f32>;

@group(0)
@binding(1)
var r_sampler: sampler;

@fragment
fn fs_main(vertex: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(r_color, r_sampler, vertex.tex_coords);
}
";

pub struct BlitPassParameters<'a> {
    pub src_view: &'a wgpu::TextureView,
    pub dst_view: &'a wgpu::TextureView,
    pub target_format: wgpu::TextureFormat,
    pub blending: Option<f32>,
}

thread_local! {
    static BLIT_PIPELINE: RefCell<Option<Arc<wgpu::RenderPipeline>>> = const { RefCell::new(None) };
}

pub fn encode_blit(
    parameters: &BlitPassParameters,
    device: &wgpu::Device,
    command_encoder: &mut wgpu::CommandEncoder,
    pipeline_database: &mut PipelineDatabase,
) {
    let shader = pipeline_database.shader_from_src(device, BLIT_SHADER_SRC);
    let pipeline = pipeline_database.render_pipeline(
        device,
        wgpu::RenderPipelineDescriptor {
            label: Some(&format!("blit {:?}", parameters.target_format)),
            layout: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(parameters.target_format.into())],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        },
        || {
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("blit"),
                bind_group_layouts: &[&device.create_bind_group_layout(
                    &wgpu::BindGroupLayoutDescriptor {
                        label: None,
                        entries: &[
                            wgpu::BindGroupLayoutEntry {
                                binding: 0,
                                visibility: wgpu::ShaderStages::FRAGMENT,
                                ty: wgpu::BindingType::Texture {
                                    sample_type: wgpu::TextureSampleType::Float {
                                        filterable: true,
                                    },
                                    view_dimension: wgpu::TextureViewDimension::D2,
                                    multisampled: false,
                                },
                                count: None,
                            },
                            wgpu::BindGroupLayoutEntry {
                                binding: 1,
                                visibility: wgpu::ShaderStages::FRAGMENT,
                                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                                count: None,
                            },
                        ],
                    },
                )],
                push_constant_ranges: &[],
            })
        },
    );

    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });

    let bind_group_layout = pipeline.get_bind_group_layout(0);
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(parameters.src_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&sampler),
            },
        ],
    });

    {
        let mut rpass = command_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: parameters.dst_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        rpass.set_pipeline(&pipeline);
        rpass.set_bind_group(0, &bind_group, &[]);
        rpass.draw(0..3, 0..1);
    }
}
