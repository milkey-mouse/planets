use vulkano::{
    buffer::{cpu_access::CpuAccessibleBuffer, BufferAccess, BufferUsage},
    command_buffer::{AutoCommandBuffer, AutoCommandBufferBuilder, DynamicState},
    device::{Device, DeviceExtensions},
    format::Format,
    framebuffer::{Framebuffer, FramebufferAbstract, RenderPassAbstract, Subpass},
    image::{swapchain::SwapchainImage, ImageUsage},
    instance::{ApplicationInfo, Instance, PhysicalDevice, QueueFamily},
    pipeline::{viewport::Viewport, GraphicsPipeline, GraphicsPipelineAbstract},
    single_pass_renderpass,
    swapchain::{
        acquire_next_image, AcquireError, Surface, SurfaceTransform, Swapchain,
        SwapchainCreationError,
    },
    sync::{self, GpuFuture},
};
use vulkano_win::VkSurfaceBuild;
use winit::{dpi::LogicalSize, Event, EventsLoop, Window, WindowBuilder, WindowEvent};

use std::{iter::FromIterator, sync::Arc, thread};

mod assets;
mod audio;
mod render;
use render::{
    config::{self, DeviceConfig},
    queues::{self, QueueFamilies, QueuePriorities, Queues},
    shaders,
};

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 720;

struct PlanetsGame {
    instance: Arc<Instance>,
    event_loop: EventsLoop,
    surface: Arc<Surface<Window>>,
    device_config: DeviceConfig,
    device: Arc<Device>,
    queues: Queues,
    swapchain: Arc<Swapchain<Window>>,
    swapchain_images: Vec<Arc<SwapchainImage<Window>>>,
    render_pass: Arc<dyn RenderPassAbstract + Send + Sync>,
    graphics_pipeline: Arc<dyn GraphicsPipelineAbstract + Send + Sync>,
    swapchain_framebuffers: Vec<Arc<dyn FramebufferAbstract + Send + Sync>>,
    vertex_buffer: Arc<dyn BufferAccess + Send + Sync>,
    command_buffers: Vec<Arc<AutoCommandBuffer>>,
    previous_frame_end: Option<Box<dyn GpuFuture>>,
    recreate_swapchain: bool,
}

// TODO: prefer() function for best available choice
// it would be something like this:
// `fn prefer<'a, T>(wanted: Iterable<&T>, supported: Iterable<&T>, default_to_first: bool) -> Option<&'a T>`
// we can replace some of the queue_family, format, present_mode, etc.
// selection logic with this fn. the lifetime annotations in the above
// signature may be unnecessary; I don't know Rust well enough to know
// without a `cargo check`

impl PlanetsGame {
    pub fn init() -> Self {
        let app_info = ApplicationInfo {
            engine_name: Some("Newton".into()),
            ..vulkano::app_info_from_cargo_toml!()
        };

        let instance = Self::create_instance(&app_info);
        let (event_loop, surface) = Self::create_surface(&instance, &app_info);

        let (physical_device_index, device_config) =
            config::pick_physical_device(&instance, &surface);
        let (device, queues) = Self::create_logical_device(
            &instance,
            physical_device_index,
            &device_config.queue_families,
        );

        let (swapchain, swapchain_images) =
            Self::create_swapchain(&surface, &device, &queues, &device_config);

        let render_pass = Self::create_render_pass(&device, swapchain.format());
        let graphics_pipeline =
            Self::create_graphics_pipeline(&device, device_config.extents, &render_pass);

        let swapchain_framebuffers = Self::create_framebuffers(&swapchain_images, &render_pass);

        let previous_frame_end = Some(Self::create_sync_objects(&device));

        let vertex_buffer = Self::create_vertex_buffer(&device);

        let mut app = Self {
            instance,
            event_loop,
            surface,
            device_config,
            device,
            queues,
            swapchain,
            swapchain_images,
            render_pass,
            graphics_pipeline,
            swapchain_framebuffers,
            vertex_buffer,
            command_buffers: vec![],
            previous_frame_end,
            recreate_swapchain: false,
        };

        app.create_command_buffers();

        app
    }

    fn create_instance(app_info: &ApplicationInfo) -> Arc<Instance> {
        // window-drawing functionality is in non-core extensions
        let extensions = vulkano_win::required_extensions();

        Instance::new(Some(&app_info), &extensions, None).expect("Failed to create Vulkan instance")
    }

