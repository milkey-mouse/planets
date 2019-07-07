use vulkano::{
    device::{Device, DeviceCreationError, Features, Queue, RawDeviceExtensions},
    instance::{PhysicalDevice, QueueFamily},
    swapchain::Surface,
    sync::SharingMode,
};
use winit::window::Window;

use std::{
    convert::TryInto,
    iter::{repeat, DoubleEndedIterator, ExactSizeIterator, FromIterator},
    sync::Arc,
    vec::IntoIter,
};

use crate::util::prefer_fn;

// TODO: lib w/ proc_macro #derive(Iter) on structs with fields of uniform type

const QUEUE_LIST_SIZE: u8 = 4;
pub struct QueueList<T> {
    pub graphics: T,
    pub compute: T,
    pub transfer: T,
    pub present: T,
    // if you add more queues, remember to update QUEUE_LIST_SIZE above
}

impl<'a, T> QueueList<T> {
    pub fn iter(&'a self) -> QueueListIterator<'a, T> {
        QueueListIterator::new(&self)
    }
}

impl<T> FromIterator<T> for QueueList<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut iter = iter.into_iter();
        let ret = Self {
            graphics: iter.next().unwrap(),
            compute: iter.next().unwrap(),
            transfer: iter.next().unwrap(),
            present: iter.next().unwrap(),
        };

        assert!(iter.next().is_none());

        ret
    }
}

pub struct QueueListIterator<'a, T> {
    list: &'a QueueList<T>,
    index_forward: u8,
    index_back: u8,
}

impl<'a, T> QueueListIterator<'a, T> {
    fn new(list: &'a QueueList<T>) -> Self {
        QueueListIterator {
            list,
            index_forward: 0,
            index_back: QUEUE_LIST_SIZE,
        }
    }

    fn lookup(&mut self, index: u8) -> &'a T {
        match index {
            0 => &self.list.graphics,
            1 => &self.list.compute,
            2 => &self.list.transfer,
            3 => &self.list.present,
            _ => unreachable!(),
        }
    }
}

impl<'a, T> Iterator for QueueListIterator<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index_forward == self.index_back {
            None
        } else {
            let ret = self.lookup(self.index_forward);
            self.index_forward += 1;

            Some(ret)
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let size: usize = (self.index_back - self.index_forward).try_into().unwrap();
        (size, Some(size))
    }
}

impl<'a, T> ExactSizeIterator for QueueListIterator<'a, T> {}

impl<'a, T> DoubleEndedIterator for QueueListIterator<'a, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.index_forward == self.index_back {
            None
        } else {
            self.index_back -= 1;
            Some(self.lookup(self.index_back))
        }
    }
}

pub type QueuePriorities = QueueList<f32>;
pub type QueueFamilies = QueueList<u32>;
pub type Queues = QueueList<Arc<Queue>>;

impl Default for QueuePriorities {
    fn default() -> Self {
        repeat(1.0)
            .take(QUEUE_LIST_SIZE.try_into().unwrap())
            .collect()
    }
}

pub fn find_queue_families(
    surface: &Surface<Window>,
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
    // presentation commands, which would be faster than separate queues.
    // if no such queues exist on this physical device, just choose the first
    // graphics-capable queue family.
    let graphics = prefer_fn(
        |&q| surface.is_supported(q).unwrap_or(false),
        device.queue_families().filter(|&q| q.supports_graphics()),
        true,
    )
    // if none exist (weird for a GPU to have no graphics capabilities!), fail
    .ok_or(())?;

    let present = if surface.is_supported(graphics).unwrap_or(false) {
        // if the graphics queue supports presentation, use that here too
        graphics
    } else {
        // otherwise use the first queue family capable of presentation
        device
            .queue_families()
            .find(|q| surface.is_supported(*q).unwrap_or(false))
            .ok_or(())?
    };

    // for compute, first try to choose a queue family that *only* supports
    // compute and not graphics. these dedicated queues are probably faster
    let compute = device
        .queue_families()
        .find(|&q| q.supports_compute() && !q.supports_graphics())
        // otherwise, try to choose a different queue family than for graphics.
        // if all else fails, fall back to the queue family used for graphics
        .or_else(|| {
            prefer_fn(
                |&q| q.id() != graphics.id(),
                device.queue_families().filter(|&q| q.supports_compute()),
                true,
            )
        })
        // of course, if the graphics queue isn't even in the compute_capable
        // list (i.e. it doesn't support compute shaders at all, and neither
        // do any other queue families), fail
        .ok_or(())?;

    // it wouldn't be that bad if graphics and transfer shared a queue.
    // but many discrete GPUs have a separate queue *explicitly* supporting
    // transfers (even though transfer operation support is implied by
    // graphics or compute support) to "indicate a special relationship with
    // the DMA module and more efficient transfers."

    // try to find such a queue, with only transfer support. if no such queue
    // exists, fall back to any queue that explicitly supports transfers.
    // (perhaps it's still faster than the others?)
    let transfer = prefer_fn(
        |&q| q.id() != graphics.id() && q.id() != compute.id(),
        device
            .queue_families()
            .filter(|&q| q.explicitly_supports_transfers()),
        true,
    )
    // if there are no transfer-exclusive queues, try extra compute queues
    .or_else(|| {
        prefer_fn(
            |&q| q.id() != compute.id(),
            device.queue_families().filter(|&q| q.supports_compute()),
            true,
        )
    })
    // ...or extra graphics queues...
    // if push comes to shove, share a queue (family) with graphics.
    .or_else(|| {
        prefer_fn(
            |&q| q.id() != graphics.id(),
            device.queue_families().filter(|&q| q.supports_graphics()),
            true,
        )
    })
    // this should never fail since we can always fall back to the graphics
    // queue; if there were no graphics queue, we wouldn't have gotten here
    .unwrap();

    Ok(QueueFamilies {
        graphics: graphics.id(),
        compute: compute.id(),
        transfer: transfer.id(),
        present: present.id(),
    })
}

// the sharing mode this function creates allows all queues to share
pub fn get_sharing_mode(queue_families: &QueueFamilies, queues: &Queues) -> SharingMode {
    use std::collections::HashMap;

    let unique_queues = queue_families
        .iter()
        .zip(queues.iter())
        .rev() // reverse the order so earlier queues "bump out" later ones
        .collect::<HashMap<_, _>>()
        .values()
        .copied()
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
