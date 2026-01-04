use log::{Level, LevelFilter};
use std::{marker::PhantomData, sync::Arc};
use winit::{
    error::EventLoopError,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

use crate::{
    egui_util::{self, EguiPass},
    wgpu_util::{
        blit_pass::{encode_blit, BlitPassParameters},
        context_wrapper::ContextWrapper,
        surface_wrapper::SurfaceWrapper,
        PipelineDatabase,
    },
};

use time::{macros::format_description, OffsetDateTime};

static STATIC: std::sync::OnceLock<Static> = std::sync::OnceLock::new();

#[cfg(not(target_arch = "wasm32"))]
#[derive(clap::Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Forcefully disable gpu validation
    #[arg(long, default_value_t = false)]
    no_gpu_validation: bool,
}

pub struct Static {
    // TODO: Sentry telemetry
}

impl Static {
    pub fn init() -> &'static Self {
        STATIC.get_or_init(|| {
            // let base_level = log::LevelFilter::Info;
            // let wgpu_level = log::LevelFilter::Info;

            // cfg_if::cfg_if! {
            //     if #[cfg(target_arch = "wasm32")] {
            //         fern::Dispatch::new()
            //             .level(base_level)
            //             .level_for("wgpu_core", wgpu_level)
            //             .level_for("wgpu_hal", wgpu_level)
            //             .level_for("naga", wgpu_level)
            //             .chain(fern::Output::call(console_log::log))
            //             .apply()
            //             .unwrap();

            //         std::panic::set_hook(Box::new(console_error_panic_hook::hook));
            //     } else if #[cfg(target_os = "android")] {
            //         use log::LevelFilter;
            //         use android_logger::{Config,FilterBuilder};

            //         android_logger::init_once(
            //             Config::default()
            //                 .with_max_level(base_level)
            //         );
            //     } else {
            //         env_logger::builder()
            //             .filter_level(base_level)
            //             .filter_module("wgpu_core", wgpu_level)
            //             .filter_module("wgpu_hal", wgpu_level)
            //             .filter_module("naga", wgpu_level)
            //             .parse_default_env()
            //             .init();
            //     }
            // }

            #[cfg(windows)]
            let _ = ansi_term::enable_ansi_support();

            let mut fern_init = fern::Dispatch::new()
                .level_for("wgpu_hal", LevelFilter::Error)
                .filter(|metadata| metadata.level() <= LevelFilter::Info)
                .format(|out, message, record| {
                    if cfg!(target_os = "android") {
                        out.finish(format_args!("{}", message))
                    } else {
                        let level = match record.level() {
                            Level::Debug | Level::Trace => {
                                ansi_term::Colour::Blue.paint(record.level().as_str())
                            }
                            Level::Info => ansi_term::Colour::Green.paint(record.level().as_str()),
                            Level::Warn => ansi_term::Colour::Yellow.paint(record.level().as_str()),
                            Level::Error => ansi_term::Colour::Red.paint(record.level().as_str()),
                        };

                        let pretty_date_format = format_description!(
                            "[[[year]-[month]-[day]][[[hour]:[minute]:[second]]"
                        );

                        out.finish(format_args!(
                            "{}[{}][{}] {}",
                            OffsetDateTime::now_local()
                                .unwrap()
                                .format(&pretty_date_format)
                                .unwrap(),
                            record.target(),
                            level,
                            message
                        ))
                    }
                });

            #[cfg(target_os = "android")]
            {
                fern_init = fern_init.chain(Box::new(android_logger::AndroidLogger::new(
                    android_logger::Config::default(),
                )) as Box<dyn log::Log>);
            }

            // android_logger already prints to logcat, and android-activity forwards stdout/stderr to logcat
            // (so dbg!/println! messages for simple apps don't get lost). Chaining to stdout() would
            // make everything appear twice.
            if cfg!(not(target_os = "android")) {
                fern_init = fern_init.chain(std::io::stdout());
            }

            fern_init.apply().expect("A logger has already been set!");
            panic_log::initialize_hook(panic_log::Configuration {
                force_capture: true,
                ..Default::default()
            });

            Self {}
        })
    }
}