    fn create_swapchain(
        surface: &Arc<Surface<Window>>,
        device: &Arc<Device>,
        queues: &Queues,
        device_config: &DeviceConfig,
    ) -> (Arc<Swapchain<Window>>, Vec<Arc<SwapchainImage<Window>>>) {
        let capabilities = &device_config.capabilities;

        let image_count = capabilities
            .max_image_count
            .unwrap_or(u32::max_value())
            .min(capabilities.min_image_count + 1);

        let image_usage = ImageUsage {
            color_attachment: true,
            ..ImageUsage::none()
        };

        Swapchain::new(
            device.clone(),
            surface.clone(),
            image_count,
            device_config.surface_format.0,
            device_config.extents,
            1,
            image_usage,
            queues::get_sharing_mode(&device_config.queue_families, &queues),
            SurfaceTransform::Identity,
            config::choose_alpha_mode(&capabilities.supported_composite_alpha),
            device_config.present_mode,
            true,
            None, // old_swapchain
        )
        .expect("Failed to create swapchain")
    }

    fn create_render_pass(
        device: &Arc<Device>,
        color_format: Format,
    ) -> Arc<dyn RenderPassAbstract + Send + Sync> {
        Arc::new(
            single_pass_renderpass!(device.clone(),
                attachments: {
                    color: {
                        load: Clear,
                        store: Store,
                        format: color_format,
                        samples: 1,
                    }
                },
                pass: {
                    color: [color],
                    depth_stencil: {}
                }
            )
            .unwrap(),
        )
    }

    fn create_graphics_pipeline(
        device: &Arc<Device>,
        extents: [u32; 2],
        render_pass: &Arc<dyn RenderPassAbstract + Send + Sync>,
    ) -> Arc<dyn GraphicsPipelineAbstract + Send + Sync> {
        use shaders::particle_vert::Vertex;

        let vertex = shaders::particle_vert::Shader::load(device.clone())
            .expect("Failed to create/compile vertex shader module");
        let fragment = shaders::particle_frag::Shader::load(device.clone())
            .expect("Failed to create/compile fragment shader module");

        let viewport = Viewport {
            origin: [0.0, 0.0],
            dimensions: [extents[0] as f32, extents[1] as f32],
            depth_range: 0.0..1.0,
        };

        Arc::new(
            // TODO: simplify pipeline builder settings
            // see main.old.rs (old branch) and vulkan-tutorial-rs
            GraphicsPipeline::start()
                .vertex_input_single_buffer::<Vertex>()
                .vertex_shader(vertex.main_entry_point(), ())
                .point_list()
                .primitive_restart(false)
                .viewports(vec![viewport])
                .fragment_shader(fragment.main_entry_point(), ())
                .depth_clamp(false)
                // TODO: "there's a commented out .rasterizer_discard() in Vulkano..."
                .render_pass(Subpass::from(render_pass.clone(), 0).unwrap())
                .build(device.clone())
                .expect("Failed to create graphics pipeline"),
        )
    }

    fn create_framebuffers(
        swapchain_images: &[Arc<SwapchainImage<Window>>],
        render_pass: &Arc<dyn RenderPassAbstract + Send + Sync>,
    ) -> Vec<Arc<dyn FramebufferAbstract + Send + Sync>> {
        swapchain_images
            .iter()
            .map(|image| {
                let fba: Arc<dyn FramebufferAbstract + Send + Sync> = Arc::new(
                    Framebuffer::start(render_pass.clone())
                        .add(image.clone())
                        .expect("Failed to add image to framebuffer")
                        .build()
                        .expect("Failed to build framebuffer"),
                );
                fba
            })
            .collect()
    }

    fn create_vertex_buffer(device: &Arc<Device>) -> Arc<dyn BufferAccess + Send + Sync> {
        use shaders::particle_vert::Vertex;

        // TODO: better buffer type
        CpuAccessibleBuffer::from_iter(
            device.clone(),
            BufferUsage::vertex_buffer(),
            [
                Vertex {
                    position: [-0.5, -0.5],
                    ..Default::default()
                },
                Vertex {
                    position: [-0.5, 0.5],
                    ..Default::default()
                },
                Vertex {
                    position: [0.5, 0.5],
                    ..Default::default()
                },
                Vertex {
                    position: [0.5, -0.5],
                    ..Default::default()
                },
            ]
            .iter()
            .cloned(),
        )
        .expect("Failed to create vertex buffer")
    }

    fn create_command_buffers(&mut self) {
        let queue_family = self.queues.graphics.family();
        self.command_buffers = self
            .swapchain_framebuffers
            .iter()
            .map(|fb| {
                Arc::new(
                    AutoCommandBufferBuilder::primary_simultaneous_use(
                        self.device.clone(),
                        queue_family,
                    )
                    .unwrap()
                    .begin_render_pass(fb.clone(), false, vec![[0.0, 0.0, 0.0, 1.0].into()])
                    .unwrap()
                    .draw(
                        self.graphics_pipeline.clone(),
                        &DynamicState::none(),
                        vec![self.vertex_buffer.clone()],
                        (),
                        (),
                    )
                    .unwrap()
                    .end_render_pass()
                    .unwrap()
                    .build()
                    .unwrap(),
                )
            })
            .collect();
    }

