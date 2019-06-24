use sample::{
    frame::{Frame, Stereo},
    interpolate, ring_buffer,
    signal::Signal,
};

pub mod music;
pub mod sink;
mod source;

// TODO: dynamically set master sample rate to rate of open sink
const MASTER_SAMPLE_RATE: u32 = 44100;
const HIGH_QUALITY_INTERPOLATION: bool = true;
//const SINC_INITIALIZATION_ARRAY: [f64; 100];

// this probably would be i16 were it not for Interpolators requiring f64 frames
pub type CanonicalFormat = f64;
pub trait CanonicalSignal = Signal<Frame = Stereo<CanonicalFormat>> + Send + Sync;

fn resample<F>(
    in_rate: u32,
    out_rate: Option<u32>,
    mut signal: Box<dyn Signal<Frame = F> + Send + Sync>,
) -> Box<dyn Signal<Frame = F> + Send + Sync>
where
    F: Frame<Sample = CanonicalFormat> + Send + Sync + 'static,
{
    let out_rate = out_rate.unwrap_or(MASTER_SAMPLE_RATE);

    if in_rate == out_rate {
        signal
    } else if HIGH_QUALITY_INTERPOLATION {
        let buffer = ring_buffer::Fixed::from([F::equilibrium(); 100]);
        let sinc = interpolate::Sinc::new(buffer);
        Box::new(signal.from_hz_to_hz(sinc, in_rate.into(), out_rate.into()))
    } else {
        let linear = interpolate::Linear::from_source(&mut signal);
        Box::new(signal.from_hz_to_hz(linear, in_rate.into(), out_rate.into()))
    }
}