pub struct Runtime<A> {
    #[cfg(not(target_arch = "wasm32"))]
    args: Args,
    user_app: A,
}

impl<A> Runtime<A> {
    pub fn new(user_app: A) -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        let args = {
            use clap::Parser;
            Args::parse()
        };

        Self {
            #[cfg(not(target_arch = "wasm32"))]
            args,
            user_app,
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn run<R: RenderPipeline<A>>(
        self,
        #[cfg(target_os = "android")] android_app: android_activity::AndroidApp,
    ) {
        let render_loop_handler = ApplicationHandler::<A, R>::new(self);
        render_loop_handler
            .run(
                #[cfg(target_os = "android")]
                android_app,
            )
            .unwrap()
    }

    #[cfg(target_arch = "wasm32")]
    pub async fn run<R: RenderPipeline<A>>(self) {
        let render_loop_handler = ApplicationHandler::<A, R>::new(self);
        render_loop_handler.run().unwrap()
    }
}

pub trait RenderPipeline<A>: 'static + Sized {
    const SRGB: bool = true;

    fn optional_features() -> wgpu::Features {
        wgpu::Features::empty()
    }

    fn required_features() -> wgpu::Features {
        wgpu::Features::empty()
    }

    fn required_downlevel_capabilities() -> wgpu::DownlevelCapabilities {
        wgpu::DownlevelCapabilities {
            flags: wgpu::DownlevelFlags::empty(),
            shader_model: wgpu::ShaderModel::Sm5,
            ..wgpu::DownlevelCapabilities::default()
        }
    }

    fn required_limits() -> wgpu::Limits {
        wgpu::Limits::downlevel_webgl2_defaults()
    }

    fn init(
        config: wgpu::SurfaceConfiguration,
        adapter: &wgpu::Adapter,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        window: Arc<Window>,
    ) -> Self;

    fn resize(
        &mut self,
        config: wgpu::SurfaceConfiguration,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    );

    fn window_event(&mut self, _event: winit::event::WindowEvent) {}
    //fn device_event(&mut self, _event: winit::event::DeviceEvent) {}

    fn render(
        &mut self,
        target_view: &wgpu::TextureView,
        target_format: wgpu::TextureFormat,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        egui_ctx: &mut egui::Context,
        app: &mut A,
    );
}

struct RenderPipelineState<A, R: RenderPipeline<A>> {
    window: Arc<Window>,
    surface: SurfaceWrapper,
    context: ContextWrapper,
    render_pipeline: R,
    color_target: wgpu::Texture,
    egui_pass: EguiPass,
    pipeline_database: PipelineDatabase,

    _phantom: PhantomData<A>,
}

impl<A, R: RenderPipeline<A>> RenderPipelineState<A, R> {
    pub async fn from_window(
        mut surface: SurfaceWrapper,
        window: Arc<Window>,
        no_gpu_validation: bool,
    ) -> Self {
        let context = ContextWrapper::init_with_window(
            &mut surface,
            window.clone(),
            R::optional_features(),
            R::required_features(),
            R::required_downlevel_capabilities(),
            R::required_limits(),
            no_gpu_validation,
        )
        .await;

        surface.resume(&context, window.clone(), R::SRGB);

        let render_pipeline = R::init(
            surface.config().clone(),
            &context.adapter,
            &context.device,
            &context.queue,
            window.clone(),
        );

        let color_target = context.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("color_texture"),
            size: wgpu::Extent3d {
                width: surface.config().width,
                height: surface.config().height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[wgpu::TextureFormat::Rgba16Float],
        });

        let egui_pass = EguiPass::new(color_target.format(), 1, &window, &context.device);

        let pipeline_database = PipelineDatabase::new();

