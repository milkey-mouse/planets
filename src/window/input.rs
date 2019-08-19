// TODO: write a crate to convert scan codes to virtual keys
// à la Windows's MapVirtualKeyA().
// or we could cut out the middleman and convert to keys' unicode value.
// Windows has ToUnicode/ToUnicodeEx, macOS has UCKeyTranslate, X11 and Wayland
// are probably totally different as well.
// perhaps this belongs in winit? if not, it could make its own tiny crate.
// (for macOS, see also https://github.com/JensAyton/KeyNaming)
use arr_macro::arr;
use crossbeam_utils::atomic::AtomicCell;
use hashed::Hashed32;
use winit::event::{ButtonId, DeviceEvent, DeviceId, ScanCode};

use std::{
    convert::{TryFrom, TryInto},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

// TODO: all of this only handles binary inputs
// TODO: refactor this file out into its own library

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum InputID {
    None,
    Button(ButtonId),
    Key(ScanCode),
}

impl Default for InputID {
    fn default() -> Self {
        Self::None
    }
}

impl TryFrom<DeviceEvent> for InputID {
    type Error = ();

    fn try_from(evt: DeviceEvent) -> Result<Self, Self::Error> {
        match evt {
            DeviceEvent::Button { button, .. } => Ok(Self::Button(button)),
            DeviceEvent::Key(kb_input) => Ok(Self::Key(kb_input.scancode)),
            _ => Err(()),
        }
    }
}

#[derive(Copy, Clone, Default, Debug)]
pub struct Input {
    input_id: InputID,
    device: Hashed32<Option<DeviceId>>,
}

impl Input {
    pub fn new(input_id: InputID, device: Hashed32<Option<DeviceId>>) -> Self {
        Self { input_id, device }
    }
}

impl PartialEq for Input {
    fn eq(&self, other: &Self) -> bool {
        let any_device = Default::default();

        self.input_id == other.input_id
            && (self.device == other.device
                || self.device == any_device
                || other.device == any_device)
    }
}

impl Into<Input> for InputID {
    fn into(self) -> Input {
        Input {
            input_id: self,
            device: None.into(),
        }
    }
}

pub struct InputBinding<'a> {
    state: &'a KeyState,
    input: Input,
    index: usize,
}

impl<'a> InputBinding<'a> {
    pub fn new(state: &'a KeyState, input: Input) -> Self {
        let index = state.add(input);

        Self {
            state,
            input,
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

    pub fn name() -> String {
        unimplemented!()
    }
}

impl<'a> Drop for InputBinding<'a> {
    fn drop(&mut self) {
        self.state.remove(self.index);
    }
}

pub struct KeyState {
    state_map: [AtomicCell<Input>; 64],
    old_state: AtomicU64,
    state: AtomicU64,
}

impl KeyState {
    pub fn new() -> Self {
        Self {
            // TODO: remove arr_macro once Default is generic over array lengths >= 32
            //state_map: [AtomicCell::new(Default::default()); 64],
            state_map: arr![AtomicCell::new(Default::default()); 64],
            old_state: AtomicU64::new(0),
            state: AtomicU64::new(0),
        }
    }

    pub fn bind(&self, input: Input) -> AtomicCell<Arc<InputBinding>> {
        AtomicCell::new(Arc::new(InputBinding::new(&self, input)))
    }

    fn add(&self, input: Input) -> usize {
        let empty_slot = Default::default();

        let (new_index, slot) = self
            .state_map
            .iter()
            .enumerate()
            .find(|(_, x)| x.load() == empty_slot)
            .unwrap();

        slot.store(input);

        new_index
    }

    fn remove(&self, index: usize) {
        let pointer = 1u64.wrapping_shl(index.try_into().unwrap());
        self.state.fetch_and(!pointer, Ordering::Release);
        self.old_state.fetch_and(!pointer, Ordering::Release);

        self.state_map[index].store(Default::default());
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

    pub fn set(&self, input: Input, pressed: bool) {
        dbg!(input);
        if let Some(index) = self
            .state_map
            .iter()
            .enumerate()
            .find(|(_, x)| x.load() == input)
            .map(|(i, _)| i)
        {
            let pointer = 1u64.wrapping_shl(index.try_into().unwrap());

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
