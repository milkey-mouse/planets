use std::{
    panic,
    sync::atomic::{AtomicBool, Ordering},
};
use vulkano::swapchain::Capabilities;
use winit::dpi::PhysicalSize;

pub fn clamp<T: PartialOrd>(num: T, min: T, max: T) -> T {
    assert!(max > min);
    if num < min {
        min
    } else if num > max {
        max
    } else {
        num
    }
}

// TODO: prefer() with references

// TODO: prefer() should be a wrapper around prefer_fn()
pub fn prefer<'a, T: PartialEq + 'a>(
    wanted: impl IntoIterator<Item = &'a T>,
    supported: impl IntoIterator<Item = T>,
    default_to_first: bool,
) -> Option<T> {
    // NOTE: if T were guaranteed to support Hash, we could use a HashSet here
    let wanted = wanted.into_iter().collect::<Vec<_>>();
    let mut supported = supported.into_iter();

    let first = match supported.next() {
        Some(x) => x,
        None => return None,
    };

    if wanted.contains(&&first) {
        Some(first)
    } else {
        supported
            .find(|x| wanted.contains(&x))
            .or_else(|| Some(first).filter(|_| default_to_first))
    }
}

pub fn prefer_fn<'a, T: 'a>(
    wanted: impl Fn(&T) -> bool,
    supported: impl IntoIterator<Item = T>,
    default_to_first: bool,
) -> Option<T> {
    let mut supported = supported.into_iter();

    let first = match supported.next() {
        Some(x) => x,
        None => return None,
    };

    if wanted(&first) {
        Some(first)
    } else {
        supported
            .find(wanted)
            .or_else(|| Some(first).filter(|_| default_to_first))
    }
}

static SETUP_HOOK: AtomicBool = AtomicBool::new(false);

pub struct IntentionalPanic;

impl IntentionalPanic {
    pub fn setup_hook() {
        if !SETUP_HOOK.load(Ordering::Acquire) {
            let original_hook = panic::take_hook();
            panic::set_hook(Box::new(move |panic_info| {
                if panic_info.payload().downcast_ref::<Self>().is_none() {
                    original_hook(panic_info);
                }
            }));

            SETUP_HOOK.store(true, Ordering::Release);
        }
    }
}

pub fn clamp_window_size(dims: PhysicalSize, caps: &Capabilities) -> PhysicalSize {
    return dims;

    let Capabilities {
        min_image_extent: min,
        max_image_extent: max,
        ..
    } = caps;

    (
        clamp(dims.width, min[0].into(), max[0].into()),
        clamp(dims.height, min[1].into(), max[1].into()),
    )
        .into()
}

pub trait ToExtents<T> {
    fn to_extents(self) -> [T; 2];
}

impl<T> ToExtents<T> for [T; 2] {
    fn to_extents(self) -> [T; 2] {
        self
    }
}

impl ToExtents<u32> for PhysicalSize {
    fn to_extents(self) -> [u32; 2] {
        let x: (u32, u32) = self.into();
        [x.0, x.1]
    }
}

impl ToExtents<f64> for PhysicalSize {
    fn to_extents(self) -> [f64; 2] {
        let x: (f64, f64) = self.into();
        [x.0, x.1]
    }
}

impl ToExtents<f32> for PhysicalSize {
    fn to_extents(self) -> [f32; 2] {
        let x: (f64, f64) = self.into();
        [x.0 as f32, x.1 as f32]
    }
}
