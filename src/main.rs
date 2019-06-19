use winit::{
    dpi::LogicalSize,
    Event,
    EventsLoop,
    Window,
    WindowBuilder,
    WindowEvent
};
use vulkano::{
    buffer::{
        cpu_access::CpuAccessibleBuffer,
        BufferUsage,
        BufferAccess,
    },
    command_buffer::{
        AutoCommandBuffer,
        AutoCommandBufferBuilder,
        DynamicState,
    },
    device::{
        Device,
        DeviceCreationError,
        DeviceExtensions,
        Features,
        Queue,
        RawDeviceExtensions,
    },
    format::Format,
    framebuffer::{
        RenderPassAbstract,
        Subpass,
        FramebufferAbstract,
        Framebuffer,
    },
    image::{
        ImageUsage,
        swapchain::SwapchainImage,
    },
    instance::{
        ApplicationInfo,
        Instance,
        PhysicalDevice,
        QueueFamily,
    },
    pipeline::{
        GraphicsPipeline,
        GraphicsPipelineAbstract,
        viewport::Viewport,
    },
    single_pass_renderpass,
    swapchain::{
        acquire_next_image,
        Surface,
        SurfaceTransform,
        Capabilities,
        ColorSpace,
        SupportedPresentModes,
        PresentMode,
        Swapchain,
        CompositeAlpha,
    },
    sync::{
        SharingMode,
        GpuFuture,
    },
};
use vulkano_win::VkSurfaceBuild;

use std::sync::Arc;
use std::vec::IntoIter;

mod shaders;

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 720;

struct QueueFamilyIDs {
    graphics: u32,
    compute: u32,
    transfer: u32,
    present: u32,
}

struct Queues {
    graphics: Arc<Queue>,
    compute: Arc<Queue>,
    transfer: Arc<Queue>,
    present: Arc<Queue>,
}

// TODO: impl Iterator for QueueFamilyIDs, Queues

struct DeviceConfig {
    queue_families: QueueFamilyIDs,
    capabilities: Capabilities,
    surface_format: (Format, ColorSpace),
    present_mode: PresentMode,
    extents: [u32; 2],
}

struct PlanetsGame {
    instance: Arc<Instance>,
    event_loop: EventsLoop,
    surface: Arc<Surface<Window>>,
    // TODO: store PhysicalDevice directly (lifetime issues)
    physical_device_index: usize,
    device: Arc<Device>,
    queues: Queues,
    swapchain: Arc<Swapchain<Window>>,
    swapchain_images: Vec<Arc<SwapchainImage<Window>>>,
    render_pass: Arc<RenderPassAbstract + Send + Sync>,
    graphics_pipeline: Arc<GraphicsPipelineAbstract + Send + Sync>,
    swapchain_framebuffers: Vec<Arc<FramebufferAbstract + Send + Sync>>,
    vertex_buffer: Arc<BufferAccess + Send + Sync>,
    command_buffers: Vec<Arc<AutoCommandBuffer>>,
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

        let (physical_device_index, device_config) = Self::pick_physical_device(&instance, &surface);
        let (device, queues) = Self::create_logical_device(&instance, physical_device_index, &device_config.queue_families);

        let (swapchain, swapchain_images) = Self::create_swapchain(&instance, &surface, &device, &queues, &device_config);

        let render_pass = Self::create_render_pass(&device, swapchain.format());
        let graphics_pipeline = Self::create_graphics_pipeline(&device, &device_config, &render_pass);

        let swapchain_framebuffers = Self::create_framebuffers(&swapchain_images, &render_pass);

        let vertex_buffer = Self::create_vertex_buffer(&device);

        let mut app = Self {
            instance,
            event_loop,
            surface,
            physical_device_index,
            device,
            queues,
            swapchain,
            swapchain_images,
            render_pass,
            graphics_pipeline,
            swapchain_framebuffers,
            vertex_buffer,
            command_buffers: vec![],
        };

        app.create_command_buffers();
        
