use vulkano::{
    buffer::{cpu_access::CpuAccessibleBuffer, BufferAccess, BufferUsage},
    device::{Device, DeviceExtensions},
    format::Format,
    framebuffer::{Framebuffer, FramebufferAbstract, RenderPassAbstract, Subpass},
    image::{swapchain::SwapchainImage, ImageUsage},
    instance::{
        debug::{DebugCallback, MessageTypes},
        layers_list, Instance, QueueFamily,
    },
    pipeline::{viewport::Viewport, GraphicsPipeline, GraphicsPipelineAbstract},
    single_pass_renderpass,
    swapchain::{Surface, SurfaceTransform, Swapchain},
    sync::{self, GpuFuture},
};
use winit::{dpi::PhysicalSize, window::Window};

use std::{iter::FromIterator, sync::Arc, u32};

use super::{
    config::{self, DeviceConfig},
    queues::{self, QueuePriorities, Queues},
};
use crate::{
    get_app_info,
    util::{clamp_window_size, ToExtents},
};

const ENABLE_VALIDATION_LAYERS: bool = cfg!(debug_assertions);
const VALIDATION_LAYERS: &[&str] = &["VK_LAYER_KHRONOS_validation"];

pub fn create_instance() -> (Arc<Instance>, Option<DebugCallback>) {
    let layers = if ENABLE_VALIDATION_LAYERS {
        if check_validation_layer_support() {
            VALIDATION_LAYERS
        } else {
            eprintln!("warning: validation layers are unavailable");
            &[]
        }
    } else {
        &[]
    }
    .iter()
    .copied();

    // window-drawing functionality is in non-core extensions
    let mut extensions = vulkano_win::required_extensions();

    if ENABLE_VALIDATION_LAYERS {
        // TODO: this should be ext_debug_utils (_report is deprecated)
        // ext_debug_utils doesn't yet exist in vulkano
        extensions.ext_debug_report = true;
    }

    let instance = Instance::new(Some(&get_app_info()), &extensions, layers)
        .expect("Failed to create Vulkan instance");

    let debug_callback = setup_debug_callback(&instance);

    (instance, debug_callback)
}

fn check_validation_layer_support() -> bool {
    // TODO: maybe use prefer() here? simplify
    let layers: Vec<_> = layers_list()
        .unwrap()
        .map(|l| l.name().to_owned())
        .collect();
    VALIDATION_LAYERS
        .iter()
        .all(|layer_name| layers.contains(&layer_name.to_string()))
}

fn setup_debug_callback(instance: &Arc<Instance>) -> Option<DebugCallback> {
    if ENABLE_VALIDATION_LAYERS {
        let msg_types = MessageTypes {
            error: true,
            warning: true,
            performance_warning: true,
            information: false,
            debug: true,
        };

        DebugCallback::new(&instance, msg_types, |msg| {
            eprintln!("[validation]{}", msg.description);
        })
        .ok()
    } else {
        None
    }
}

pub fn create_logical_device(
    instance: &Arc<Instance>,
    surface: &Arc<Surface<Window>>,
) -> (Arc<Device>, DeviceConfig, Queues) {
    let (physical_device, device_config) = config::pick_physical_device(&instance, &surface);

    // one might think if queue_families.graphics == queue_families.compute
    // we wouldn't have to have multiple (redundant, in this case) entries.
    // but there might be multiple queues in a queue family, and we still
    // want to have different ones if possible.
    let queue_families: Vec<(QueueFamily, f32)> = {
        let families = device_config
            .queue_families
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

    (device, device_config, queues)
}

pub fn create_swapchain(
    surface: Arc<Surface<Window>>,
    device: Arc<Device>,
    dimensions: PhysicalSize,
    device_config: &DeviceConfig,
    queues: &Queues,
) -> (Arc<Swapchain<Window>>, Vec<Arc<SwapchainImage<Window>>>) {
    let capabilities = &device_config.capabilities;

    let image_count = capabilities
        .max_image_count
        .unwrap_or(u32::MAX)
        .min(capabilities.min_image_count + 1);

    let image_usage = ImageUsage {
        color_attachment: true,
        ..ImageUsage::none()
    };

    Swapchain::new(
        device,
        surface,
        image_count,
        device_config.surface_format.0,
        clamp_window_size(dimensions, capabilities).to_extents(),
        1,
        image_usage,
        queues::get_sharing_mode(&device_config.queue_families, &queues),
        SurfaceTransform::Identity,
        config::choose_alpha_mode(capabilities.supported_composite_alpha),
        device_config.present_mode,
        true,
        None, // old_swapchain
    )
    .expect("Failed to create swapchain")
}

pub fn create_render_pass(
    device: Arc<Device>,
    color_format: Format,
) -> Arc<dyn RenderPassAbstract + Send + Sync> {
    Arc::new(
        single_pass_renderpass!(device,
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

pub fn create_graphics_pipeline(
    device: Arc<Device>,
    dimensions: PhysicalSize,
    device_config: &DeviceConfig,
    render_pass: Arc<dyn RenderPassAbstract + Send + Sync>,
) -> Arc<dyn GraphicsPipelineAbstract + Send + Sync> {
    use crate::shaders::{particle_frag, particle_vert};

    let vertex = particle_vert::Shader::load(device.clone())
        .expect("Failed to create/compile vertex shader module");
    let fragment = particle_frag::Shader::load(device.clone())
        .expect("Failed to create/compile fragment shader module");

    let capabilities = &device_config.capabilities;
    let viewport = Viewport {
        origin: [0.0, 0.0],
        dimensions: clamp_window_size(dimensions, capabilities).to_extents(),
        depth_range: 0.0..1.0,
    };

    Arc::new(
        // TODO: simplify pipeline builder settings
        // see main.old.rs (old branch) and vulkan-tutorial-rs
        GraphicsPipeline::start()
            .vertex_input_single_buffer::<particle_vert::Vertex>()
            .vertex_shader(vertex.main_entry_point(), ())
            .point_list()
            .primitive_restart(false)
            .viewports(vec![viewport])
            .fragment_shader(fragment.main_entry_point(), ())
            .depth_clamp(false)
            // TODO: "there's a commented out .rasterizer_discard() in Vulkano..."
            .render_pass(Subpass::from(render_pass, 0).unwrap())
            .build(device)
            .expect("Failed to create graphics pipeline"),
    )
}

pub fn create_framebuffers(
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

pub fn create_vertex_buffer(device: Arc<Device>) -> Arc<dyn BufferAccess + Send + Sync> {
    use crate::shaders::particle_vert::Vertex;

    // TODO: better buffer type
    CpuAccessibleBuffer::from_iter(
        device,
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

pub fn create_sync_objects(device: Arc<Device>) -> Box<dyn GpuFuture> {
    Box::new(sync::now(device))
}