    fn create_sync_objects(device: &Arc<Device>) -> Box<dyn GpuFuture> {
        Box::new(sync::now(device.clone())) //as Box<dyn GpuFuture>
    }

    fn create_logical_device(
        instance: &Arc<Instance>,
        index: usize,
        queue_families: &QueueFamilies,
    ) -> (Arc<Device>, Queues) {
        let physical_device = PhysicalDevice::from_index(&instance, index).unwrap();

        // one might think if queue_families.graphics == queue_families.compute
        // we wouldn't have to have multiple (redundant, in this case) entries.
        // but there might be multiple queues in a queue family, and we still
        // want to have different ones if possible.
        let queue_families: Vec<(QueueFamily, f32)> = {
            let families = queue_families
                .iter()
                .map(|q| physical_device.queue_family_by_id(*q).unwrap());

            // TODO: make graphics priority vs. compute priority configurable
            let priorities: QueuePriorities = Default::default();

            families.zip(priorities.iter().map(|p| *p)).collect()
        };

        let device_ext = DeviceExtensions {
            khr_swapchain: true,
            ..DeviceExtensions::none()
        };

        let (device, queues) = queues::create_device(
            physical_device,
            physical_device.supported_features(),
            &device_ext,
            queue_families,
        )
        .expect("Failed to create logical device");

        let queues = Queues::from_iter(queues);

        (device, queues)
    }

    fn create_surface(
        instance: &Arc<Instance>,
        app_info: &ApplicationInfo,
    ) -> (EventsLoop, Arc<Surface<Window>>) {
        let event_loop = EventsLoop::new();
        let surface = if let Some(name) = &app_info.application_name {
            WindowBuilder::new().with_title(name.to_owned())
        } else {
            WindowBuilder::new()
        }
        .with_dimensions(LogicalSize::new(WIDTH.into(), HEIGHT.into()))
        .build_vk_surface(&event_loop, instance.clone())
        .expect("Failed to create window Vulkan surface");
        (event_loop, surface)
    }

    fn main_loop(&mut self) {
        let mut done = false;
        while !done {
            self.draw_frame();

            let mut recreate_swapchain = self.recreate_swapchain;
            self.event_loop.poll_events(|ev| match ev {
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    ..
                }
                | Event::WindowEvent {
                    event: WindowEvent::Destroyed,
                    ..
                } => done = true,
                Event::WindowEvent {
                    event: WindowEvent::Resized(_),
                    ..
                } => recreate_swapchain = true,
                //evt => {dbg!(evt);},
                _ => (),
            });
            self.recreate_swapchain = recreate_swapchain;
        }
    }

    fn draw_frame(&mut self) {
        self.previous_frame_end.as_mut().unwrap().cleanup_finished();

        if self.recreate_swapchain {
            if self.recreate_swapchain() {
                self.recreate_swapchain = false;
            } else {
                return;
            }
        }

        let (index, acquire_future) = match acquire_next_image(self.swapchain.clone(), None) {
            Ok(r) => r,
            Err(AcquireError::OutOfDate) => {
                self.recreate_swapchain = !self.recreate_swapchain();
                return;
            }
            Err(err) => panic!("{:?}", err),
        };

        let command_buffer = self.command_buffers[index].clone();

        let future = self
            .previous_frame_end
            .take()
            .unwrap()
            .join(acquire_future)
            .then_execute(self.queues.graphics.clone(), command_buffer)
            .unwrap()
            .then_swapchain_present(self.queues.present.clone(), self.swapchain.clone(), index)
            .then_signal_fence_and_flush();

        self.previous_frame_end = Some(match future {
            Ok(future) => Box::new(future),
            Err(sync::FlushError::OutOfDate) => {
                self.recreate_swapchain = true;
                Box::new(sync::now(self.device.clone()))
            }
            Err(e) => {
                eprintln!("Frame end sync failed: {:?}", e);
                Box::new(sync::now(self.device.clone()))
            }
        });
    }

    fn recreate_swapchain(&mut self) -> bool {
        let extents = config::get_extents(&self.device_config.capabilities, &self.surface.window());
        let (swapchain, images) = match self.swapchain.recreate_with_dimension(extents) {
            Ok(r) => r,
            Err(SwapchainCreationError::UnsupportedDimensions) => return false,
            Err(err) => panic!("{:?}", err),
        };
        self.swapchain = swapchain;
        self.swapchain_images = images;

        self.render_pass = Self::create_render_pass(&self.device, self.swapchain.format());
        self.graphics_pipeline = Self::create_graphics_pipeline(
            &self.device,
            self.swapchain.dimensions(),
            &self.render_pass,
        );
        self.swapchain_framebuffers =
            Self::create_framebuffers(&self.swapchain_images, &self.render_pass);
        self.create_command_buffers();

        return true;
    }
}

fn main() {
    let mut game = PlanetsGame::init();
    thread::spawn(audio::sink::test);
    game.main_loop();
}