        Self {
            window,
            surface,
            context,
            render_pipeline,
            color_target,
            egui_pass,
            pipeline_database,
            _phantom: PhantomData,
        }
    }

    pub fn resize(&mut self, size: winit::dpi::PhysicalSize<u32>) {
        self.surface.resize(&self.context, size);

        self.render_pipeline.resize(
            self.surface.config().clone(),
            &self.context.device,
            &self.context.queue,
        );

        self.color_target = self
            .context
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("color_texture"),
                size: wgpu::Extent3d {
                    width: self.surface.config().width,
                    height: self.surface.config().height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba16Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[wgpu::TextureFormat::Rgba16Float],
            });

        self.window.request_redraw();
    }
}

pub struct ApplicationHandler<A, R: RenderPipeline<A>> {
    // Render pipeline state can be recreated or dropped altogether when in android or web. Applications out of focus lose access to the graphics device.
    rp_state: Option<RenderPipelineState<A, R>>,
    #[cfg(target_arch = "wasm32")]
    rp_state_reciever: Option<futures::channel::oneshot::Receiver<RenderPipelineState<U, R>>>,

    // The app state, everything except graphics should be persistent and stay in RAM.
    app: Runtime<A>,

    frame_idx: u32,
}

impl<A, R: RenderPipeline<A>> ApplicationHandler<A, R> {
    pub fn new(app: Runtime<A>) -> Self {
        Self {
            rp_state: None,
            #[cfg(target_arch = "wasm32")]
            rp_state_reciever: None,
            app,
            frame_idx: 0,
        }
    }

    pub fn run(
        mut self,
        #[cfg(target_os = "android")] android_app: android_activity::AndroidApp,
    ) -> Result<(), EventLoopError> {
        let mut builder = EventLoop::builder();

        #[cfg(target_os = "android")]
        {
            use winit::platform::android::{
                activity::WindowManagerFlags, EventLoopBuilderExtAndroid,
            };

            android_app.set_window_flags(
                WindowManagerFlags::FULLSCREEN | WindowManagerFlags::KEEP_SCREEN_ON,
                WindowManagerFlags::empty(),
            );

            builder.with_android_app(android_app).handle_volume_keys();
        }

        let event_loop = builder.build().unwrap();
        event_loop.set_control_flow(ControlFlow::Poll);
        event_loop.run_app(&mut self)
    }

    #[cfg(target_arch = "wasm32")]
    pub fn poll_state(&mut self) -> bool {
        let mut received_new_state = false;

        if let Some(receiver) = &mut self.rp_state_reciever {
            if let Ok(rp_state) = receiver.try_recv() {
                received_new_state = rp_state.is_some();

                if received_new_state {
                    self.rp_state = rp_state;

                    // The render loop handles will probably have missed the initial resize event, send it again
                    let rp_state = self.rp_state.as_mut().unwrap();
                    rp_state.render_pipeline.resize(
                        rp_state.surface.config().clone(),
                        &rp_state.context.device,
                        &rp_state.context.queue,
                    );
                }
            }
        }

        if received_new_state {
            self.rp_state_reciever = None;
        }

        received_new_state
    }
}

impl<A, R: RenderPipeline<A>> winit::application::ApplicationHandler for ApplicationHandler<A, R> {
    fn new_events(&mut self, _event_loop: &ActiveEventLoop, _cause: winit::event::StartCause) {}

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let surface = if let Some(rp_state) = self.rp_state.take() {
            rp_state.surface
        } else {
            SurfaceWrapper::new()
        };

        #[allow(unused_mut)]
        let mut window_attributes = Window::default_attributes().with_maximized(true);
        #[cfg(target_arch = "wasm32")]
        {
            use wasm_bindgen::JsCast;
            use winit::platform::web::WindowAttributesExtWebSys;
            let canvas = web_sys::window()
                .unwrap()
                .document()
                .unwrap()
                .get_element_by_id("canvas")
                .unwrap()
                .dyn_into::<web_sys::HtmlCanvasElement>()
                .unwrap();

            window_attributes = window_attributes.with_canvas(Some(canvas));
        }

