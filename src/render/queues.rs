use vulkano::{
    device::{Device, DeviceCreationError, Features, Queue, RawDeviceExtensions},
    instance::{PhysicalDevice, QueueFamily},
    swapchain::Surface,
    sync::SharingMode,
};
use winit::Window;

use std::{sync::Arc, vec::IntoIter};

// TODO: lib w/ proc_macro #derive(Iter) on structs with fields of uniform type

// TODO: keep QueueList<T> private while typedefs are public
pub struct QueueList<T> {
    pub graphics: T,
    pub compute: T,
    pub transfer: T,
    pub present: T,
}

pub type QueueFamilies = QueueList<u32>;
pub type Queues = QueueList<Arc<Queue>>;

pub fn find_queue_families(
    surface: &Arc<Surface<Window>>,
    device: &PhysicalDevice,
) -> Result<QueueFamilies, ()> {
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
    let graphics_capable: Vec<_> = device
        .queue_families()
        .filter(|&q| q.supports_graphics())
        .collect();
    let graphics_family = graphics_capable
        .iter()
        .find(|&q| surface.is_supported(*q).unwrap_or(false))
        // or if no such queues exist on this physical device, just choose the
        // first graphics-capable queue family. if none exist (very weird for
        // a GPU to have no graphics capabilities!) throw an error
        .or_else(|| graphics_capable.first())
        .ok_or(())?;
    let graphics = graphics_family.id();

    let present = if surface.is_supported(*graphics_family).unwrap_or(false) {
        // if the graphics queue supports presentation, use that here too
        graphics
    } else {
        // otherwise use the first queue family capable of presentation
        device
            .queue_families()
            .find(|q| surface.is_supported(*q).unwrap_or(false))
            .ok_or(())?
            .id()
    };

    // for compute, try to choose a different queue than for graphics, but
    // fall back to the one used for graphics if it's the only one capable
    // if no queue can run compute shaders, obviously we raise an error
    let compute_capable: Vec<_> = device
        .queue_families()
        .filter(|&q| q.supports_compute())
        .collect();
    // for compute, first try to choose a queue family that *only* supports
    // compute and not graphics. these dedicated queues are probably faster
    let compute_family = compute_capable
        .iter()
        .find(|&q| !q.supports_graphics())
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
    let explicitly_supports_transfers: Vec<_> = device
        .queue_families()
        .filter(|&q| q.explicitly_supports_transfers())
        .collect();
    let transfer = explicitly_supports_transfers
        .iter()
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

    Ok(QueueFamilies {
        graphics,
        compute,
        transfer,
        present,
    })
}

// the sharing mode this function creates allows all queues to share
pub fn get_sharing_mode(queue_families: &QueueFamilies, queues: &Queues) -> SharingMode {
    use std::collections::HashMap;

    // order is reversed from the struct so later queues take priority
    let unique_queues = [
        (queue_families.graphics, &queues.graphics),
        (queue_families.compute, &queues.compute),
        (queue_families.transfer, &queues.transfer),
        (queue_families.present, &queues.present),
    ]
    .iter()
    .cloned()
    .collect::<HashMap<_, _>>()
    .values()
    .map(|q| *q)
    .collect::<Vec<_>>();

    if unique_queues.len() == 1 {
        unique_queues[0].into()
    } else {
        unique_queues.as_slice().into()
    }
}

/// This function is just like the normal Device::new(), except that it
/// ensures the indices of the returned queues align with the indices of
/// the given QueueFamilies even if multiple families correspond to the
/// same queue (i.e. they have the same ID).
pub fn create_device<'a, I, Ext>(
    phys: PhysicalDevice,
    requested_features: &Features,
    extensions: Ext,
    queue_families: I,
) -> Result<(Arc<Device>, IntoIter<Arc<Queue>>), DeviceCreationError>
where
    I: IntoIterator<Item = (QueueFamily<'a>, f32)>,
    Ext: Into<RawDeviceExtensions>,
{
    use std::collections::HashMap;

    let mut families = Vec::new();
    let unique_queue_families = queue_families.into_iter().filter(|(q, _)| {
        let seen = families.contains(&q.id());
        families.push(q.id());
        !seen
    });

    let (device, output_queues) =
        Device::new(phys, requested_features, extensions, unique_queue_families)?;

    let output_queues_map: HashMap<_, _> = output_queues.map(|q| (q.family().id(), q)).collect();
    let redundant_output_queues = families
        .into_iter()
        .map(|id| output_queues_map.get(&id).unwrap())
        .cloned()
        .collect::<Vec<_>>()
        .into_iter();

    Ok((device, redundant_output_queues))
}
