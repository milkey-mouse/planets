use vulkano::{
    buffer::BufferAccess,
    command_buffer::{AutoCommandBuffer, AutoCommandBufferBuilder, DynamicState},
    device::Device,
    framebuffer::{FramebufferAbstract, RenderPassAbstract},
    image::swapchain::SwapchainImage,
    pipeline::GraphicsPipelineAbstract,
    swapchain::{acquire_next_image, AcquireError, Swapchain},
    sync::{self, GpuFuture},
};
use winit::{dpi::PhysicalSize, window::Window as WinitWindow};

use std::sync::Arc;

mod config;
mod queues;
mod setup;

use config::DeviceConfig;
use queues::Queues;

use crate::{
    util::ToExtents,
    window::{Window, WindowEvents},
};

pub use setup::create_instance;

#[derive(Default)]
struct Particle; // TODO: real Particle struct linked to vertex shader

pub struct Render {
    window: Window,
    events: Arc<WindowEvents>,
    device_config: DeviceConfig,
    device: Arc<Device>,
    queues: Queues,
    swapchain: Arc<Swapchain<WinitWindow>>,
    swapchain_images: Vec<Arc<SwapchainImage<WinitWindow>>>,
    render_pass: Arc<dyn RenderPassAbstract + Send + Sync>,
    graphics_pipeline: Arc<dyn GraphicsPipelineAbstract + Send + Sync>,
    swapchain_framebuffers: Vec<Arc<dyn FramebufferAbstract + Send + Sync>>,
    vertex_buffer: Arc<dyn BufferAccess + Send + Sync>,
    command_buffers: Vec<Arc<AutoCommandBuffer>>,
    previous_frame_end: Option<Box<dyn GpuFuture>>,
}

impl Render {
    pub fn new(window: Window) -> Self {
        let events = window.events();

        let (device, device_config, queues) =
            setup::create_logical_device(&window.instance(), &window.surface());

        let dimensions = window.dimensions();

        let (swapchain, swapchain_images) = setup::create_swapchain(
            window.surface(),
            device.clone(),
            dimensions,
            &device_config,
            &queues,
        );

        let render_pass = setup::create_render_pass(device.clone(), swapchain.format());

        let graphics_pipeline = setup::create_graphics_pipeline(
            device.clone(),
            dimensions,
            &device_config,
            render_pass.clone(),
        );

        let swapchain_framebuffers = setup::create_framebuffers(&swapchain_images, &render_pass);

        let vertex_buffer = setup::create_vertex_buffer(device.clone());

        let previous_frame_end = Some(setup::create_sync_objects(device.clone()));

        let mut me = Self {
            window,
            events,
            device_config,
            device,
            queues,
            swapchain,
            swapchain_images,
            render_pass,
            graphics_pipeline,
            swapchain_framebuffers,
            vertex_buffer,
            command_buffers: Vec::new(),
            previous_frame_end,
        };

        me.create_command_buffers();

        me
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

    fn resize_to(&mut self, dimensions: PhysicalSize) {
        let (swapchain, swapchain_images) = self
            .swapchain
            .recreate_with_dimension(dimensions.to_extents())
            .unwrap();
        self.swapchain = swapchain;
        self.swapchain_images = swapchain_images;

        self.render_pass = setup::create_render_pass(self.device.clone(), self.swapchain.format());
        self.graphics_pipeline = setup::create_graphics_pipeline(
            self.device.clone(),
            dimensions,
            &self.device_config,
            self.render_pass.clone(),
        );
        self.swapchain_framebuffers =
            setup::create_framebuffers(&self.swapchain_images, &self.render_pass);
        self.create_command_buffers();
    }

    fn recreate_swapchain(&mut self) {
        self.resize_to(self.window.dimensions());
    }

    fn draw_frame(&mut self) {
        self.previous_frame_end.as_mut().unwrap().cleanup_finished();

        let (index, acquire_future) = loop {
            match acquire_next_image(self.swapchain.clone(), None) {
                Err(AcquireError::OutOfDate) => self.recreate_swapchain(),
                x => break x.unwrap(),
            }
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
                self.recreate_swapchain();
                Box::new(sync::now(self.device.clone()))
            }
            Err(e) => {
                eprintln!("frame end sync failed: {:?}", e);
                Box::new(sync::now(self.device.clone()))
            }
        });
    }

    pub fn update(&mut self) {
        if let Some(new_size) = self.events.resize_to() {
            self.resize_to(new_size);
        }
        self.draw_frame();
    }
}
