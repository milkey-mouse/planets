use std::convert::TryFrom;

mod mixer;
pub mod music;
pub mod sink;
pub mod source;

// this probably would be i16 were it not for Interpolators requiring f64 frames
pub type SampleFormat = f64;

const HIGH_QUALITY_INTERPOLATION: bool = true;

#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Channels {
    Stereo,
    Mono,
}

/*impl Into<u8> for Channels {
    fn into(self) -> u8 {
        match self {
            Channels::Mono => 1,
            Channels::Stereo => 2,
        }
    }
}*/

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
        Self::try_from(channels as u32)
    }
}

impl TryFrom<u8> for Channels {
    type Error = ();

    fn try_from(channels: u8) -> Result<Self, Self::Error> {
        Self::try_from(channels as u32)
    }
}
