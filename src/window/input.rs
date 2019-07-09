// TODO: write a crate to convert scan codes to virtual keys
// à la Windows's MapVirtualKeyA().
// or we could cut out the middleman and convert to keys' unicode value.
// Windows has ToUnicode/ToUnicodeEx, macOS has UCKeyTranslate, X11 and Wayland
// are probably totally different as well.
// perhaps this belongs in winit? if not, it could make its own tiny crate.
// (for macOS, see also https://github.com/JensAyton/KeyNaming)
use winit::event::ScanCode;

use std::{
    collections::{HashMap, HashSet},
    convert::TryInto,
    sync::{
        atomic::{AtomicU64, Ordering},
        RwLock,
    },
};

pub struct Keybinds {
    free_indices: RwLock<HashSet<u8>>,
    state_map: RwLock<HashMap<ScanCode, u8>>,
    old_state: AtomicU64,
    state: AtomicU64,
}

impl Keybinds {
    pub fn new() -> Self {
        Self {
            free_indices: RwLock::new(HashSet::new()),
            state_map: RwLock::new(HashMap::new()),
            old_state: AtomicU64::new(0),
            state: AtomicU64::new(0),
        }
    }

    pub fn add(&self, scan_code: ScanCode) {
        let new_index = self
            .free_indices
            .write()
            .unwrap()
            .drain()
            .next()
            .unwrap_or(self.state_map.read().unwrap().len().try_into().unwrap());
        assert!(new_index < 64);

        assert!(self
            .state_map
            .write()
            .unwrap()
            .insert(scan_code, new_index)
            .is_none());
    }

    pub fn remove(&self, scan_code: ScanCode) {
        if let Some(deleted_index) = self.state_map.write().unwrap().remove(&scan_code) {
            self.free_indices.write().unwrap().insert(deleted_index);
        }

        /*loop {
            let state = self.state.load(Ordering::Acquire);
            let old_state = self.old_state.load(Ordering::Acquire);
            let mask = u64::max_value() << deleted_index;
        }

        for (_, index) in state_map.iter_mut() {
            if *index > deleted_index {
                index -= 1;

            }
        }*/
    }

    fn get(&self, state: &AtomicU64, scan_code: ScanCode) -> bool {
        let index = *self.state_map.read().unwrap().get(&scan_code).unwrap();
        let pointer = 1u64.wrapping_shl(index.into());
        let state = state.load(Ordering::Acquire);

        state & pointer != 0
    }

    pub fn pressed(&self, scan_code: ScanCode) -> bool {
        !self.get(&self.old_state, scan_code) && self.get(&self.state, scan_code)
    }

    pub fn down(&self, scan_code: ScanCode) -> bool {
        self.get(&self.state, scan_code)
    }

    pub fn released(&self, scan_code: ScanCode) -> bool {
        self.get(&self.old_state, scan_code) && !self.get(&self.state, scan_code)
    }

    pub fn set(&self, scan_code: ScanCode, pressed: bool) {
        if let Some(&index) = self.state_map.read().unwrap().get(&scan_code) {
            let pointer = 1u64.wrapping_shl(index.into());

            if pressed {
                self.state.fetch_or(pointer, Ordering::Release);
            } else {
                self.state.fetch_and(!pointer, Ordering::Release);
            }
        }
    }

    pub fn update(&self) {
        let state = self.state.load(Ordering::Acquire);
        self.old_state.store(state, Ordering::Release);
    }
}
