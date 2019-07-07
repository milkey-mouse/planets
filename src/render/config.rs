use vulkano::{
    device::DeviceExtensions,
    format::Format,
    instance::{Instance, PhysicalDevice},
    swapchain::{
        Capabilities, ColorSpace, CompositeAlpha, PresentMode, SupportedCompositeAlpha,
        SupportedPresentModes, Surface,
    },
};
use winit::window::Window;

use std::sync::Arc;

use super::queues::{self, QueueFamilies};
use crate::util::prefer;

pub struct DeviceConfig {
    pub queue_families: QueueFamilies,
    pub capabilities: Capabilities,
    pub surface_format: (Format, ColorSpace),
    pub present_mode: PresentMode,
}

pub fn choose_alpha_mode(supported: SupportedCompositeAlpha) -> CompositeAlpha {
    // prefer premultiplied over opaque over inherit alpha modes
    // postmultiplied mode won't work well because we're cheating
    // by making the clear color the only transparency in the game
    // and drawing everything else as if there were none (alpha 255)
    const WANTED: &[CompositeAlpha] = &[
        CompositeAlpha::PreMultiplied,
        CompositeAlpha::Opaque,
        CompositeAlpha::Inherit,
    ];

    prefer(WANTED, supported.iter(), true).unwrap()
}

pub fn pick_physical_device<'a>(
    instance: &'a Arc<Instance>,
    surface: &Surface<Window>,
) -> (PhysicalDevice<'a>, DeviceConfig) {
    let mut device_config = Err(());
    let device = PhysicalDevice::enumerate(&instance)
        .find(|device| {
            device_config = create_device_config(surface, &device);
            device_config.is_ok()
        })
        .expect("No Vulkan-capable devices (GPUs) found");
    (device, device_config.unwrap())
}

pub fn create_device_config(
    surface: &Surface<Window>,
    device: &PhysicalDevice,
) -> Result<DeviceConfig, ()> {
    if !check_device_extension_support(device) {
        return Err(());
    }

    let queue_families = queues::find_queue_families(surface, device)?;

    // TODO: selectively enable panic!ing on failures like the one below
    // instead of just moving onto the next GPU/physical device
    //let capabilities = surface.capabilities(*device).expect("Failed to enumerate surface capabilities");
    let capabilities = surface.capabilities(*device).ok().ok_or(())?;
    let surface_format = choose_surface_format(&capabilities.supported_formats)?;
    let present_mode = choose_present_mode(capabilities.present_modes)?;

    Ok(DeviceConfig {
        queue_families,
        capabilities,
        surface_format,
        present_mode,
    })
}

fn required_device_extensions(inherit: Option<DeviceExtensions>) -> DeviceExtensions {
    DeviceExtensions {
        khr_swapchain: true,
        ..inherit.unwrap_or_else(DeviceExtensions::none)
    }
}

fn check_device_extension_support(device: &PhysicalDevice) -> bool {
    let available = DeviceExtensions::supported_by_device(*device);
    // if adding all our required extensions doesn't change the struct, it
    // already contained the extensions we needed (i.e. they are supported)
    required_device_extensions(Some(available)) == available
}

fn choose_surface_format(
    available_formats: &[(Format, ColorSpace)],
) -> Result<(Format, ColorSpace), ()> {
    // TODO: why prefer Unorm and not Srgb?
    // is it more widely supported?
    available_formats
        .iter()
        .find(|(format, color_space)| {
            *format == Format::B8G8R8A8Unorm && *color_space == ColorSpace::SrgbNonLinear
        })
        .or_else(|| available_formats.first())
        .copied()
        .ok_or(())
}

fn choose_present_mode(available: SupportedPresentModes) -> Result<PresentMode, ()> {
    const WANTED: &[PresentMode] = &[
        PresentMode::Mailbox,
        PresentMode::Immediate,
        PresentMode::Fifo,
    ];

    prefer(WANTED, available.iter(), true).ok_or(())
}
