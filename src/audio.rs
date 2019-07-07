use std::convert::TryFrom;

mod mixer;
pub mod music;
mod sink;
mod source;

pub use sink::AudioThread;

// this probably would be i16 were it not for Interpolators requiring f64 frames
pub type SampleFormat = f64;

const HIGH_QUALITY_INTERPOLATION: bool = true;

#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Channels {
    Stereo,
    Mono,
}

impl TryFrom<u32> for Channels {
    type Error = ();

    fn try_from(channels: u32) -> Result<Self, Self::Error> {
        match channels {
            1 => Ok(Channels::Mono),
            2 => Ok(Channels::Stereo),
            _ => Err(()),
        }
    }
}

impl TryFrom<u16> for Channels {
    type Error = ();

    fn try_from(channels: u16) -> Result<Self, Self::Error> {
        Self::try_from(u32::from(channels))
    }
}

impl TryFrom<u8> for Channels {
    type Error = ();

    fn try_from(channels: u8) -> Result<Self, Self::Error> {
        Self::try_from(u32::from(channels))
    }
}