        app
    }

    fn create_instance(app_info: &ApplicationInfo) -> Arc<Instance> {
        // window-drawing functionality is in non-core extensions
        let extensions = vulkano_win::required_extensions();

        Instance::new(Some(&app_info), &extensions, None).expect("Failed to create Vulkan instance")
    }

    fn pick_physical_device(instance: &Arc<Instance>, surface: &Arc<Surface<Window>>) -> (usize, DeviceConfig) {
        let mut device_config = Err(());
        let device_index = PhysicalDevice::enumerate(&instance)
            .find(|device| {
                device_config = Self::create_device_config(surface, &device);
                device_config.is_ok()
            })
            .expect("No Vulkan-capable devices (GPUs) found")
            .index();
       (device_index, device_config.unwrap())
    }

    // TODO: move all DeviceConfig-related functions to a 'device' module
    // perhaps have all of this in a 'render' submodule?
    
    // TODO: abstract this fn's return type into a struct (DeviceConfig? taken
    // by vulkano?) as it's passed around a bit before being used/destructured
    fn create_device_config(surface: &Arc<Surface<Window>>, device: &PhysicalDevice) -> Result<DeviceConfig, ()> {
        let queue_families = Self::find_queue_families(surface, device)?;

        if !Self::check_device_extension_support(device) {
            return Err(());
        }

        // TODO: selectively enable panic!ing on failures like the one below
        // instead of just moving onto the next GPU/physical device
        //let capabilities = surface.capabilities(*device).expect("Failed to enumerate surface capabilities");
        let capabilities = surface.capabilities(*device).ok().ok_or(())?;
        let surface_format = Self::choose_surface_format(&capabilities.supported_formats)?;
        let present_mode = Self::choose_present_mode(capabilities.present_modes)?;
        let extents = Self::get_extents(&capabilities, &surface.window());

        Ok(DeviceConfig {
            queue_families,
            capabilities,
            surface_format,
            present_mode,
            extents,
        })
    }

    fn find_queue_families(surface: &Arc<Surface<Window>>, device: &PhysicalDevice) -> Result<QueueFamilyIDs, ()> {
        // TODO: implement PartialEQ on QueueFamilies
        // this could be done by q.id() && q.physical_device().id().
        // this is really a vulkano problem...
        // actually, seems like many (most?) of these are missing Eq<> impl's

        // TODO: use HashSet<QueueFamily> instead of Vec<_>
        // .filter(|&q| q.supports_graphics() would turn into a subtraction
        // from the graphics_capable set. could this impact use of prefer()?
        // also useful for transfer queue selection (union of graphics & comp.)

        // NOTE: in these comments, "queue" actually refers to a queue *family*

        // for graphics, try to find a queue that supports both drawing and
        // presentation commands, which would be faster than separate queues
        let graphics_capable: Vec<_> = device.queue_families().filter(|&q| q.supports_graphics()).collect();
        let graphics_family = graphics_capable.iter().find(|&q| surface.is_supported(*q).unwrap_or(false))
        // or if no such queues exist on this physical device, just choose the
        // first graphics-capable queue family. if none exist (very weird for
        // a GPU to have no graphics capabilities!) throw an error
            .or_else(|| graphics_capable.first()).ok_or(())?;
        let graphics = graphics_family.id();

        let present = if surface.is_supported(*graphics_family).unwrap_or(false) {
            // if the graphics queue supports presentation, use that here too
            graphics
        } else {
            // otherwise use the first queue family capable of presentation
            device.queue_families().find(|q| surface.is_supported(*q).unwrap_or(false)).ok_or(())?.id()
        };

        // for compute, try to choose a different queue than for graphics, but
        // fall back to the one used for graphics if it's the only one capable
        // if no queue can run compute shaders, obviously we raise an error
        let compute_capable: Vec<_> = device.queue_families().filter(|&q| q.supports_compute()).collect();
        // for compute, first try to choose a queue family that *only* supports
        // compute and not graphics. these dedicated queues are probably faster
        let compute_family = compute_capable.iter().find(|&q| !q.supports_graphics())
        // otherwise, try to choose a different queue family than for graphics
            .or_else(|| compute_capable.iter().find(|&q| q.id() != graphics))
        // if all else fails, fall back to the queue family used for graphics
        // this queue would be first in the list because it was for graphics
            .or_else(|| compute_capable.first())
        // of course, if the graphics queue isn't even in the compute_capable
        // list (i.e. it doesn't support compute shaders at all, and neither
        // do any other queue families), fail
            .ok_or(())?;
        let compute = compute_family.id();

        // it wouldn't be that bad if graphics and transfer shared a queue.
        // but many discrete GPUs have a separate queue *explicitly* supporting
        // transfers (even though transfer operation support is implied by
        // graphics or compute support) to "indicate a special relationship with
        // the DMA module and more efficient transfers."
        let explicitly_supports_transfers: Vec<_> = device.queue_families().filter(|&q| q.explicitly_supports_transfers()).collect();
        let transfer = explicitly_supports_transfers.iter()
        // try to find such a queue, with only transfer support
            .find(|&q| q.id() != graphics && q.id() != compute)
        // fall back to any queue that explicitly supports transfers
        // (perhaps it's still faster than the others?)
            .or_else(|| explicitly_supports_transfers.first())
        // if there are no transfer-exclusive queues, try extra graphics queues
            .or_else(|| graphics_capable.iter().find(|&q| q.id() != graphics))
        // ...or extra compute queues...
            .or_else(|| compute_capable.iter().find(|&q| q.id() != compute))
        // and if push comes to shove, share a queue (family) with graphics
        // this should never fail because if there were no graphics queue, we
        // wouldn't have gotten this far anyway
            .unwrap_or(graphics_family)
            .id();

        Ok(QueueFamilyIDs { graphics, compute, transfer, present })
    }

    fn required_device_extensions(inherit: Option<DeviceExtensions>) -> DeviceExtensions {
        DeviceExtensions {
            khr_swapchain: true,
            ..inherit.unwrap_or(DeviceExtensions::none())
        }
    }

    fn check_device_extension_support(device: &PhysicalDevice) -> bool {
        let available = DeviceExtensions::supported_by_device(*device);
        // if adding all our required extensions doesn't change the struct, it
        // already contained the extensions we needed (i.e. they are supported)
        Self::required_device_extensions(Some(available)) == available
    }

    fn choose_surface_format(available_formats: &[(Format, ColorSpace)]) -> Result<(Format, ColorSpace), ()> {
        // TODO: why prefer Unorm and not Srgb?
        // is it more widely supported?
        available_formats.iter()
            .find(|(format, color_space)|
                *format == Format::B8G8R8A8Unorm && *color_space == ColorSpace::SrgbNonLinear
            )
            .or_else(|| available_formats.first())
            .map(|f| *f)
            .ok_or(())
    }

    fn choose_present_mode(available_present_modes: SupportedPresentModes) -> Result<PresentMode, ()> {
        if available_present_modes.mailbox {
            Ok(PresentMode::Mailbox)
        } else if available_present_modes.immediate {
            Ok(PresentMode::Immediate)
        } else if available_present_modes.fifo {
            Ok(PresentMode::Fifo)
        } else {
            available_present_modes.iter().next().ok_or(())
        }
    }

    fn get_extents(capabilities: &Capabilities, window: &Window) -> [u32; 2] {
        let dims: (u32, u32) = window
            .get_inner_size()
            .unwrap_or_else(|| LogicalSize::new(WIDTH.into(), HEIGHT.into()))
            .to_physical(window.get_hidpi_factor())
            .into();

        capabilities.current_extent.unwrap_or_else(|| [
            // TODO: use clamp() when stabilized
            // see rust-lang/rust#44095
            capabilities.min_image_extent[0].max(capabilities.max_image_extent[0].min(dims.0)),
            capabilities.min_image_extent[1].max(capabilities.max_image_extent[1].min(dims.1)),
        ])
    }

    fn choose_surface_transform(capabilities: &Capabilities) -> SurfaceTransform {
        // we could do the same sort of thing as choose_alpha_mode() (prefer())
        // but this seems to give a sane default
        capabilities.current_transform
    }

    // TODO: take only needed field of Capabilities for these fn's
    fn choose_alpha_mode(capabilities: &Capabilities) -> CompositeAlpha {
        // prefer premultiplied over opaque over inherit alpha modes
        // postmultiplied mode won't work well because we're cheating
        // by making the clear color the only transparency in the game
        // and drawing everything else as if there was none
        let supported = capabilities.supported_composite_alpha;
        [
            CompositeAlpha::PreMultiplied,
            CompositeAlpha::Opaque,
            CompositeAlpha::Inherit,
        ].iter()
            .cloned()
            .filter(|a| supported.supports(*a))
            .next()
            .or(supported.iter().next())
            .unwrap()
    }

    fn create_swapchain(
        instance: &Arc<Instance>,
        surface: &Arc<Surface<Window>>,
        device: &Arc<Device>,
        queues: &Queues,
        device_config: &DeviceConfig,
    ) -> (Arc<Swapchain<Window>>, Vec<Arc<SwapchainImage<Window>>>) {
        let capabilities = &device_config.capabilities;

        let mut image_count = capabilities.min_image_count + 1;
        if capabilities.max_image_count.is_some() && image_count > capabilities.max_image_count.unwrap() {
            image_count = capabilities.max_image_count.unwrap();
        }

        let image_usage = ImageUsage {
            color_attachment: true,
            ..ImageUsage::none()
        };

        // TODO: refactor into get_sharing_mode
        let sharing_mode: SharingMode = {
            use std::collections::HashMap;
            
            let queue_families = &device_config.queue_families;
            
            // order is reversed from the struct so later queues take priority            
            [
                (queue_families.graphics, &queues.graphics),
                (queue_families.compute, &queues.compute),
                (queue_families.transfer, &queues.transfer),
                (queue_families.present, &queues.present),
            ].iter()
                .cloned()
                .collect::<HashMap<u32, &Arc<Queue>>>()
                .values()
                .map(|q| *q)
                .collect::<Vec<_>>()
                .as_slice()
                .into()
        };

        Swapchain::new(
            device.clone(),
            surface.clone(),
            image_count,
            device_config.surface_format.0,
            device_config.extents,
            1,
            image_usage,
            sharing_mode,
            Self::choose_surface_transform(&capabilities),
            Self::choose_alpha_mode(&capabilities),
            device_config.present_mode,
            true,
            None, // old_swapchain
        ).expect("Failed to create swapchain")
    }

    fn create_render_pass(device: &Arc<Device>, color_format: Format) -> Arc<RenderPassAbstract + Send + Sync> {
        Arc::new(single_pass_renderpass!(device.clone(),
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
        ).unwrap())
    }

    fn create_graphics_pipeline(
        device: &Arc<Device>,
        device_config: &DeviceConfig,
        render_pass: &Arc<RenderPassAbstract + Send + Sync>
    ) -> Arc<GraphicsPipelineAbstract + Send + Sync> {
        use shaders::particle_vert::Vertex;
        
        let vertex = shaders::particle_vert::Shader::load(device.clone())
            .expect("Failed to create/compile vertex shader module");
        let fragment = shaders::particle_frag::Shader::load(device.clone())
            .expect("Failed to create/compile fragment shader module");

        let viewport = Viewport {
            origin: [0.0, 0.0],
            dimensions: [device_config.extents[0] as f32, device_config.extents[1] as f32],
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
                .expect("Failed to create graphics pipeline")
        )
    }

    fn create_framebuffers(
        swapchain_images: &[Arc<SwapchainImage<Window>>],
        render_pass: &Arc<RenderPassAbstract + Send + Sync>
    ) -> Vec<Arc<FramebufferAbstract + Send + Sync>> {
        // TODO: expect() instead of unwrap() on FB creation
        swapchain_images.iter()
            .map(|image| {
                // TODO: why are we assigning to a variable?
                let fba: Arc<FramebufferAbstract + Send + Sync> = Arc::new(Framebuffer::start(render_pass.clone())
                    .add(image.clone()).unwrap()
                    .build().unwrap());
                fba
            })
            .collect()
    }

    fn create_vertex_buffer(device: &Arc<Device>) -> Arc<BufferAccess + Send + Sync> {
        use shaders::particle_vert::Vertex;
    
        // TODO: better buffer type
        CpuAccessibleBuffer::from_iter(
            device.clone(),
            BufferUsage::vertex_buffer(),
            [
                Vertex { position: [-0.5, -0.5], ..Default::default() },
                Vertex { position: [-0.5, 0.5], ..Default::default() },
                Vertex { position: [0.5, 0.5], ..Default::default() },
                Vertex { position: [0.5, -0.5], ..Default::default() },
            ].iter()
             .cloned(),
        ).expect("Failed to create vertex buffer")
    }

    fn create_command_buffers(&mut self) {
        let queue_family = self.queues.graphics.family();
        self.command_buffers = self.swapchain_framebuffers.iter()
            .map(|fb| {
                Arc::new(AutoCommandBufferBuilder::primary_simultaneous_use(self.device.clone(), queue_family)
                    .unwrap()
                    .begin_render_pass(fb.clone(), false, vec![[0.0, 0.0, 0.0, 1.0].into()])
                    .unwrap()
                    .draw(self.graphics_pipeline.clone(), &DynamicState::none(), vec![self.vertex_buffer.clone()], (), ())
                    .unwrap()
                    .end_render_pass()
                    .unwrap()
                    .build()
                    .unwrap()
                )
            })
            .collect();
    }

    /// This function is just like the normal Device::new(), except that it
    /// ensures the indices of the returned queues align with the indices of
    /// the given QueueFamilies even if multiple families correspond to the
    /// same queue (i.e. they have the same ID).
    fn create_queues<'a, I, Ext>(
        phys: PhysicalDevice,
        requested_features: &Features,
        extensions: Ext,
        queue_families: I
        ) -> Result<(Arc<Device>, IntoIter<Arc<Queue>>), DeviceCreationError>
        where
            I: IntoIterator<Item = (QueueFamily<'a>, f32)>,
            Ext: Into<RawDeviceExtensions>,
    {
        use std::collections::HashMap;
    
        let mut families = Vec::new();
        let unique_queue_families = queue_families.into_iter()
            .filter(|(q, _)| {
                let seen = families.contains(&q.id());
                families.push(q.id());
                !seen
            });

        let (device, output_queues) = Device::new(phys, requested_features, extensions, unique_queue_families)?;

        let output_queues_map: HashMap<_, _> = output_queues.map(|q| (q.family().id(), q)).collect();
        let redundant_output_queues = families.into_iter()
            .map(|id| output_queues_map.get(&id).unwrap())
            .cloned()
            .collect::<Vec<_>>().into_iter();
                
        Ok((device, redundant_output_queues))
    }

    fn create_logical_device(instance: &Arc<Instance>, index: usize, queue_families: &QueueFamilyIDs) -> (Arc<Device>, Queues) {
        let physical_device = PhysicalDevice::from_index(&instance, index).unwrap();

        // one might think if queue_families.graphics == queue_families.compute
        // we wouldn't have to have multiple (redundant, in this case) entries.
        // but there might be multiple queues in a queue family, and we still
        // want to have different ones if possible.
        let queue_families = {
            // TODO: assert len(queue_families) == len(config.queue_families)
            let graphics = physical_device.queue_families().filter(|&q| q.id() == queue_families.graphics).next().unwrap();
            let compute = physical_device.queue_families().filter(|&q| q.id() == queue_families.compute).next().unwrap();
            let transfer = physical_device.queue_families().filter(|&q| q.id() == queue_families.transfer).next().unwrap();
            let present = physical_device.queue_families().filter(|&q| q.id() == queue_families.present).next().unwrap();
            
            // TODO: make graphics priority vs. compute priority configurable
            let graphics_priority = 1.0;
            let compute_priority = 1.0;

            [
                (graphics, graphics_priority),
                (compute, compute_priority),
                (transfer, compute_priority),
                (present, graphics_priority),
            ]
        };

        let device_ext = DeviceExtensions {
            khr_swapchain: true,
            ..DeviceExtensions::none()
        };

        let (device, mut queues) = Self::create_queues(
            physical_device,
            physical_device.supported_features(),
            &device_ext,
            queue_families.iter().cloned()
        ).expect("Failed to create logical device");

        let queues = Queues {
            graphics: queues.next().unwrap(),
            compute: queues.next().unwrap(),
            transfer: queues.next().unwrap(),
            present: queues.next().unwrap(),
        };

        (device, queues)
    }

    fn create_surface(instance: &Arc<Instance>, app_info: &ApplicationInfo) -> (EventsLoop, Arc<Surface<Window>>) {
        let event_loop = EventsLoop::new();
        let surface = if let Some(name) = &app_info.application_name {
            WindowBuilder::new().with_title(name.to_owned())
        } else {
            WindowBuilder::new()
        }.with_dimensions(LogicalSize::new(WIDTH.into(), HEIGHT.into()))
         .build_vk_surface(&event_loop, instance.clone())
         .expect("Failed to create window Vulkan surface");
        (event_loop, surface)
    }

    fn main_loop(&mut self) {
        let mut done = false;
        while !done {
            self.draw_frame();
        
            self.event_loop.poll_events(|ev| match ev {
                Event::WindowEvent { event: WindowEvent::CloseRequested, .. } |
                Event::WindowEvent { event: WindowEvent::Destroyed, .. } => done = true,
                //evt => {dbg!(evt);},
                _ => (),
            });
        }
    }

    fn draw_frame(&mut self) {
        let (index, acquire_future) = acquire_next_image(self.swapchain.clone(), None).unwrap();

        let command_buffer = self.command_buffers[index].clone();

        let future = acquire_future
            .then_execute(self.queues.graphics.clone(), command_buffer)
            .unwrap()
            .then_swapchain_present(self.queues.present.clone(), self.swapchain.clone(), index)
            .then_signal_fence_and_flush()
            .unwrap();

        future.wait(None).unwrap();
    }
}

fn main() {
    let mut game = PlanetsGame::init();
    game.main_loop();
}