        let window = Arc::new(event_loop.create_window(window_attributes).unwrap());

        cfg_if::cfg_if! {
            if #[cfg(target_arch = "wasm32")] {
                use futures::channel::oneshot;

                let (sender, receiver) = oneshot::channel::<RenderPipelineState<U, R>>();
                self.rp_state_reciever = Some(receiver);

                wasm_bindgen_futures::spawn_local(async move {
                    let rp_state = RenderPipelineState::<U, R>::from_window(surface, window, false).await;
                    let _ = sender.send(rp_state);
                });
            } else {
                use futures::executor::block_on;

                self.rp_state = Some(block_on(RenderPipelineState::<A, R>::from_window(surface, window, self.app.args.no_gpu_validation)));
            }
        }
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(rp_state) = &mut self.rp_state {
            rp_state.surface.suspend();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        #[cfg(target_arch = "wasm32")]
        if self.poll_state() {
            // Request redraw when a new rp_state has been received, this prevents web from being idle untill the first resize event
            self.rp_state.as_ref().unwrap().window.request_redraw();
        }

        if let Some(rp_state) = &mut self.rp_state {
            rp_state.render_pipeline.window_event(event.clone());
            rp_state
                .egui_pass
                .handle_window_event(&rp_state.window, &event);
        }

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                if let Some(rp_state) = &mut self.rp_state {
                    let Some(frame) = rp_state.surface.acquire(&rp_state.context) else {
                        return;
                    };

                    let frame_format = rp_state.surface.config().view_formats[0];
                    let frame_view = frame.texture.create_view(&wgpu::TextureViewDescriptor {
                        format: Some(frame_format),
                        ..wgpu::TextureViewDescriptor::default()
                    });

                    let view = rp_state
                        .color_target
                        .create_view(&wgpu::TextureViewDescriptor::default());

                    let mut egui_ctx = rp_state.egui_pass.begin_frame(&rp_state.window);

                    rp_state.render_pipeline.render(
                        &view,
                        rp_state.color_target.format(),
                        &rp_state.context.device,
                        &rp_state.context.queue,
                        &mut egui_ctx,
                        &mut self.app.user_app,
                    );

                    rp_state.egui_pass.end_frame(egui_ctx);

                    let mut command_encoder = rp_state.context.device.create_command_encoder(
                        &wgpu::CommandEncoderDescriptor {
                            label: Some("Finalize"),
                        },
                    );

                    let screen_descriptor = egui_util::ScreenDescriptor {
                        size_in_pixels: [
                            rp_state.color_target.width(),
                            rp_state.color_target.height(),
                        ],
                        pixels_per_point: rp_state.window.scale_factor() as f32,
                    };
                    rp_state.egui_pass.encode(
                        &rp_state.window,
                        &view,
                        screen_descriptor,
                        &rp_state.context.device,
                        &rp_state.context.queue,
                        &mut command_encoder,
                    );

                    encode_blit(
                        &BlitPassParameters {
                            src_view: &view,
                            dst_view: &frame_view,
                            target_format: frame_format,
                            blending: None,
                        },
                        &rp_state.context.device,
                        &mut command_encoder,
                        &mut rp_state.pipeline_database,
                    );

                    rp_state
                        .context
                        .queue
                        .submit(Some(command_encoder.finish()));

                    frame.present();

                    rp_state.window.request_redraw();
                }

                self.frame_idx += 1;
            }
            WindowEvent::Resized(size) => {
                if let Some(rp_state) = &mut self.rp_state {
                    rp_state.resize(size);
                }
            }
            _ => (),
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: winit::event::DeviceId,
        _event: winit::event::DeviceEvent,
    ) {
        // if let Some(rp_state) = &mut self.rp_state {
        //     // rp_state.render_pipeline.device_event(event.clone());

        //     // TODO: send to different rp_state
        // }
    }
}
