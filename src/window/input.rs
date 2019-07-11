// TODO: write a crate to convert scan codes to virtual keys
// à la Windows's MapVirtualKeyA().
// or we could cut out the middleman and convert to keys' unicode value.
// Windows has ToUnicode/ToUnicodeEx, macOS has UCKeyTranslate, X11 and Wayland
// are probably totally different as well.
// perhaps this belongs in winit? if not, it could make its own tiny crate.
// (for macOS, see also https://github.com/JensAyton/KeyNaming)
use arr_macro::arr;
use crossbeam_utils::atomic::AtomicCell;
use winit::event::ScanCode;

use std::{
    convert::TryInto,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

pub struct Keybind<'a> {
    // TODO: keep sizeof(Keybind) <= 64 so AtomicCell<Arc<Keybind>> is lock-free
    state: &'a KeyState,
    //scan_code: ScanCode,
    index: usize,
}

impl<'a> Keybind<'a> {
    pub fn new(state: &'a KeyState, scan_code: ScanCode) -> Self {
        let index = state.add(scan_code);

        Self {
            state,
            //scan_code,
            index,
        }
    }

    pub fn pressed(&self) -> bool {
        self.state.pressed(self.index)
    }

    pub fn down(&self) -> bool {
        self.state.down(self.index)
    }

    pub fn released(&self) -> bool {
        self.state.released(self.index)
    }
}

impl<'a> Drop for Keybind<'a> {
    fn drop(&mut self) {
        self.state.remove(self.index);
    }
}

pub struct KeyState {
    state_map: [AtomicCell<Option<ScanCode>>; 64],
    old_state: AtomicU64,
    state: AtomicU64,
}

impl KeyState {
    pub fn new() -> Self {
        Self {
            // TODO: remove arr_macro once Default is generic over array lengths >= 32
            //state_map: [AtomicCell::new(None); 64],
            state_map: arr![AtomicCell::new(None); 64],
            old_state: AtomicU64::new(0),
            state: AtomicU64::new(0),
        }
    }

    pub fn bind(&self, scan_code: ScanCode) -> AtomicCell<Arc<Keybind>> {
        AtomicCell::new(Arc::new(Keybind::new(&self, scan_code)))
    }

    fn add(&self, scan_code: ScanCode) -> usize {
        let (new_index, slot) = self
            .state_map
            .iter()
            .enumerate()
            .find(|(_, x)| x.load().is_none())
            .unwrap();

        slot.store(Some(scan_code));

        new_index
    }

    fn remove(&self, index: usize) {
        let pointer = 1u64.wrapping_shl(index.try_into().unwrap());
        self.state.fetch_and(!pointer, Ordering::Release);
        self.old_state.fetch_and(!pointer, Ordering::Release);

        self.state_map[index].store(None);
    }

    fn pressed(&self, index: usize) -> bool {
        !Self::get(&self.old_state, index) && Self::get(&self.state, index)
    }

    fn down(&self, index: usize) -> bool {
        Self::get(&self.state, index)
    }

    fn released(&self, index: usize) -> bool {
        Self::get(&self.old_state, index) && !Self::get(&self.state, index)
    }

    fn get(state: &AtomicU64, index: usize) -> bool {
        let pointer = 1u64.wrapping_shl(index.try_into().unwrap());
        let state = state.load(Ordering::Acquire);

        state & pointer != 0
    }

    pub fn set(&self, scan_code: ScanCode, pressed: bool) {
        let index = self
            .state_map
            .iter()
            .enumerate()
            .find(|(_, x)| x.load() == Some(scan_code))
            .map(|(i, _)| i)
            .unwrap();

        let pointer = 1u64.wrapping_shl(index.try_into().unwrap());
        if pressed {
            self.state.fetch_or(pointer, Ordering::Release);
        } else {
            self.state.fetch_and(!pointer, Ordering::Release);
        }
    }

    pub fn update(&self) {
        let state = self.state.load(Ordering::Acquire);
        self.old_state.store(state, Ordering::Release);
    }
}
